//! Telegram, through its official API (the same one Telegram's own apps use).
//!
//! - Logging in: phone number, then the code Telegram sends, then the cloud password if one is set.
//!   Sompio never stores the phone number or password; only Telegram's session key is kept,
//!   in the OS credential store, and "Disconnect" logs that session out on Telegram's side too.
//! - Syncing: for every chat, asks Telegram for the messages *you* sent (a server-side filter),
//!   so other people's messages are never downloaded.
//! - Deleting: removes your message for everyone, where Telegram allows it.
//!
//! Using the API needs an "app key" (api_id + api_hash) from my.telegram.org. A release build
//! can carry Sompio's own key (SOMPIO_TG_API_ID / SOMPIO_TG_API_HASH at compile time); otherwise the
//! person enters their own once.

use crate::SecretStore;
use anyhow::{anyhow, bail, Context, Result};
use grammers_client::peer::Peer;
use grammers_client::{Client, SenderPool, SignInError};
use grammers_session::storages::{MemorySession, MemorySessionError};
use grammers_session::types::{DcOption, PeerId, PeerInfo, PeerRef, UpdateState, UpdatesState};
use grammers_session::{BoxFuture, Session, SessionData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sompio_clean::{LiveChannel, LiveMessage};

const KEY_SESSION: &str = "telegram.session";
const KEY_APP: &str = "telegram.app";
const KEY_NAME: &str = "telegram.name";

// ---------------------------------------------------------------------------------
// Session: kept in memory, with only the login keys written to the credential store.

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    home_dc: i32,
    dcs: Vec<DcOption>,
}

pub(crate) struct VaultSession {
    inner: MemorySession,
    saved: Mutex<Saved>,
    store: Arc<dyn SecretStore>,
}

impl VaultSession {
    fn load(store: Arc<dyn SecretStore>) -> Result<Self> {
        let saved: Saved = match store.get(KEY_SESSION)? {
            Some(json) => serde_json::from_str(&json).context("saved Telegram session is unreadable")?,
            None => Saved::default(),
        };
        let mut data = SessionData::default();
        if saved.home_dc != 0 {
            data.home_dc = saved.home_dc;
        }
        for dc in &saved.dcs {
            data.dc_options.insert(dc.id, dc.clone());
        }
        let home_dc = data.home_dc;
        Ok(Self {
            inner: MemorySession::from(data),
            saved: Mutex::new(Saved { home_dc, dcs: saved.dcs }),
            store,
        })
    }

    fn persist(&self) {
        let saved = self.saved.lock().unwrap();
        // Only the home datacenter's login key is worth keeping (and it keeps the entry small
        // enough for Windows Credential Manager). Other datacenters get fresh keys when needed.
        let keep = Saved {
            home_dc: saved.home_dc,
            dcs: saved.dcs.iter().filter(|d| d.auth_key.is_some() && d.id == saved.home_dc).cloned().collect(),
        };
        if let Ok(json) = serde_json::to_string(&keep) {
            let _ = self.store.set(KEY_SESSION, &json);
        }
    }
}

impl Session for VaultSession {
    type Error = MemorySessionError;

    fn home_dc_id(&self) -> Result<i32, Self::Error> {
        self.inner.home_dc_id()
    }
    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, Result<(), Self::Error>> {
        Box::pin(async move {
            self.inner.set_home_dc_id(dc_id).await?;
            self.saved.lock().unwrap().home_dc = dc_id;
            self.persist();
            Ok(())
        })
    }
    fn dc_option(&self, dc_id: i32) -> Result<Option<DcOption>, Self::Error> {
        self.inner.dc_option(dc_id)
    }
    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), Self::Error>> {
        let dc = dc_option.clone();
        Box::pin(async move {
            self.inner.set_dc_option(&dc).await?;
            {
                let mut saved = self.saved.lock().unwrap();
                saved.dcs.retain(|d| d.id != dc.id);
                saved.dcs.push(dc);
            }
            self.persist();
            Ok(())
        })
    }
    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, Self::Error>> {
        self.inner.peer(peer)
    }
    fn cache_peer(&self, peer: PeerInfo) -> BoxFuture<'_, Result<(), Self::Error>> {
        self.inner.cache_peer(peer)
    }
    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, Self::Error>> {
        self.inner.updates_state()
    }
    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, Result<(), Self::Error>> {
        self.inner.set_update_state(update)
    }
}

// ---------------------------------------------------------------------------------
// The connector

#[derive(Serialize, Deserialize, Clone)]
struct AppKey {
    api_id: i32,
    api_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// An app key is available (built in, or entered by the person).
    pub has_app_key: bool,
    pub connected: bool,
    /// The account's display name, when connected.
    pub name: Option<String>,
    /// A login is in progress and waiting for this step.
    pub waiting_for: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum LoginStep {
    Done { name: String },
    NeedPassword { hint: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub chats_done: usize,
    pub chats_total: usize,
    pub messages: usize,
}

struct Connection {
    client: Client,
    pool: tokio::task::JoinHandle<()>,
}

pub struct Telegram {
    store: Arc<dyn SecretStore>,
    conn: Option<Connection>,
    login: Option<grammers_client::client::LoginToken>,
    password: Option<grammers_client::client::PasswordToken>,
    peers: HashMap<i64, PeerRef>,
}

fn built_in_key() -> Option<AppKey> {
    let id = option_env!("SOMPIO_TG_API_ID")?.parse().ok()?;
    let hash = option_env!("SOMPIO_TG_API_HASH")?.to_string();
    Some(AppKey { api_id: id, api_hash: hash })
}

/// Turns Telegram's error codes into something a person can act on.
fn friendly(e: impl std::fmt::Display) -> anyhow::Error {
    let s = e.to_string();
    let msg = if s.contains("PHONE_NUMBER_INVALID") {
        "That phone number wasn't accepted. Include the country code, like +31 6 12345678."
    } else if s.contains("PHONE_CODE_EXPIRED") {
        "That code has expired. Start again to get a new one."
    } else if s.contains("FLOOD_WAIT") || s.contains("FLOOD") {
        "Telegram asked Sompio to slow down. Wait a few minutes and try again."
    } else if s.contains("API_ID_INVALID") {
        "Telegram didn't accept the app key. Check the api_id and api_hash from my.telegram.org."
    } else if s.contains("AUTH_KEY_UNREGISTERED") || s.contains("SESSION_REVOKED") {
        "Telegram ended this login. Connect again."
    } else {
        return anyhow!("Telegram: {s}");
    };
    anyhow!(msg)
}

impl Telegram {
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self { store, conn: None, login: None, password: None, peers: HashMap::new() }
    }

    fn app_key(&self) -> Result<Option<AppKey>> {
        if let Some(k) = built_in_key() {
            return Ok(Some(k));
        }
        Ok(match self.store.get(KEY_APP)? {
            Some(json) => serde_json::from_str(&json).ok(),
            None => None,
        })
    }

    /// Saves an app key from my.telegram.org (only needed when the build has none).
    pub fn set_app_key(&mut self, api_id: &str, api_hash: &str) -> Result<()> {
        let api_id: i32 = api_id.trim().parse().map_err(|_| anyhow!("The api_id is a number, like 1234567."))?;
        let api_hash = api_hash.trim();
        if api_hash.len() != 32 || !api_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("The api_hash is 32 letters and numbers, copied from my.telegram.org.");
        }
        self.store.set(KEY_APP, &serde_json::to_string(&AppKey { api_id, api_hash: api_hash.to_string() })?)?;
        self.disconnect_local();
        Ok(())
    }

    fn client(&mut self) -> Result<Client> {
        if let Some(c) = &self.conn {
            return Ok(c.client.clone());
        }
        let key = self.app_key()?.ok_or_else(|| anyhow!("Add a Telegram app key first."))?;
        let session = Arc::new(VaultSession::load(self.store.clone())?);
        let SenderPool { runner, handle, .. } = SenderPool::new(session, key.api_id);
        let client = Client::new(handle);
        let pool = tokio::spawn(async move {
            runner.run().await;
        });
        self.conn = Some(Connection { client: client.clone(), pool });
        Ok(client)
    }

    fn disconnect_local(&mut self) {
        if let Some(c) = self.conn.take() {
            c.client.disconnect();
            c.pool.abort();
        }
        self.login = None;
        self.password = None;
        self.peers.clear();
    }

    /// Reports the connection without going online: Sompio only contacts Telegram when the
    /// person logs in, syncs or deletes.
    pub async fn status(&mut self) -> Result<Status> {
        let has_app_key = self.app_key()?.is_some();
        let waiting_for = if self.password.is_some() {
            Some("password")
        } else if self.login.is_some() {
            Some("code")
        } else {
            None
        };
        let connected = has_app_key && self.store.get(KEY_SESSION)?.is_some() && self.store.get(KEY_NAME)?.is_some();
        let name = if connected { self.store.get(KEY_NAME)? } else { None };
        Ok(Status { has_app_key, connected, name, waiting_for })
    }

    fn logged_in(&mut self, user: grammers_client::peer::User) -> Result<LoginStep> {
        let name = user.full_name();
        self.store.set(KEY_NAME, &name)?;
        Ok(LoginStep::Done { name })
    }

    /// Checks the login is still valid; if Telegram ended it, forgets it here too.
    async fn authorized_client(&mut self) -> Result<Client> {
        let client = self.client()?;
        if !client.is_authorized().await.map_err(friendly)? {
            self.disconnect_local();
            self.store.delete(KEY_SESSION)?;
            self.store.delete(KEY_NAME)?;
            bail!("Telegram ended this login, so Sompio has disconnected. Connect again to continue.");
        }
        Ok(client)
    }

    /// Step 1: Telegram sends a login code to the person's Telegram app (or by SMS).
    pub async fn start_login(&mut self, phone: &str) -> Result<()> {
        let key = self.app_key()?.ok_or_else(|| anyhow!("Add a Telegram app key first."))?;
        let phone: String = phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
        if phone.len() < 8 {
            bail!("Enter your phone number with the country code, like +31 6 12345678.");
        }
        let client = self.client()?;
        let token = client.request_login_code(&phone, &key.api_hash).await.map_err(friendly)?;
        self.login = Some(token);
        self.password = None;
        Ok(())
    }

    /// Step 2: the code. May lead to step 3 when the account has a cloud password.
    pub async fn submit_code(&mut self, code: &str) -> Result<LoginStep> {
        let token = self.login.take().ok_or_else(|| anyhow!("Start again: enter your phone number first."))?;
        let client = self.client()?;
        let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
        match client.sign_in(&token, &code).await {
            Ok(user) => self.logged_in(user),
            Err(SignInError::PasswordRequired(pt)) => {
                let hint = pt.hint().map(str::to_string);
                self.password = Some(pt);
                Ok(LoginStep::NeedPassword { hint })
            }
            Err(SignInError::InvalidCode) => {
                self.login = Some(token);
                bail!("That code isn't right. Check the newest message from Telegram and try again.")
            }
            Err(SignInError::SignUpRequired) => bail!("This phone number has no Telegram account."),
            Err(e) => Err(friendly(e)),
        }
    }

    /// Step 3: the cloud password (two-step verification). Sent to Telegram only, never stored.
    pub async fn submit_password(&mut self, password: &str) -> Result<LoginStep> {
        let token = self.password.take().ok_or_else(|| anyhow!("Start again: enter your phone number first."))?;
        let client = self.client()?;
        match client.check_password(token, password.as_bytes()).await {
            Ok(user) => self.logged_in(user),
            Err(SignInError::InvalidPassword(pt)) => {
                self.password = Some(pt);
                bail!("That password isn't right.")
            }
            Err(e) => Err(friendly(e)),
        }
    }

    /// Logs out on Telegram's side and forgets the session on this computer.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(c) = &self.conn {
            let _ = c.client.sign_out().await;
        }
        self.disconnect_local();
        self.store.delete(KEY_SESSION)?;
        self.store.delete(KEY_NAME)?;
        Ok(())
    }

    /// Downloads every message you sent, chat by chat. Other people's messages are not fetched.
    pub async fn fetch_own_messages(&mut self, progress: &mut (dyn FnMut(SyncProgress) + Send)) -> Result<Vec<LiveChannel>> {
        let client = self.authorized_client().await?;
        // Collect the chat list first, so progress can show a total.
        let mut dialogs = Vec::new();
        let mut it = client.iter_dialogs();
        while let Some(d) = it.next().await.map_err(friendly)? {
            dialogs.push((d.peer().clone(), d.peer_ref()));
        }
        self.peers.clear();
        let total = dialogs.len();
        let mut out = Vec::new();
        let mut count = 0;
        for (i, (peer, peer_ref)) in dialogs.into_iter().enumerate() {
            progress(SyncProgress { chats_done: i, chats_total: total, messages: count });
            let bare = peer.id().bare_id().unwrap_or(0);
            self.peers.insert(bare, peer_ref);
            let (kind, linkable) = match &peer {
                Peer::User(_) => ("dm", false),
                Peer::Group(_) => ("group_dm", false),
                Peer::Channel(c) if matches!(c.kind(), Some(grammers_session::types::ChannelKind::Broadcast)) => ("channel", true),
                _ => ("group_dm", true),
            };
            let name = peer.name().unwrap_or("Telegram chat").to_string();
            let mut messages = Vec::new();
            let mut search = client.search_messages(peer_ref).sent_by_self();
            while let Some(m) = search.next().await.map_err(friendly)? {
                let text = m.text().to_string();
                if text.trim().is_empty() {
                    continue;
                }
                let id = m.id();
                messages.push(LiveMessage {
                    ext_id: format!("{bare}/{id}"),
                    ts: m.date().as_millisecond(),
                    content: text,
                    link: linkable.then(|| format!("https://t.me/c/{bare}/{id}")),
                });
            }
            count += messages.len();
            if !messages.is_empty() {
                out.push(LiveChannel { ext_id: bare.to_string(), name, kind: kind.to_string(), messages });
            }
            // Be gentle with Telegram: a short pause between chats.
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        progress(SyncProgress { chats_done: total, chats_total: total, messages: count });
        Ok(out)
    }

    /// Deletes one of your messages for everyone. `uid` is Sompio's "telegram:<chat>/<message>".
    pub async fn delete_message(&mut self, uid: &str) -> Result<()> {
        let (chat, msg) = parse_uid(uid)?;
        let client = self.authorized_client().await?;
        if !self.peers.contains_key(&chat) {
            // After a restart the chat list isn't cached yet: look the chat up again.
            let mut it = client.iter_dialogs();
            while let Some(d) = it.next().await.map_err(friendly)? {
                if let Some(bare) = d.peer().id().bare_id() {
                    self.peers.insert(bare, d.peer_ref());
                }
            }
        }
        let peer = *self.peers.get(&chat).ok_or_else(|| anyhow!("That chat isn't in your Telegram chat list any more."))?;
        client.delete_messages(peer, &[msg]).await.map_err(friendly)?;
        Ok(())
    }
}

/// "telegram:12345/678" -> (12345, 678)
pub fn parse_uid(uid: &str) -> Result<(i64, i32)> {
    let rest = uid.strip_prefix("telegram:").ok_or_else(|| anyhow!("not a Telegram message"))?;
    let (chat, msg) = rest.split_once('/').ok_or_else(|| anyhow!("unexpected message id"))?;
    Ok((chat.parse()?, msg.parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryStore;

    #[test]
    fn uids() {
        assert_eq!(parse_uid("telegram:1500100200/55").unwrap(), (1500100200, 55));
        assert!(parse_uid("discord:1/2").is_err());
    }

    #[test]
    fn app_key_is_checked_and_stored() {
        let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let mut tg = Telegram::new(store.clone());
        assert!(tg.set_app_key("abc", "0123456789abcdef0123456789abcdef").is_err());
        assert!(tg.set_app_key("123", "short").is_err());
        tg.set_app_key(" 1234567 ", "0123456789abcdef0123456789abcdef").unwrap();
        let k = tg.app_key().unwrap().unwrap();
        assert_eq!(k.api_id, 1234567);
    }

    #[tokio::test]
    async fn session_keeps_only_login_keys() {
        let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let s = VaultSession::load(store.clone()).unwrap();
        let mut dc = s.dc_option(2).unwrap().unwrap();
        s.set_home_dc_id(2).await.unwrap();
        dc.auth_key = Some([7u8; 256]);
        s.set_dc_option(&dc).await.unwrap();
        let saved = store.get(KEY_SESSION).unwrap().unwrap();
        assert!(saved.len() < 2000, "fits in Windows Credential Manager: {} bytes", saved.len());
        let again = VaultSession::load(store).unwrap();
        assert_eq!(again.home_dc_id().unwrap(), 2);
        assert_eq!(again.dc_option(2).unwrap().unwrap().auth_key, Some([7u8; 256]));
    }

    #[tokio::test]
    async fn status_never_goes_online() {
        let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let mut tg = Telegram::new(store.clone());
        let s = tg.status().await.unwrap();
        assert!(!s.connected && !s.has_app_key && s.waiting_for.is_none());
        tg.set_app_key("1234567", "0123456789abcdef0123456789abcdef").unwrap();
        store.set(KEY_SESSION, "{}").unwrap();
        store.set(KEY_NAME, "Sam").unwrap();
        let s = tg.status().await.unwrap();
        assert!(s.connected && s.name.as_deref() == Some("Sam"));
        assert!(tg.conn.is_none(), "no connection was opened");
    }
}

/// The desktop app runs these on a multi-threaded runtime, so every future must be `Send`.
/// This only has to compile.
#[allow(dead_code)]
mod send_checks {
    use super::*;
    fn send<T: Send>(_: T) {}
    fn status(t: &mut Telegram) { send(t.status()) }
    fn login(t: &mut Telegram) { send(t.start_login("")) }
    fn code(t: &mut Telegram) { send(t.submit_code("")) }
    fn password(t: &mut Telegram) { send(t.submit_password("")) }
    fn disconnect(t: &mut Telegram) { send(t.disconnect()) }
    fn delete(t: &mut Telegram) { send(t.delete_message("")) }
    fn fetch(t: &mut Telegram, f: &mut (dyn FnMut(SyncProgress) + Send)) { send(t.fetch_own_messages(f)) }
    fn whole(t: Telegram) { send(t) }
}
