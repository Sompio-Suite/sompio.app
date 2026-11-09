//! Connecting to a voice server. The server's certificate is checked against the fingerprint
//! you trusted the first time (like SSH). Your identity is a separate key for every server,
//! derived from your Sompio Lock, so servers can't tell you're the same person elsewhere.

use crate::proto::*;
use anyhow::{anyhow, bail, Result};
use ed25519_dalek::{Signer, SigningKey};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub enum Event {
    Msg(ServerMsg),
    Voice { from: u32, seq: u16, data: Vec<u8> },
    Closed { reason: String },
}

pub enum Outcome {
    Connected(Session),
    /// First time on this server: show the fingerprint and ask before connecting
    NewServer { fingerprint: String, name: String },
    /// The certificate changed since you trusted it: someone may be in between
    Changed { expected: String, got: String },
}

pub struct Session {
    conn: quinn::Connection,
    tx: mpsc::UnboundedSender<ClientMsg>,
    pub fingerprint: String,
    pub welcome: ServerMsg,
    pub address: String,
    endpoint: quinn::Endpoint,
}

impl Session {
    pub fn send(&self, msg: ClientMsg) -> Result<()> {
        self.tx.send(msg).map_err(|_| anyhow!("Not connected."))
    }
    pub fn send_voice(&self, seq: u16, opus: &[u8]) {
        let _ = self.conn.send_datagram(bytes::Bytes::from(voice_out(seq, opus)));
    }
    pub fn rtt_ms(&self) -> u64 {
        self.conn.rtt().as_millis() as u64
    }
    pub async fn close(self) {
        self.conn.close(0u32.into(), b"bye");
        self.endpoint.wait_idle().await;
    }
}

/// Accepts the server's certificate only if it matches the trusted fingerprint (or records it
/// the first time). Names aren't checked: servers are self-signed, the fingerprint is the trust.
#[derive(Debug)]
struct Pinned {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::client::danger::ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity);
        *self.seen.lock().map_err(|_| rustls::Error::General("lock".into()))? = Some(fp.clone());
        match &self.expected {
            Some(e) if *e != fp => Err(rustls::Error::General("fingerprint changed".into())),
            _ => Ok(rustls::client::danger::ServerCertVerified::assertion()),
        }
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.2 isn't used".into()))
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &rustls::pki_types::CertificateDer<'_>, dss: &rustls::DigitallySignedStruct) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// "voice.example.org", "voice.example.org:9988", "203.0.113.5" or "[2001:db8::1]:9987"
pub async fn resolve(address: &str) -> Result<SocketAddr> {
    let a = address.trim();
    if a.is_empty() {
        bail!("Enter the server's address.");
    }
    let with_port = if a.parse::<SocketAddr>().is_ok() {
        a.to_string()
    } else if a.starts_with('[') || a.matches(':').count() == 1 {
        a.to_string()
    } else if a.contains(':') {
        format!("[{a}]:{DEFAULT_PORT}")
    } else {
        format!("{a}:{DEFAULT_PORT}")
    };
    let mut addrs = tokio::net::lookup_host(&with_port).await.map_err(|_| anyhow!("Couldn't find \"{a}\". Check the address."))?;
    addrs.next().ok_or_else(|| anyhow!("Couldn't find \"{a}\". Check the address."))
}

/// This person's key for one server: from their master identity and the server's fingerprint.
pub fn identity_for(master: &[u8; 32], fingerprint: &str) -> SigningKey {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"sompio-voice-identity-v1"), master);
    let mut seed = zeroize::Zeroizing::new([0u8; 32]);
    hk.expand(fingerprint.as_bytes(), seed.as_mut()).expect("valid length");
    SigningKey::from_bytes(&seed)
}

pub struct Connect<'a> {
    pub address: &'a str,
    pub name: &'a str,
    pub password: Option<&'a str>,
    pub master_identity: &'a [u8; 32],
    pub trusted: Option<&'a str>,
}

pub async fn connect(c: Connect<'_>, events: mpsc::UnboundedSender<Event>) -> Result<Outcome> {
    let addr = resolve(c.address).await?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let seen = Arc::new(Mutex::new(None));
    let verifier = Pinned { expected: c.trusted.map(str::to_string), seen: seen.clone(), provider: provider.clone() };
    let mut tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut cc = quinn::ClientConfig::new(Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(tls)?));
    cc.transport_config(Arc::new(crate::server::transport()));
    let bind: SocketAddr = if addr.is_ipv6() { "[::]:0".parse()? } else { "0.0.0.0:0".parse()? };
    let mut endpoint = quinn::Endpoint::client(bind)?;
    endpoint.set_default_client_config(cc);
    let connecting = endpoint.connect(addr, "sompio-voice")?;
    let conn = match tokio::time::timeout(std::time::Duration::from_secs(10), connecting).await {
        Err(_) => bail!("The server didn't answer. Check the address, and that UDP port {} is open on the server.", addr.port()),
        Ok(Err(e)) => {
            let got = seen.lock().ok().and_then(|s| s.clone());
            if let (Some(expected), Some(got)) = (c.trusted, got) {
                if expected != got {
                    return Ok(Outcome::Changed { expected: expected.into(), got });
                }
            }
            bail!("Couldn't connect: {e}");
        }
        Ok(Ok(conn)) => conn,
    };
    let fp = seen.lock().ok().and_then(|s| s.clone()).ok_or_else(|| anyhow!("No certificate seen"))?;
    let (mut send, mut recv) = conn.open_bi().await?;
    // The stream only reaches the server once something is written on it
    send.write_all(&[]).await?;
    write_msg(&mut send, &ClientMsg::Ping).await.ok();
    let challenge: ServerMsg = tokio::time::timeout(std::time::Duration::from_secs(10), read_msg(&mut recv)).await??;
    let ServerMsg::Challenge { version, nonce, name, needs_password } = challenge else { bail!("The server sent something unexpected.") };
    if c.trusted.is_none() {
        conn.close(0u32.into(), b"checking");
        return Ok(Outcome::NewServer { fingerprint: fp, name });
    }
    if version != VERSION {
        bail!("This server runs a different version of Sompio Voice.");
    }
    if needs_password && c.password.unwrap_or("").is_empty() {
        conn.close(0u32.into(), b"password");
        bail!("This server needs a password.");
    }
    let key = identity_for(c.master_identity, &fp);
    let sig = key.sign(&hello_payload(&nonce, &fp));
    let hello = ClientMsg::Hello {
        version: VERSION,
        name: c.name.into(),
        public_key: hex::encode(key.verifying_key().to_bytes()),
        signature: hex::encode(sig.to_bytes()),
        password: c.password.map(str::to_string),
    };
    write_msg(&mut send, &hello).await?;
    let welcome: ServerMsg = loop {
        let m: ServerMsg = tokio::time::timeout(std::time::Duration::from_secs(10), read_msg(&mut recv)).await??;
        match m {
            ServerMsg::Welcome { .. } => break m,
            ServerMsg::Error { message } => bail!("{message}"),
            ServerMsg::Pong => continue,
            _ => continue,
        }
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<ClientMsg>();
    tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if write_msg(&mut send, &m).await.is_err() {
                break;
            }
        }
    });
    let ev = events.clone();
    tokio::spawn(async move {
        let reason = loop {
            match read_msg::<ServerMsg>(&mut recv).await {
                Ok(ServerMsg::Kicked { reason }) => break format!("You were removed: {reason}"),
                Ok(m) => {
                    if ev.send(Event::Msg(m)).is_err() {
                        break "closed".into();
                    }
                }
                Err(_) => break "The connection to the server was lost.".into(),
            }
        };
        let _ = ev.send(Event::Closed { reason });
    });
    let vconn = conn.clone();
    let ev = events.clone();
    tokio::spawn(async move {
        while let Ok(d) = vconn.read_datagram().await {
            if let Some((from, seq, data)) = parse_relay(&d) {
                if ev.send(Event::Voice { from, seq, data: data.to_vec() }).is_err() {
                    break;
                }
            }
        }
    });
    Ok(Outcome::Connected(Session { conn, tx, fingerprint: fp, welcome, address: c.address.to_string(), endpoint }))
}
