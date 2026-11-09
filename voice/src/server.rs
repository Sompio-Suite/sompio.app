//! The voice server. Run it on a VPS or a PC; people connect by IP or domain name. It keeps
//! the channel tree, roles and bans in one JSON file next to its certificate.

use crate::proto::*;
use anyhow::{anyhow, bail, Context, Result};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredChannel {
    #[serde(flatten)]
    channel: Channel,
    password_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ban {
    key: String,
    until_ms: i64,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrivilegeKey {
    key: String,
    role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    name: String,
    welcome: String,
    password_hash: Option<String>,
    max_clients: u32,
    default_channel: u32,
    next_channel: u32,
    channels: Vec<StoredChannel>,
    /// Public key (hex) to role, for everyone above Member
    roles: HashMap<String, Role>,
    bans: Vec<Ban>,
    privilege_keys: Vec<PrivilegeKey>,
}

impl Stored {
    fn new(name: &str) -> Self {
        let ch = |id, name: &str, order| StoredChannel {
            channel: Channel { id, parent: None, name: name.into(), topic: String::new(), has_password: false, permanent: true, max_users: 0, order },
            password_hash: None,
        };
        Stored {
            name: name.into(),
            welcome: "Welcome! Be kind.".into(),
            password_hash: None,
            max_clients: 64,
            default_channel: 1,
            next_channel: 4,
            channels: vec![ch(1, "Lobby", 0), ch(2, "Gaming", 1), ch(3, "AFK", 2)],
            roles: HashMap::new(),
            bans: Vec::new(),
            privilege_keys: Vec::new(),
        }
    }
    fn info(&self) -> ServerInfo {
        ServerInfo { name: self.name.clone(), welcome: self.welcome.clone(), max_clients: self.max_clients, default_channel: self.default_channel }
    }
}

struct Conn {
    info: Client,
    key: String,
    tx: mpsc::UnboundedSender<ServerMsg>,
    conn: quinn::Connection,
}

struct Shared {
    stored: Stored,
    clients: HashMap<u32, Conn>,
    next_client: u32,
    path: PathBuf,
    fingerprint: String,
}

type State = Arc<Mutex<Shared>>;

pub struct Options {
    pub data_dir: PathBuf,
    pub port: u16,
    /// Name for a new server; an existing one keeps its own
    pub name: String,
    /// Listen on all addresses (a real server) or only this PC (tests)
    pub public: bool,
}

pub struct Running {
    pub addr: SocketAddr,
    pub fingerprint: String,
    /// Made on the very first start: whoever uses it becomes admin
    pub first_admin_key: Option<String>,
    pub name: String,
    stop: watch::Sender<bool>,
    endpoint: quinn::Endpoint,
}

impl Running {
    pub async fn stop(self) {
        let _ = self.stop.send(true);
        self.endpoint.close(0u32.into(), b"server stopping");
        self.endpoint.wait_idle().await;
    }
}

fn hash_password(p: &str) -> Result<String> {
    Ok(argon2::Argon2::default().hash_password(p.as_bytes(), &SaltString::generate(&mut OsRng)).map_err(|e| anyhow!("{e}"))?.to_string())
}

fn password_ok(hash: &str, given: &str) -> bool {
    PasswordHash::new(hash).map(|h| argon2::Argon2::default().verify_password(given.as_bytes(), &h).is_ok()).unwrap_or(false)
}

fn random_key() -> String {
    let b: [u8; 20] = rand::random();
    const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let s: String = b.iter().map(|x| A[(*x as usize) % A.len()] as char).collect();
    format!("{}-{}-{}-{}-{}", &s[0..4], &s[4..8], &s[8..12], &s[12..16], &s[16..20])
}

fn save(s: &Shared) {
    let mut stored = s.stored.clone();
    stored.channels.retain(|c| c.channel.permanent);
    if let Ok(json) = serde_json::to_vec_pretty(&stored) {
        let tmp = s.path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &s.path);
        }
    }
}

fn load_or_make_cert(dir: &Path) -> Result<(rustls::pki_types::CertificateDer<'static>, rustls::pki_types::PrivateKeyDer<'static>)> {
    let (cp, kp) = (dir.join("cert.der"), dir.join("key.der"));
    if cp.exists() && kp.exists() {
        let cert = std::fs::read(&cp)?;
        let key = std::fs::read(&kp)?;
        return Ok((cert.into(), rustls::pki_types::PrivateKeyDer::Pkcs8(key.into())));
    }
    let made = rcgen::generate_simple_self_signed(vec!["sompio-voice".into()])?;
    let cert = made.cert.der().to_vec();
    let key = made.key_pair.serialize_der();
    std::fs::write(&cp, &cert)?;
    std::fs::write(&kp, &key)?;
    Ok((cert.into(), rustls::pki_types::PrivateKeyDer::Pkcs8(key.into())))
}

pub fn transport() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
    t.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().expect("valid")));
    t.datagram_receive_buffer_size(Some(256 * 1024));
    t
}

pub async fn start(opts: Options) -> Result<Running> {
    std::fs::create_dir_all(&opts.data_dir).with_context(|| format!("Couldn't make {}", opts.data_dir.display()))?;
    let (cert, key) = load_or_make_cert(&opts.data_dir)?;
    let fp = fingerprint(&cert);
    let path = opts.data_dir.join("server.json");
    let mut first_admin_key = None;
    let stored = match std::fs::read(&path) {
        Ok(b) => serde_json::from_slice(&b).context("server.json is damaged")?,
        Err(_) => {
            let mut s = Stored::new(&opts.name);
            let k = random_key();
            s.privilege_keys.push(PrivilegeKey { key: k.clone(), role: Role::Admin });
            first_admin_key = Some(k);
            s
        }
    };
    let name = stored.name.clone();
    let shared = Shared { stored, clients: HashMap::new(), next_client: 1, path, fingerprint: fp.clone() };
    save(&shared);

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut sc = quinn::ServerConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicServerConfig::try_from(tls)?));
    sc.transport_config(Arc::new(transport()));
    let bind: SocketAddr = if opts.public { format!("[::]:{}", opts.port).parse()? } else { format!("127.0.0.1:{}", opts.port).parse()? };
    let endpoint = match quinn::Endpoint::server(sc.clone(), bind) {
        Ok(e) => e,
        // Some systems have IPv6 turned off
        Err(_) if opts.public => quinn::Endpoint::server(sc, format!("0.0.0.0:{}", opts.port).parse()?)?,
        Err(e) => return Err(e).with_context(|| format!("Port {} is in use or not allowed", opts.port)),
    };
    let addr = endpoint.local_addr()?;
    let state: State = Arc::new(Mutex::new(shared));
    let (stop, mut stopped) = watch::channel(false);
    let ep = endpoint.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stopped.changed() => break,
                incoming = ep.accept() => {
                    let Some(incoming) = incoming else { break };
                    let st = state.clone();
                    tokio::spawn(async move {
                        if let Ok(conn) = incoming.await {
                            let _ = handle(st, conn).await;
                        }
                    });
                }
            }
        }
    });
    Ok(Running { addr, fingerprint: fp, first_admin_key, name, stop, endpoint })
}

fn broadcast(s: &Shared, msg: &ServerMsg) {
    for c in s.clients.values() {
        let _ = c.tx.send(msg.clone());
    }
}

fn channel_mut(s: &mut Shared, id: u32) -> Option<&mut StoredChannel> {
    s.stored.channels.iter_mut().find(|c| c.channel.id == id)
}

/// Temporary channels close when the last person leaves (unless they have sub-channels in use).
fn tidy_channels(s: &mut Shared) {
    loop {
        let used: Vec<u32> = s.clients.values().map(|c| c.info.channel).collect();
        let parents: Vec<u32> = s.stored.channels.iter().filter_map(|c| c.channel.parent).collect();
        let gone: Vec<u32> = s
            .stored
            .channels
            .iter()
            .filter(|c| !c.channel.permanent && !used.contains(&c.channel.id) && !parents.contains(&c.channel.id))
            .map(|c| c.channel.id)
            .collect();
        if gone.is_empty() {
            return;
        }
        s.stored.channels.retain(|c| !gone.contains(&c.channel.id));
        for id in gone {
            broadcast(s, &ServerMsg::ChannelDeleted { id });
        }
    }
}

async fn handle(state: State, conn: quinn::Connection) -> Result<()> {
    let (mut send, mut recv) = conn.accept_bi().await?;
    let nonce = hex::encode(rand::random::<[u8; 24]>());
    let (server_name, needs_password, fp) = {
        let s = state.lock().map_err(|_| anyhow!("state"))?;
        (s.stored.name.clone(), s.stored.password_hash.is_some(), s.fingerprint.clone())
    };
    write_msg(&mut send, &ServerMsg::Challenge { version: VERSION, nonce: nonce.clone(), name: server_name, needs_password }).await?;
    let hello: ClientMsg = loop {
        let m: ClientMsg = tokio::time::timeout(std::time::Duration::from_secs(20), read_msg(&mut recv)).await??;
        if m != ClientMsg::Ping {
            break m;
        }
    };
    let ClientMsg::Hello { version, name, public_key, signature, password } = hello else { bail!("expected hello") };

    let refuse = |m: &str| ServerMsg::Error { message: m.to_string() };
    let check = || -> std::result::Result<(), String> {
        if version != VERSION {
            return Err("This server runs a different version of Sompio Voice. Update Sompio.".into());
        }
        let key_bytes: [u8; 32] = hex::decode(&public_key).ok().and_then(|b| b.try_into().ok()).ok_or("Bad identity")?;
        let sig: [u8; 64] = hex::decode(&signature).ok().and_then(|b| b.try_into().ok()).ok_or("Bad identity")?;
        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| "Bad identity")?
            .verify(&hello_payload(&nonce, &fp), &Signature::from_bytes(&sig))
            .map_err(|_| "Identity check failed")?;
        Ok(())
    };
    if let Err(m) = check() {
        write_msg(&mut send, &refuse(&m)).await?;
        send.finish()?;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        return Ok(());
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let joined: std::result::Result<u32, String> = (|| {
        let mut s = state.lock().map_err(|_| "Server error".to_string())?;
        if let Some(h) = &s.stored.password_hash {
            if !password.as_deref().is_some_and(|p| password_ok(h, p)) {
                return Err("Wrong server password.".to_string());
            }
        }
        let now = now_ms();
        s.stored.bans.retain(|b| b.until_ms > now);
        if let Some(b) = s.stored.bans.iter().find(|b| b.key == public_key) {
            return Err(format!("You're banned from this server: {}", if b.reason.is_empty() { "no reason given" } else { &b.reason }));
        }
        if s.clients.len() as u32 >= s.stored.max_clients {
            return Err("The server is full.".to_string());
        }
        let mut nick: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if nick.is_empty() {
            nick = "Guest".into();
        }
        let base = nick.clone();
        let mut n = 2;
        while s.clients.values().any(|c| c.info.name.eq_ignore_ascii_case(&nick)) {
            nick = format!("{base} ({n})");
            n += 1;
        }
        let me = s.next_client;
        s.next_client += 1;
        let role = s.stored.roles.get(&public_key).copied().unwrap_or(Role::Member);
        let info = Client { id: me, name: nick, channel: s.stored.default_channel, role, muted: false, deafened: false, away: None, identity: short_identity(&public_key) };
        broadcast(&s, &ServerMsg::ClientJoined { client: info.clone() });
        s.clients.insert(me, Conn { info, key: public_key.clone(), tx: tx.clone(), conn: conn.clone() });
        let welcome = ServerMsg::Welcome {
            you: me,
            role,
            server: s.stored.info(),
            channels: s.stored.channels.iter().map(|c| c.channel.clone()).collect(),
            clients: s.clients.values().map(|c| c.info.clone()).collect(),
        };
        let _ = tx.send(welcome);
        Ok(me)
    })();
    let me = match joined {
        Ok(me) => me,
        Err(m) => {
            write_msg(&mut send, &refuse(&m)).await?;
            send.finish()?;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            return Ok(());
        }
    };

    // Writer: everything for this person goes out in order on the control stream
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            let kicked = matches!(m, ServerMsg::Kicked { .. });
            if write_msg(&mut send, &m).await.is_err() {
                break;
            }
            if kicked {
                let _ = send.finish();
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                break;
            }
        }
    });

    // Voice: relay to everyone else in the same channel
    let vstate = state.clone();
    let vconn = conn.clone();
    let voice = tokio::spawn(async move {
        while let Ok(d) = vconn.read_datagram().await {
            if d.len() < 3 || d.len() > 1400 {
                continue;
            }
            let targets: Vec<quinn::Connection> = {
                let Ok(s) = vstate.lock() else { break };
                let Some(sender) = s.clients.get(&me) else { break };
                if sender.info.muted {
                    continue;
                }
                s.clients.values().filter(|c| c.info.id != me && c.info.channel == sender.info.channel && !c.info.deafened).map(|c| c.conn.clone()).collect()
            };
            let packet = bytes::Bytes::from(voice_relay(me, &d));
            for t in targets {
                let _ = t.send_datagram(packet.clone());
            }
        }
    });

    let result = control_loop(&state, me, &mut recv).await;
    let reason = match &result {
        Ok(()) => "left".to_string(),
        Err(e) if e.to_string().contains("kicked") => e.to_string(),
        Err(_) => "lost connection".to_string(),
    };
    {
        if let Ok(mut s) = state.lock() {
            if s.clients.remove(&me).is_some() {
                broadcast(&s, &ServerMsg::ClientLeft { id: me, reason });
                tidy_channels(&mut s);
            }
        }
    }
    voice.abort();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), writer).await;
    conn.close(0u32.into(), b"bye");
    Ok(())
}

async fn control_loop(state: &State, me: u32, recv: &mut quinn::RecvStream) -> Result<()> {
    loop {
        let msg: ClientMsg = match read_msg(recv).await {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        let mut s = state.lock().map_err(|_| anyhow!("state"))?;
        if !s.clients.contains_key(&me) {
            bail!("kicked");
        }
        if let Err(e) = apply(&mut s, me, msg) {
            if let Some(c) = s.clients.get(&me) {
                let _ = c.tx.send(ServerMsg::Error { message: e.to_string() });
            }
        }
    }
}

fn role_of(s: &Shared, id: u32) -> Role {
    s.clients.get(&id).map(|c| c.info.role).unwrap_or(Role::Member)
}

fn need(s: &Shared, me: u32, r: Role) -> Result<()> {
    if role_of(s, me) < r {
        bail!("You need to be {} for that.", if r == Role::Admin { "an admin" } else { "a moderator or admin" });
    }
    Ok(())
}

fn clean_text(t: &str, max: usize) -> String {
    t.trim().chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect()
}

fn update_client(s: &mut Shared, id: u32, f: impl FnOnce(&mut Client)) {
    if let Some(c) = s.clients.get_mut(&id) {
        f(&mut c.info);
        let msg = ServerMsg::ClientUpdated { client: c.info.clone() };
        broadcast(s, &msg);
    }
}

fn remove_client(s: &mut Shared, id: u32, reason: &str) {
    if let Some(c) = s.clients.remove(&id) {
        let _ = c.tx.send(ServerMsg::Kicked { reason: reason.to_string() });
        broadcast(s, &ServerMsg::ClientLeft { id, reason: format!("removed: {reason}") });
        let conn = c.conn.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            conn.close(1u32.into(), b"kicked");
        });
    }
    tidy_channels(s);
}

fn apply(s: &mut Shared, me: u32, msg: ClientMsg) -> Result<()> {
    let my_name = s.clients.get(&me).map(|c| c.info.name.clone()).unwrap_or_default();
    match msg {
        ClientMsg::Hello { .. } => {}
        ClientMsg::Ping => {
            if let Some(c) = s.clients.get(&me) {
                let _ = c.tx.send(ServerMsg::Pong);
            }
        }
        ClientMsg::Join { channel, password } => {
            let role = role_of(s, me);
            let ch = s.stored.channels.iter().find(|c| c.channel.id == channel).ok_or_else(|| anyhow!("That channel is gone."))?.clone();
            if let Some(h) = &ch.password_hash {
                if role < Role::Moderator && !password.as_deref().is_some_and(|p| password_ok(h, p)) {
                    bail!("That channel needs a password.");
                }
            }
            if ch.channel.max_users > 0 && s.clients.values().filter(|c| c.info.channel == channel).count() as u32 >= ch.channel.max_users && role < Role::Moderator {
                bail!("That channel is full.");
            }
            update_client(s, me, |c| c.channel = channel);
            tidy_channels(s);
        }
        ClientMsg::Chat { target, text } => {
            let text = clean_text(&text, MAX_CHAT);
            if text.is_empty() {
                return Ok(());
            }
            let msg = ServerMsg::Chat { from: me, from_name: my_name, target: target.clone(), text, at_ms: now_ms() };
            let my_channel = s.clients.get(&me).map(|c| c.info.channel);
            for c in s.clients.values() {
                let send = match &target {
                    ChatTarget::Server => true,
                    ChatTarget::Channel => Some(c.info.channel) == my_channel,
                    ChatTarget::Client { id } => c.info.id == *id || c.info.id == me,
                };
                if send {
                    let _ = c.tx.send(msg.clone());
                }
            }
        }
        ClientMsg::Poke { client, text } => {
            let c = s.clients.get(&client).ok_or_else(|| anyhow!("They've left."))?;
            let _ = c.tx.send(ServerMsg::Poked { from: me, from_name: my_name, text: clean_text(&text, 200) });
        }
        ClientMsg::SetState { muted, deafened, away } => {
            update_client(s, me, |c| {
                c.muted = muted || deafened;
                c.deafened = deafened;
                c.away = away.map(|a| clean_text(&a, 80));
            });
        }
        ClientMsg::Rename { name } => {
            let n = clean_text(&name, MAX_NAME);
            if n.is_empty() {
                bail!("Pick a name.");
            }
            if s.clients.values().any(|c| c.info.id != me && c.info.name.eq_ignore_ascii_case(&n)) {
                bail!("Someone here already uses that name.");
            }
            update_client(s, me, |c| c.name = n);
        }
        ClientMsg::CreateChannel { name, parent, topic, password, permanent, max_users } => {
            if permanent {
                need(s, me, Role::Moderator)?;
            }
            let name = clean_text(&name, 60);
            if name.is_empty() {
                bail!("Give the channel a name.");
            }
            if let Some(p) = parent {
                if !s.stored.channels.iter().any(|c| c.channel.id == p) {
                    bail!("That parent channel is gone.");
                }
            }
            if s.stored.channels.len() >= 500 {
                bail!("This server has too many channels.");
            }
            let id = s.stored.next_channel;
            s.stored.next_channel += 1;
            let order = s.stored.channels.iter().filter(|c| c.channel.parent == parent).map(|c| c.channel.order + 1).max().unwrap_or(0);
            let password_hash = password.filter(|p| !p.is_empty()).map(|p| hash_password(&p)).transpose()?;
            let channel = Channel { id, parent, name, topic: clean_text(&topic, 300), has_password: password_hash.is_some(), permanent, max_users, order };
            s.stored.channels.push(StoredChannel { channel: channel.clone(), password_hash });
            broadcast(s, &ServerMsg::ChannelSaved { channel });
            // The creator goes in, like TeamSpeak
            update_client(s, me, |c| c.channel = id);
            tidy_channels(s);
            save(s);
        }
        ClientMsg::EditChannel { id, name, topic, password, max_users } => {
            need(s, me, Role::Moderator)?;
            let name = clean_text(&name, 60);
            let new_hash = match password {
                None => None,
                Some(None) => Some(None),
                Some(Some(p)) if p.is_empty() => Some(None),
                Some(Some(p)) => Some(Some(hash_password(&p)?)),
            };
            let c = channel_mut(s, id).ok_or_else(|| anyhow!("That channel is gone."))?;
            if !name.is_empty() {
                c.channel.name = name;
            }
            c.channel.topic = clean_text(&topic, 300);
            c.channel.max_users = max_users;
            if let Some(h) = new_hash {
                c.password_hash = h;
            }
            c.channel.has_password = c.password_hash.is_some();
            let channel = c.channel.clone();
            broadcast(s, &ServerMsg::ChannelSaved { channel });
            save(s);
        }
        ClientMsg::DeleteChannel { id } => {
            need(s, me, Role::Moderator)?;
            if id == s.stored.default_channel {
                bail!("The default channel can't be deleted.");
            }
            // The channel and everything under it
            let mut gone = vec![id];
            let mut i = 0;
            while i < gone.len() {
                let p = gone[i];
                gone.extend(s.stored.channels.iter().filter(|c| c.channel.parent == Some(p)).map(|c| c.channel.id));
                i += 1;
            }
            let default = s.stored.default_channel;
            let inside: Vec<u32> = s.clients.values().filter(|c| gone.contains(&c.info.channel)).map(|c| c.info.id).collect();
            for c in inside {
                update_client(s, c, |x| x.channel = default);
            }
            s.stored.channels.retain(|c| !gone.contains(&c.channel.id));
            for g in gone {
                broadcast(s, &ServerMsg::ChannelDeleted { id: g });
            }
            save(s);
        }
        ClientMsg::Move { client, channel } => {
            if client != me {
                need(s, me, Role::Moderator)?;
            }
            if !s.stored.channels.iter().any(|c| c.channel.id == channel) {
                bail!("That channel is gone.");
            }
            update_client(s, client, |c| c.channel = channel);
            tidy_channels(s);
        }
        ClientMsg::Kick { client, reason, from_server } => {
            need(s, me, Role::Moderator)?;
            if role_of(s, client) > role_of(s, me) {
                bail!("You can't kick someone with a higher role.");
            }
            if from_server {
                remove_client(s, client, &clean_text(&reason, 200));
            } else {
                let default = s.stored.default_channel;
                update_client(s, client, |c| c.channel = default);
                if let Some(c) = s.clients.get(&client) {
                    let _ = c.tx.send(ServerMsg::Error { message: format!("{my_name} moved you out of the channel{}", if reason.is_empty() { ".".into() } else { format!(": {reason}") }) });
                }
                tidy_channels(s);
            }
        }
        ClientMsg::Ban { client, reason, minutes } => {
            need(s, me, Role::Moderator)?;
            if role_of(s, client) >= role_of(s, me) {
                bail!("You can't ban someone with the same or a higher role.");
            }
            let key = s.clients.get(&client).map(|c| c.key.clone()).ok_or_else(|| anyhow!("They've left."))?;
            let until_ms = if minutes == 0 { i64::MAX } else { now_ms() + minutes as i64 * 60_000 };
            let reason = clean_text(&reason, 200);
            s.stored.bans.push(Ban { key, until_ms, reason: reason.clone() });
            remove_client(s, client, &format!("banned{}", if reason.is_empty() { String::new() } else { format!(": {reason}") }));
            save(s);
        }
        ClientMsg::SetRole { client, role } => {
            need(s, me, Role::Admin)?;
            let key = s.clients.get(&client).map(|c| c.key.clone()).ok_or_else(|| anyhow!("They've left."))?;
            if role == Role::Member {
                s.stored.roles.remove(&key);
            } else {
                s.stored.roles.insert(key, role);
            }
            update_client(s, client, |c| c.role = role);
            save(s);
        }
        ClientMsg::UsePrivilegeKey { key } => {
            let key = key.trim().to_uppercase();
            let Some(i) = s.stored.privilege_keys.iter().position(|k| k.key == key) else { bail!("That key isn't valid on this server, or was used already.") };
            let pk = s.stored.privilege_keys.remove(i);
            let ident = s.clients.get(&me).map(|c| c.key.clone()).unwrap_or_default();
            s.stored.roles.insert(ident, pk.role);
            update_client(s, me, |c| c.role = pk.role);
            save(s);
        }
        ClientMsg::MakePrivilegeKey { role } => {
            need(s, me, Role::Admin)?;
            let key = random_key();
            s.stored.privilege_keys.push(PrivilegeKey { key: key.clone(), role });
            if let Some(c) = s.clients.get(&me) {
                let _ = c.tx.send(ServerMsg::PrivilegeKey { key, role });
            }
            save(s);
        }
        ClientMsg::EditServer { name, welcome } => {
            need(s, me, Role::Admin)?;
            let name = clean_text(&name, 60);
            if !name.is_empty() {
                s.stored.name = name;
            }
            s.stored.welcome = clean_text(&welcome, 1000);
            let info = s.stored.info();
            broadcast(s, &ServerMsg::ServerUpdated { server: info });
            save(s);
        }
    }
    Ok(())
}

/// For the command-line server: set or clear the join password.
pub fn set_password(data_dir: &Path, password: Option<&str>) -> Result<()> {
    let path = data_dir.join("server.json");
    let mut s: Stored = serde_json::from_slice(&std::fs::read(&path).context("Start the server once first, so it makes its files")?)?;
    s.password_hash = password.filter(|p| !p.is_empty()).map(hash_password).transpose()?;
    std::fs::write(&path, serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}

/// A new admin key, for when the first one is lost.
pub fn new_admin_key(data_dir: &Path) -> Result<String> {
    let path = data_dir.join("server.json");
    let mut s: Stored = serde_json::from_slice(&std::fs::read(&path).context("Start the server once first, so it makes its files")?)?;
    let k = random_key();
    s.privilege_keys.push(PrivilegeKey { key: k.clone(), role: Role::Admin });
    std::fs::write(&path, serde_json::to_vec_pretty(&s)?)?;
    Ok(k)
}
