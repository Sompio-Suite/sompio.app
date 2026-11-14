//! The bridge between the Sompio browser extension and the Vault. Everything stays on this PC:
//!
//!   extension ⇄ (native messaging: the browser starts Sompio.exe in "host" mode) ⇄ local pipe ⇄ Sompio
//!
//! Nothing here opens a network port. The browser only starts the host for the Sompio extension
//! (its id is fixed in the manifest), and the extension has to be paired once: it makes a random
//! key, you approve it in Sompio (the same short code shows on both sides), and every request
//! after that is encrypted and authenticated with that key (AES-256-GCM), with a fresh nonce
//! and a time stamp so an old message can't be replayed. Paired keys live inside the encrypted
//! vault, so while Sompio is locked nothing can be asked at all.

use crate::{AppState, CmdResult};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sompio_licence::App;
use sompio_vault::browser::BrowserClient;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub use sompio_browser_host::{is_host_launch, pipe_name, run as run_host, FIREFOX_ID, HOST_NAME, MAX_FRAME};
/// Chrome, Edge, Brave and other Chromium browsers: fixed by the "key" in the extension's manifest
pub const CHROME_ID: &str = "phmimpfbhcneblnnojaialplfglbkoke";
/// A request older (or newer) than this is refused, so a captured one can't be sent again later
const MAX_SKEW_MS: i64 = 120_000;

#[derive(Default)]
pub struct Bridge {
    /// Pairing requests waiting for a yes or no in Sompio, by client id
    pending: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    /// Nonces seen recently, so each request is answered once
    seen: Mutex<VecDeque<(String, i64)>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The short code shown in both the extension and Sompio while pairing, so you can see it's the same request.
pub fn pair_code(key_b64: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let digest = Sha256::digest(key_b64.as_bytes());
    let chars: String = digest.iter().take(6).map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char).collect();
    format!("{}-{}", &chars[..3], &chars[3..])
}

fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().passwords.settings.lock().map(|s| s.browser).unwrap_or(false)
}

fn unlocked(app: &AppHandle) -> bool {
    app.state::<AppState>().vault.vault_db().is_ok()
}

fn key_of(client: &BrowserClient) -> Result<[u8; 32], String> {
    let k = B64.decode(&client.key).map_err(|_| "bad key")?;
    k.try_into().map_err(|_| "bad key".to_string())
}

fn seal(key: &[u8; 32], aad: &str, plain: &[u8]) -> (String, String) {
    let cipher = Aes256Gcm::new(key.into());
    let mut iv = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut iv);
    let data = cipher.encrypt(Nonce::from_slice(&iv), Payload { msg: plain, aad: aad.as_bytes() }).expect("encrypting in memory");
    (B64.encode(iv), B64.encode(data))
}

fn open(key: &[u8; 32], aad: &str, iv: &str, data: &str) -> Option<Vec<u8>> {
    let iv = B64.decode(iv).ok().filter(|v| v.len() == 12)?;
    let data = B64.decode(data).ok()?;
    Aes256Gcm::new(key.into()).decrypt(Nonce::from_slice(&iv), Payload { msg: &data, aad: aad.as_bytes() }).ok()
}

fn error(code: &str, text: &str) -> Value {
    json!({ "type": "error", "code": code, "error": text })
}

/// Answers one message from the extension.
pub async fn handle(app: &AppHandle, msg: Value) -> Value {
    let mut reply = match msg.get("type").and_then(Value::as_str).unwrap_or("") {
        "status" => json!({
            "type": "status", "running": true, "enabled": enabled(app), "locked": !unlocked(app),
            "version": app.package_info().version.to_string(),
        }),
        "show" => {
            crate::calendar::show_main(app);
            json!({ "type": "ok" })
        }
        "pair" => pair(app, &msg).await,
        "call" => call(app, &msg).await,
        _ => error("unknown", "Sompio didn't understand that request."),
    };
    if let Some(rid) = msg.get("rid") {
        reply["rid"] = rid.clone();
    }
    reply
}

#[derive(Clone, Serialize)]
struct PairAsk {
    id: String,
    browser: String,
    code: String,
}

async fn pair(app: &AppHandle, msg: &Value) -> Value {
    if !enabled(app) {
        return error("off", "Turn on the browser extension in Sompio first: Vault, Settings.");
    }
    if !unlocked(app) {
        crate::calendar::show_main(app);
        return error("locked", "Unlock Sompio, then connect again.");
    }
    let id = msg.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let key = msg.get("key").and_then(Value::as_str).unwrap_or("").to_string();
    let browser: String = msg.get("browser").and_then(Value::as_str).unwrap_or("Browser").chars().filter(|c| c.is_alphanumeric() || *c == ' ').take(30).collect();
    if !(16..=64).contains(&id.len()) || !id.chars().all(|c| c.is_ascii_hexdigit()) || B64.decode(&key).map(|k| k.len()) != Ok(32) {
        return error("bad", "That pairing request isn't valid.");
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let state = app.state::<AppState>();
    state.browser.pending.lock().expect("pending").insert(id.clone(), tx);
    let ask = PairAsk { id: id.clone(), browser: if browser.trim().is_empty() { "Browser".into() } else { browser.clone() }, code: pair_code(&key) };
    crate::calendar::show_main(app);
    let _ = app.emit("browser-pair", ask.clone());
    let answer = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
    state.browser.pending.lock().expect("pending").remove(&id);
    let _ = app.emit("browser-pair-done", id.clone());
    match answer {
        Ok(Ok(true)) => {}
        Ok(_) => return error("denied", "Sompio didn't allow this browser."),
        Err(_) => return error("timeout", "No answer in Sompio. Try again."),
    }
    let now = now_ms();
    let client = BrowserClient { id: id.clone(), key, browser: ask.browser, added: now, last_used: now };
    let saved = crate::vault::with_vault(app, move |v| {
        let mut all = v.browser_clients();
        all.retain(|c| c.id != client.id);
        all.push(client);
        v.set_browser_clients(&all)
    })
    .await;
    match saved {
        Ok(()) => json!({ "type": "paired", "ok": true }),
        Err(e) => error("failed", &e),
    }
}

/// Sompio's answer to a pairing question.
#[tauri::command]
pub fn browser_answer(state: State<'_, AppState>, id: String, allow: bool) -> CmdResult<()> {
    if let Some(tx) = state.browser.pending.lock().map_err(|e| e.to_string())?.remove(&id) {
        let _ = tx.send(allow);
    }
    Ok(())
}

async fn call(app: &AppHandle, msg: &Value) -> Value {
    if !enabled(app) {
        return error("off", "The browser extension is turned off in Sompio.");
    }
    if !unlocked(app) {
        return error("locked", "Sompio is locked.");
    }
    let id = msg.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let lookup = id.clone();
    let client = match crate::vault::with_vault(app, move |v| Ok(v.browser_clients().into_iter().find(|c| c.id == lookup))).await {
        Ok(Some(c)) => c,
        Ok(None) => return error("not_paired", "This browser isn't paired with Sompio."),
        Err(e) => return error("locked", &e),
    };
    let Ok(key) = key_of(&client) else { return error("not_paired", "This browser's pairing is damaged. Pair it again.") };
    let (iv, data) = (msg.get("iv").and_then(Value::as_str).unwrap_or(""), msg.get("data").and_then(Value::as_str).unwrap_or(""));
    let Some(plain) = open(&key, &format!("sompio-req:{id}"), iv, data) else {
        return error("not_paired", "This browser's pairing no longer matches. Pair it again.");
    };
    let Ok(req) = serde_json::from_slice::<Value>(&plain) else { return error("bad", "Unreadable request.") };
    let nonce = req.get("n").and_then(Value::as_str).unwrap_or("").to_string();
    let ts = req.get("t").and_then(Value::as_i64).unwrap_or(0);
    let now = now_ms();
    if nonce.len() < 16 || (now - ts).abs() > MAX_SKEW_MS {
        return error("stale", "That request is too old. Check your PC's clock.");
    }
    {
        let state = app.state::<AppState>();
        let mut seen = state.browser.seen.lock().expect("seen");
        while seen.front().is_some_and(|(_, t)| now - t > 2 * MAX_SKEW_MS) {
            seen.pop_front();
        }
        if seen.iter().any(|(n, _)| *n == nonce) {
            return error("stale", "That request was already answered.");
        }
        seen.push_back((nonce.clone(), now));
    }
    let method = req.get("m").and_then(Value::as_str).unwrap_or("").to_string();
    let args = req.get("a").cloned().unwrap_or(Value::Null);
    let body = match run(app, &method, &args).await {
        Ok(r) => json!({ "n": nonce, "ok": true, "r": r }),
        Err(e) => json!({ "n": nonce, "ok": false, "error": e }),
    };
    // Remember when it was last used, at most once an hour (each save rewrites the vault)
    if now - client.last_used > 3_600_000 {
        let cid = client.id.clone();
        let _ = crate::vault::with_vault(app, move |v| {
            let mut all = v.browser_clients();
            all.iter_mut().filter(|c| c.id == cid).for_each(|c| c.last_used = now);
            v.set_browser_clients(&all)
        })
        .await;
    }
    let (iv, data) = seal(&key, &format!("sompio-res:{id}"), body.to_string().as_bytes());
    json!({ "type": "reply", "iv": iv, "data": data })
}

fn arg(a: &Value, k: &str) -> String {
    a.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn summaries(v: &sompio_vault::Vault, list: Vec<sompio_vault::EntrySummary>) -> Value {
    let icons = v.icons();
    Value::Array(
        list.into_iter()
            .filter(|s| s.has_password || !s.username.is_empty())
            .map(|s| {
                let icon = s.icon.as_ref().and_then(|i| icons.get(i)).cloned();
                json!({ "id": s.id, "title": s.title, "username": s.username, "url": s.url, "totp": s.has_totp, "icon": icon })
            })
            .collect(),
    )
}

/// The things a paired extension may ask. Filling always checks the entry belongs to the page's
/// website, using the address the browser itself reports for the tab, not what the page says.
async fn run(app: &AppHandle, method: &str, a: &Value) -> Result<Value, String> {
    let url = arg(a, "url");
    let id = arg(a, "id");
    match method {
        "logins" => crate::vault::with_vault(app, move |v| Ok(summaries(v, v.logins_for(&url)))).await,
        "search" => {
            let q = arg(a, "q");
            crate::vault::with_vault(app, move |v| Ok(summaries(v, v.entries(None, &q)?.into_iter().take(40).collect()))).await
        }
        "fill" => {
            let any = a.get("anySite").and_then(Value::as_bool).unwrap_or(false);
            crate::vault::with_vault(app, move |v| {
                let s = if any { v.entry(&id)?.summary } else { v.check_site(&id, &url)? };
                let password = v.reveal(&id, "Password")?;
                let totp = v.totp(&id)?.map(|t| t.code);
                Ok(json!({ "username": s.username, "password": password.as_str(), "totp": totp }))
            })
            .await
        }
        "totp" => crate::vault::with_vault(app, move |v| {
            v.check_site(&id, &url)?;
            Ok(json!(v.totp(&id)?.map(|t| json!({ "code": t.code, "remaining": t.remaining }))))
        })
        .await,
        "save" => {
            app.state::<AppState>().licences.require(App::Vault)?;
            let (title, username, password) = (arg(a, "title"), arg(a, "username"), arg(a, "password"));
            let existing = (!id.is_empty()).then_some(id);
            let saved = crate::vault::with_vault(app, move |v| v.save_from_browser(existing.as_deref(), &url, &title, &username, &password)).await?;
            let _ = app.emit("vault-changed", ());
            Ok(json!({ "id": saved }))
        }
        "generate" => {
            let pw = sompio_vault::generator::password(&Default::default()).map_err(|e| e.to_string())?;
            Ok(json!({ "password": pw }))
        }
        _ => Err("Sompio didn't understand that request.".into()),
    }
}

// ---------- Paired browsers, for the Vault settings ----------

#[derive(Serialize)]
pub struct ClientView {
    id: String,
    browser: String,
    added: i64,
    last_used: i64,
}

#[tauri::command]
pub async fn browser_clients(app: AppHandle) -> CmdResult<Vec<ClientView>> {
    crate::vault::with_vault(&app, |v| {
        Ok(v.browser_clients().into_iter().map(|c| ClientView { id: c.id, browser: c.browser, added: c.added, last_used: c.last_used }).collect())
    })
    .await
}

#[tauri::command]
pub async fn browser_forget(app: AppHandle, id: String) -> CmdResult<()> {
    crate::vault::with_vault(&app, move |v| {
        let mut all = v.browser_clients();
        all.retain(|c| c.id != id);
        v.set_browser_clients(&all)
    })
    .await
}

/// The browsers the setup can open, with the page where an extension is added by hand.
const BROWSERS: &[(&str, &str, &str)] = &[
    ("chrome", "chrome.exe", "chrome://extensions"),
    ("edge", "msedge.exe", "edge://extensions"),
    ("brave", "brave.exe", "brave://extensions"),
    ("firefox", "firefox.exe", "about:debugging#/runtime/this-firefox"),
];

/// Where a browser is installed, from Windows' list of installed programs ("App Paths").
#[cfg(windows)]
fn browser_exe(exe: &str) -> Option<std::path::PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(k) = RegKey::predef(hive).open_subkey(format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe}")) {
            if let Ok(p) = k.get_value::<String, _>("") {
                let p = std::path::PathBuf::from(p.trim_matches('"'));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Where a browser is installed on a Mac: the program inside its .app, in /Applications or ~/Applications.
#[cfg(target_os = "macos")]
fn browser_exe(exe: &str) -> Option<std::path::PathBuf> {
    let (app, bin) = match exe {
        "chrome.exe" => ("Google Chrome.app", "Google Chrome"),
        "msedge.exe" => ("Microsoft Edge.app", "Microsoft Edge"),
        "brave.exe" => ("Brave Browser.app", "Brave Browser"),
        "firefox.exe" => ("Firefox.app", "firefox"),
        _ => return None,
    };
    let home = std::env::var("HOME").unwrap_or_default();
    ["/Applications".to_string(), format!("{home}/Applications")]
        .iter()
        .map(|d| std::path::Path::new(d).join(app).join("Contents/MacOS").join(bin))
        .find(|p| p.exists())
}

/// Where a browser is installed on Linux: the first of its usual command names on the PATH.
#[cfg(all(unix, not(target_os = "macos")))]
fn browser_exe(exe: &str) -> Option<std::path::PathBuf> {
    let names: &[&str] = match exe {
        "chrome.exe" => &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"],
        "msedge.exe" => &["microsoft-edge", "microsoft-edge-stable"],
        "brave.exe" => &["brave-browser", "brave"],
        "firefox.exe" => &["firefox", "firefox-esr"],
        _ => return None,
    };
    let path = std::env::var_os("PATH")?;
    names.iter().find_map(|n| std::env::split_paths(&path).map(|d| d.join(n)).find(|p| p.exists()))
}

/// Which of Chrome, Edge, Brave and Firefox are installed.
#[tauri::command]
pub fn browser_installed() -> Vec<String> {
    BROWSERS.iter().filter(|(_, exe, _)| browser_exe(exe).is_some()).map(|(k, _, _)| k.to_string()).collect()
}

/// Opens a browser on its extensions page, where the Sompio extension is added.
#[tauri::command]
pub fn browser_open_extensions_page(browser: String) -> CmdResult<()> {
    let (_, exe, page) = BROWSERS.iter().find(|(k, _, _)| *k == browser).ok_or("Unknown browser.")?;
    let path = browser_exe(exe).ok_or("That browser doesn't seem to be installed.")?;
    std::process::Command::new(path).arg(page).spawn().map(|_| ()).map_err(|e| format!("Couldn't open the browser: {e}"))
}

/// Opens the extension's folder in File Explorer.
#[tauri::command]
pub fn browser_open_extension_folder(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = browser_extension_folder(app.clone())?;
    app.opener().open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

/// Where the extension's files are, so testers can load it into their browser.
#[tauri::command]
pub fn browser_extension_folder(app: AppHandle) -> CmdResult<String> {
    let dir = app.path().resource_dir().map_err(|e| e.to_string())?.join("extension");
    if !dir.exists() {
        return Err("The extension's files weren't found next to Sompio. Reinstall Sompio.".into());
    }
    Ok(dir.to_string_lossy().trim_start_matches(r"\\?\").to_string())
}

// ---------- The local pipe ----------

async fn serve_stream<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(app: AppHandle, mut s: S) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    loop {
        let mut len = [0u8; 4];
        if s.read_exact(&mut len).await.is_err() {
            return;
        }
        let n = u32::from_le_bytes(len) as usize;
        if n > MAX_FRAME {
            return;
        }
        let mut buf = vec![0u8; n];
        if s.read_exact(&mut buf).await.is_err() {
            return;
        }
        let reply = match serde_json::from_slice::<Value>(&buf) {
            Ok(msg) => handle(&app, msg).await,
            Err(_) => error("bad", "Unreadable message."),
        };
        let out = reply.to_string().into_bytes();
        if s.write_all(&(out.len() as u32).to_le_bytes()).await.is_err() || s.write_all(&out).await.is_err() {
            return;
        }
    }
}

/// Listens for the browser host on a pipe only this PC can reach. Started once, at launch.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = listen(app).await {
            eprintln!("Browser bridge: {e}");
        }
    });
}

#[cfg(windows)]
async fn listen(app: AppHandle) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = pipe_name();
    let mut server = ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create(&name)?;
    loop {
        server.connect().await?;
        let connected = server;
        server = ServerOptions::new().reject_remote_clients(true).create(&name)?;
        tauri::async_runtime::spawn(serve_stream(app.clone(), connected));
    }
}

#[cfg(unix)]
async fn listen(app: AppHandle) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let name = pipe_name();
    let _ = std::fs::remove_file(&name);
    let listener = tokio::net::UnixListener::bind(&name)?;
    std::fs::set_permissions(&name, std::fs::Permissions::from_mode(0o600))?;
    loop {
        let (s, _) = listener.accept().await?;
        tauri::async_runtime::spawn(serve_stream(app.clone(), s));
    }
}

// ---------- Telling browsers where the host is ----------

fn manifest(path: &str, firefox: bool) -> Value {
    let mut m = json!({
        "name": HOST_NAME,
        "description": "Sompio Vault for your browser. Talks only to Sompio on this PC.",
        "path": path,
        "type": "stdio",
    });
    if firefox {
        m["allowed_extensions"] = json!([FIREFOX_ID]);
    } else {
        m["allowed_origins"] = json!([format!("chrome-extension://{CHROME_ID}/")]);
    }
    m
}

/// Registers (or removes) the host for Chrome, Edge, Brave, Chromium and Firefox, for this user only.
/// Run when the setting changes and at every start, so an update that moved Sompio still works.
pub fn register(app: &AppHandle, on: bool) -> anyhow::Result<()> {
    let dir = app.path().app_local_data_dir()?.join("browser");
    // The small console helper installed next to Sompio; a local build without it uses Sompio itself
    let me = std::env::current_exe()?;
    let helper = me.with_file_name(if cfg!(windows) { "sompio-browser.exe" } else { "sompio-browser" });
    let exe = if helper.exists() { helper } else { me };
    let exe = exe.to_string_lossy().trim_start_matches(r"\\?\").to_string();
    platform::register(&dir, &exe, on)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const CHROMIUM_KEYS: &[&str] = &[
        r"Software\Google\Chrome\NativeMessagingHosts",
        r"Software\Microsoft\Edge\NativeMessagingHosts",
        r"Software\Chromium\NativeMessagingHosts",
        r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts",
        r"Software\Vivaldi\NativeMessagingHosts",
    ];
    const FIREFOX_KEY: &str = r"Software\Mozilla\NativeMessagingHosts";

    pub fn register(dir: &std::path::Path, exe: &str, on: bool) -> anyhow::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let chrome = dir.join(format!("{HOST_NAME}.json"));
        let firefox = dir.join(format!("{HOST_NAME}.firefox.json"));
        if on {
            std::fs::create_dir_all(dir)?;
            std::fs::write(&chrome, serde_json::to_string_pretty(&manifest(exe, false))?)?;
            std::fs::write(&firefox, serde_json::to_string_pretty(&manifest(exe, true))?)?;
            for k in CHROMIUM_KEYS {
                let (key, _) = hkcu.create_subkey(format!(r"{k}\{HOST_NAME}"))?;
                key.set_value("", &chrome.to_string_lossy().to_string())?;
            }
            let (key, _) = hkcu.create_subkey(format!(r"{FIREFOX_KEY}\{HOST_NAME}"))?;
            key.set_value("", &firefox.to_string_lossy().to_string())?;
        } else {
            for k in CHROMIUM_KEYS.iter().chain([FIREFOX_KEY].iter()) {
                let _ = hkcu.delete_subkey_all(format!(r"{k}\{HOST_NAME}"));
            }
            let _ = std::fs::remove_file(chrome);
            let _ = std::fs::remove_file(firefox);
        }
        Ok(())
    }
}

#[cfg(unix)]
mod platform {
    use super::*;

    /// Each browser's folder for native messaging hosts, for this user only.
    #[cfg(target_os = "macos")]
    const CHROMIUM: &[&str] = &[
        "Library/Application Support/Google/Chrome/NativeMessagingHosts",
        "Library/Application Support/Chromium/NativeMessagingHosts",
        "Library/Application Support/Microsoft Edge/NativeMessagingHosts",
        "Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts",
        "Library/Application Support/Vivaldi/NativeMessagingHosts",
    ];
    #[cfg(target_os = "macos")]
    const FIREFOX: &str = "Library/Application Support/Mozilla/NativeMessagingHosts";

    #[cfg(not(target_os = "macos"))]
    const CHROMIUM: &[&str] = &[
        ".config/google-chrome/NativeMessagingHosts",
        ".config/chromium/NativeMessagingHosts",
        ".config/microsoft-edge/NativeMessagingHosts",
        ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
        ".config/vivaldi/NativeMessagingHosts",
    ];
    #[cfg(not(target_os = "macos"))]
    const FIREFOX: &str = ".mozilla/native-messaging-hosts";

    pub fn register(_dir: &std::path::Path, exe: &str, on: bool) -> anyhow::Result<()> {
        let home = std::path::PathBuf::from(std::env::var("HOME")?);
        for (d, firefox) in CHROMIUM.iter().map(|d| (home.join(d), false)).chain([(home.join(FIREFOX), true)]) {
            let file = d.join(format!("{HOST_NAME}.json"));
            if on {
                // Only for browsers that are there: their settings folder already exists
                let browser_dir = d.parent().map(|p| p.exists()).unwrap_or(false);
                if !browser_dir {
                    continue;
                }
                std::fs::create_dir_all(&d)?;
                std::fs::write(&file, serde_json::to_string_pretty(&manifest(exe, firefox))?)?;
            } else {
                let _ = std::fs::remove_file(file);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_messages_open_only_with_the_right_key_and_direction() {
        let key = [9u8; 32];
        let (iv, data) = seal(&key, "sompio-res:abc", b"{\"ok\":true}");
        assert_eq!(open(&key, "sompio-res:abc", &iv, &data).as_deref(), Some(&b"{\"ok\":true}"[..]));
        assert!(open(&key, "sompio-req:abc", &iv, &data).is_none(), "a reply can't pass as a request");
        assert!(open(&[8u8; 32], "sompio-res:abc", &iv, &data).is_none());
        assert_eq!(pair_code("AAAA").len(), 7);
    }

    /// Made by the extension's code (WebCrypto) in Node, so both sides agree on the format.
    #[test]
    fn opens_what_the_extension_sealed() {
        let key: [u8; 32] = B64.decode("AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=").unwrap().try_into().unwrap();
        let plain = open(&key, "sompio-req:00112233445566778899aabbccddeeff", include_str!("../tests/ext_iv.txt").trim(), include_str!("../tests/ext_data.txt").trim()).expect("opens");
        assert_eq!(String::from_utf8(plain).unwrap(), include_str!("../tests/ext_plain.txt").trim());
        assert_eq!(pair_code("AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA="), include_str!("../tests/ext_code.txt").trim());
    }
}
