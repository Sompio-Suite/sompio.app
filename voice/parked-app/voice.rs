//! Sompio Voice in the shell: connecting, passing control messages and voice packets between
//! the window and the server, hosting a server on this PC, and push-to-talk.
//! Microphone, Opus encoding and playback happen in the window (WebCodecs).

use crate::{AppState, CmdResult};
use serde::Serialize;
use sompio_voice::bookmarks::{self, Saved};
use sompio_voice::client::{self, Connect, Event, Outcome, Session};
use sompio_voice::proto::{ClientMsg, ServerMsg};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

#[derive(Default)]
pub struct VoiceState {
    session: tokio::sync::Mutex<Option<Session>>,
    /// Voice packets go out from here without waiting on the session lock
    voice_out: Mutex<Option<mpsc::UnboundedSender<(u16, Vec<u8>)>>>,
    hosted: tokio::sync::Mutex<Option<sompio_voice::server::Running>>,
    hosted_info: Mutex<Option<Hosted>>,
    /// The port the router opened for it, closed again when the server stops
    opened: tokio::sync::Mutex<Option<sompio_voice::upnp::Opened>>,
    ptt: Mutex<Option<tauri_plugin_global_shortcut::Shortcut>>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn voice_settings(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Saved> {
    let (_, key, path) = state.vault.voice_keys().map_err(err)?;
    let s = bookmarks::load(&path, &key).map_err(err)?;
    let registered = state.voice.ptt.lock().map(|p| p.is_some()).unwrap_or(true);
    if !registered && !s.push_to_talk.is_empty() {
        let _ = set_ptt(&app, &s.push_to_talk);
    }
    Ok(s)
}

#[tauri::command]
pub fn voice_save_settings(app: AppHandle, state: State<'_, AppState>, settings: Saved) -> CmdResult<()> {
    let (_, key, path) = state.vault.voice_keys().map_err(err)?;
    let old = bookmarks::load(&path, &key).map(|s| s.push_to_talk).unwrap_or_default();
    if old != settings.push_to_talk {
        set_ptt(&app, &settings.push_to_talk)?;
    }
    bookmarks::save(&path, &key, &settings).map_err(err)
}

/// Push-to-talk: a global shortcut, so it works while a game has focus.
pub fn set_ptt(app: &AppHandle, keys: &str) -> CmdResult<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let state = app.state::<AppState>();
    let mut cur = state.voice.ptt.lock().map_err(err)?;
    if let Some(old) = cur.take() {
        let _ = app.global_shortcut().unregister(old);
    }
    if keys.is_empty() {
        return Ok(());
    }
    let sc: tauri_plugin_global_shortcut::Shortcut = keys.parse().map_err(|_| format!("\"{keys}\" can't be used as a push-to-talk key."))?;
    app.global_shortcut().register(sc).map_err(|e| format!("{keys} can't be used for push-to-talk ({e}). Another program may already use it."))?;
    *cur = Some(sc);
    Ok(())
}

/// Called for every global shortcut press and release; true if it was push-to-talk.
pub fn on_shortcut(app: &AppHandle, sc: &tauri_plugin_global_shortcut::Shortcut, down: bool) -> bool {
    let Some(state) = app.try_state::<AppState>() else { return false };
    let is_ptt = state.voice.ptt.lock().map(|p| p.as_ref() == Some(sc)).unwrap_or(false);
    if is_ptt {
        let _ = app.emit("voice-ptt", down);
    }
    is_ptt
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectResult {
    Connected { fingerprint: String, welcome: ServerMsg },
    NewServer { fingerprint: String, name: String },
    Changed { expected: String, got: String },
}

#[derive(Serialize, Clone)]
struct VoicePacket {
    from: u32,
    seq: u16,
    data: Vec<u8>,
}

#[tauri::command]
pub async fn voice_connect(app: AppHandle, state: State<'_, AppState>, address: String, name: String, password: Option<String>, trusted: Option<String>) -> CmdResult<ConnectResult> {
    let (identity, _, _) = state.vault.voice_keys().map_err(err)?;
    voice_disconnect(state.clone()).await?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let outcome = client::connect(
        Connect { address: &address, name: &name, password: password.as_deref().filter(|p| !p.is_empty()), master_identity: &identity, trusted: trusted.as_deref() },
        tx,
    )
    .await
    .map_err(err)?;
    let session = match outcome {
        Outcome::NewServer { fingerprint, name } => return Ok(ConnectResult::NewServer { fingerprint, name }),
        Outcome::Changed { expected, got } => return Ok(ConnectResult::Changed { expected, got }),
        Outcome::Connected(s) => s,
    };
    let result = ConnectResult::Connected { fingerprint: session.fingerprint.clone(), welcome: session.welcome.clone() };
    // Voice going out: straight to the connection
    let (vtx, mut vrx) = mpsc::unbounded_channel::<(u16, Vec<u8>)>();
    *state.voice.voice_out.lock().map_err(err)? = Some(vtx);
    *state.voice.session.lock().await = Some(session);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some((seq, data)) = vrx.recv().await {
            let st = app2.state::<AppState>();
            let guard = st.voice.session.lock().await;
            match guard.as_ref() {
                Some(s) => s.send_voice(seq, &data),
                None => break,
            }
        }
    });
    // Everything coming in goes to the window
    tauri::async_runtime::spawn(async move {
        while let Some(e) = rx.recv().await {
            match e {
                Event::Msg(m) => {
                    let _ = app.emit("voice-event", m);
                }
                Event::Voice { from, seq, data } => {
                    let _ = app.emit("voice-audio", VoicePacket { from, seq, data });
                }
                Event::Closed { reason } => {
                    let st = app.state::<AppState>();
                    st.voice.session.lock().await.take();
                    if let Ok(mut v) = st.voice.voice_out.lock() {
                        v.take();
                    }
                    let _ = app.emit("voice-closed", reason);
                    break;
                }
            }
        }
    });
    Ok(result)
}

#[tauri::command]
pub async fn voice_disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    if let Ok(mut v) = state.voice.voice_out.lock() {
        v.take();
    }
    let s = state.voice.session.lock().await.take();
    if let Some(s) = s {
        s.close().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_send(state: State<'_, AppState>, msg: ClientMsg) -> CmdResult<()> {
    let guard = state.voice.session.lock().await;
    guard.as_ref().ok_or("Not connected.")?.send(msg).map_err(err)
}

/// One Opus frame from the microphone. Sent as raw bytes with the sequence number in a header.
#[tauri::command]
pub fn voice_audio(state: State<'_, AppState>, request: tauri::ipc::Request<'_>) -> CmdResult<()> {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else { return Err("expected bytes".into()) };
    let seq: u16 = request.headers().get("x-seq").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    if let Some(tx) = state.voice.voice_out.lock().map_err(err)?.as_ref() {
        let _ = tx.send((seq, data.clone()));
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_ping(state: State<'_, AppState>) -> CmdResult<u64> {
    let guard = state.voice.session.lock().await;
    Ok(guard.as_ref().map(|s| s.rtt_ms()).unwrap_or(0))
}

// Hosting a server on this PC

#[derive(Serialize, Clone)]
pub struct Hosted {
    port: u16,
    fingerprint: String,
    admin_key: Option<String>,
    name: String,
    /// This PC's address on the local network, for friends in the same house
    lan: Option<String>,
    /// The address friends elsewhere use, when the router opened the port
    public: Option<String>,
    /// "open", "no_router", "refused" or "shared"
    reach: String,
    reach_note: String,
}

async fn open_port(state: &State<'_, AppState>, port: u16, lan: Option<&str>) -> (Option<String>, String, String) {
    use sompio_voice::upnp::{open, Reach};
    let Some(ip) = lan.and_then(|l| l.parse().ok()) else {
        return (None, "no_router".into(), "This PC isn't on a home network Sompio can see.".into());
    };
    let with_port = |ip: std::net::IpAddr| if port == sompio_voice::proto::DEFAULT_PORT { ip.to_string() } else { format!("{ip}:{port}") };
    match open(port, ip).await {
        Reach::Open(o) => {
            let public = with_port(o.external);
            *state.voice.opened.lock().await = Some(o);
            (Some(public), "open".into(), "Your router opened the port. Friends anywhere can join with this address.".into())
        }
        Reach::NoRouter => (None, "no_router".into(), format!("Your router didn't answer (UPnP may be off). Friends on your network can join; for others, turn on UPnP in the router or forward UDP port {port} to this PC.")),
        Reach::Refused { external, reason } => (
            None,
            "refused".into(),
            format!("Your router wouldn't open the port ({reason}). Forward UDP port {port} to this PC in the router's settings{}.", external.map(|e| format!("; friends then use {}", with_port(e))).unwrap_or_default()),
        ),
        Reach::Shared { .. } => (None, "shared".into(), "Your internet provider shares one public address between customers, so no port can be opened from home. Friends on your network can join; for others, run the server on a VPS.".into()),
    }
}

/// Tries the router again (after turning on UPnP, say).
#[tauri::command]
pub async fn voice_host_open_port(state: State<'_, AppState>) -> CmdResult<Hosted> {
    let mut info = state.voice.hosted_info.lock().map_err(err)?.clone().ok_or("No server is running on this PC.")?;
    if let Some(o) = state.voice.opened.lock().await.take() {
        o.close().await;
    }
    let (public, reach, note) = open_port(&state, info.port, info.lan.as_deref()).await;
    info.public = public;
    info.reach = reach;
    info.reach_note = note;
    *state.voice.hosted_info.lock().map_err(err)? = Some(info.clone());
    Ok(info)
}

fn lan_ip() -> Option<String> {
    // Asks the OS which interface it would use; nothing is sent.
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

#[tauri::command]
pub async fn voice_host_start(state: State<'_, AppState>, port: u16, name: String) -> CmdResult<Hosted> {
    state.licences.require(sompio_licence::App::Voice)?;
    let mut hosted = state.voice.hosted.lock().await;
    if hosted.is_some() {
        return Err("A server is already running on this PC.".into());
    }
    let dir = state.vault.data_dir().join("voice-server");
    let r = sompio_voice::server::start(sompio_voice::server::Options { data_dir: dir, port, name, public: true }).await.map_err(err)?;
    let lan = lan_ip();
    let port = r.addr.port();
    let mut info = Hosted { port, fingerprint: r.fingerprint.clone(), admin_key: r.first_admin_key.clone(), name: r.name.clone(), lan: lan.clone(), public: None, reach: String::new(), reach_note: String::new() };
    *hosted = Some(r);
    drop(hosted);
    let (public, reach, note) = open_port(&state, port, lan.as_deref()).await;
    info.public = public;
    info.reach = reach;
    info.reach_note = note;
    *state.voice.hosted_info.lock().map_err(err)? = Some(info.clone());
    Ok(info)
}

#[tauri::command]
pub async fn voice_host_stop(state: State<'_, AppState>) -> CmdResult<()> {
    let r = state.voice.hosted.lock().await.take();
    state.voice.hosted_info.lock().map_err(err)?.take();
    if let Some(o) = state.voice.opened.lock().await.take() {
        o.close().await;
    }
    if let Some(r) = r {
        r.stop().await;
    }
    Ok(())
}

#[tauri::command]
pub fn voice_host_status(state: State<'_, AppState>) -> CmdResult<Option<Hosted>> {
    Ok(state.voice.hosted_info.lock().map_err(err)?.clone())
}

/// Sompio is quitting: close the router port and stop a hosted server.
pub fn on_exit(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    tauri::async_runtime::block_on(async {
        if let Some(o) = state.voice.opened.lock().await.take() {
            o.close().await;
        }
        if let Some(r) = state.voice.hosted.lock().await.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), r.stop()).await;
        }
    });
}
