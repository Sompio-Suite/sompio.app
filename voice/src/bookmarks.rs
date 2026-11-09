//! Your saved servers and the fingerprints you trusted, encrypted with a key from the lock.

use anyhow::{anyhow, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bookmark {
    pub label: String,
    pub address: String,
    pub name: String,
    /// The certificate fingerprint you trusted
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub last_channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Saved {
    pub bookmarks: Vec<Bookmark>,
    /// Push-to-talk shortcut, "" for voice activation
    #[serde(default)]
    pub push_to_talk: String,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default)]
    pub nickname: String,
}

fn default_threshold() -> f32 {
    0.02
}

pub fn load(path: &Path, key: &[u8; 32]) -> Result<Saved> {
    let Ok(raw) = std::fs::read(path) else { return Ok(Saved { threshold: default_threshold(), ..Default::default() }) };
    if raw.len() < 24 {
        return Err(anyhow!("voice settings are damaged"));
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| anyhow!("{e}"))?;
    let plain = cipher.decrypt(raw[..24].into(), &raw[24..]).map_err(|_| anyhow!("voice settings didn't open with this key"))?;
    Ok(serde_json::from_slice(&plain)?)
}

pub fn save(path: &Path, key: &[u8; 32], s: &Saved) -> Result<()> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| anyhow!("{e}"))?;
    let nonce: [u8; 24] = rand::random();
    let mut out = nonce.to_vec();
    out.extend(cipher.encrypt(&nonce.into(), serde_json::to_vec(s)?.as_slice()).map_err(|_| anyhow!("encryption failed"))?);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, out)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
