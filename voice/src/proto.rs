//! What client and server say to each other. Control messages are JSON, each prefixed with
//! its length, on one QUIC stream; voice travels as QUIC datagrams (unreliable, low latency).

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 9987;
pub const ALPN: &[u8] = b"sompio-voice/1";
/// Longest control message accepted
pub const MAX_MESSAGE: usize = 64 * 1024;
pub const MAX_CHAT: usize = 2000;
pub const MAX_NAME: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Member,
    Moderator,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub id: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub topic: String,
    pub has_password: bool,
    /// Kept when empty; temporary channels go away when the last person leaves
    pub permanent: bool,
    pub max_users: u32,
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Client {
    pub id: u32,
    pub name: String,
    pub channel: u32,
    pub role: Role,
    pub muted: bool,
    pub deafened: bool,
    pub away: Option<String>,
    /// Short fingerprint of the person's key, to tell same-named people apart
    pub identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerInfo {
    pub name: String,
    pub welcome: String,
    pub max_clients: u32,
    pub default_channel: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum ChatTarget {
    Channel,
    Server,
    Client { id: u32 },
}

/// Client to server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello { version: u32, name: String, public_key: String, signature: String, password: Option<String> },
    Join { channel: u32, password: Option<String> },
    Chat { target: ChatTarget, text: String },
    Poke { client: u32, text: String },
    SetState { muted: bool, deafened: bool, away: Option<String> },
    Rename { name: String },
    CreateChannel { name: String, parent: Option<u32>, topic: String, password: Option<String>, permanent: bool, max_users: u32 },
    EditChannel { id: u32, name: String, topic: String, password: Option<Option<String>>, max_users: u32 },
    DeleteChannel { id: u32 },
    Move { client: u32, channel: u32 },
    Kick { client: u32, reason: String, from_server: bool },
    Ban { client: u32, reason: String, minutes: u32 },
    SetRole { client: u32, role: Role },
    UsePrivilegeKey { key: String },
    MakePrivilegeKey { role: Role },
    EditServer { name: String, welcome: String },
    Ping,
}

/// Server to client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// First thing a client hears: sign this nonce to prove who you are
    Challenge { version: u32, nonce: String, name: String, needs_password: bool },
    Welcome { you: u32, role: Role, server: ServerInfo, channels: Vec<Channel>, clients: Vec<Client> },
    Error { message: String },
    ChannelSaved { channel: Channel },
    ChannelDeleted { id: u32 },
    ClientJoined { client: Client },
    ClientLeft { id: u32, reason: String },
    ClientUpdated { client: Client },
    Chat { from: u32, from_name: String, target: ChatTarget, text: String, at_ms: i64 },
    Poked { from: u32, from_name: String, text: String },
    /// You were removed from the server
    Kicked { reason: String },
    ServerUpdated { server: ServerInfo },
    PrivilegeKey { key: String, role: Role },
    Pong,
}

/// The bytes a client signs: the server's nonce and its certificate fingerprint, so a signature
/// made for one server can't be replayed to another.
pub fn hello_payload(nonce: &str, fingerprint: &str) -> Vec<u8> {
    format!("sompio-voice-hello\n{nonce}\n{fingerprint}").into_bytes()
}

pub async fn write_msg<T: Serialize>(w: &mut quinn::SendStream, msg: &T) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    w.write_all(&(body.len() as u32).to_be_bytes()).await?;
    w.write_all(&body).await?;
    Ok(())
}

pub async fn read_msg<T: for<'de> Deserialize<'de>>(r: &mut quinn::RecvStream) -> anyhow::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_MESSAGE {
        anyhow::bail!("message too long");
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

/// Voice packets. To the server: [seq u16][opus]. From the server: [sender u32][seq u16][opus].
pub fn voice_out(seq: u16, opus: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + opus.len());
    v.extend_from_slice(&seq.to_be_bytes());
    v.extend_from_slice(opus);
    v
}

pub fn voice_relay(sender: u32, packet: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + packet.len());
    v.extend_from_slice(&sender.to_be_bytes());
    v.extend_from_slice(packet);
    v
}

pub fn parse_relay(p: &[u8]) -> Option<(u32, u16, &[u8])> {
    if p.len() < 6 {
        return None;
    }
    Some((u32::from_be_bytes([p[0], p[1], p[2], p[3]]), u16::from_be_bytes([p[4], p[5]]), &p[6..]))
}

/// SHA-256 of the server's certificate, shown as groups people can compare.
pub fn fingerprint(cert_der: &[u8]) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(cert_der);
    h.iter().take(16).map(|b| format!("{b:02X}")).collect::<Vec<_>>().chunks(2).map(|c| c.concat()).collect::<Vec<_>>().join(":")
}

/// A short readable id for a person's public key.
pub fn short_identity(public_key_hex: &str) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(public_key_hex.as_bytes());
    h.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
