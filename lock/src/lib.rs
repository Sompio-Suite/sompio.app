//! The Sompio Lock.
//!
//! Everything Sompio stores is encrypted with one random 256-bit vault key. The vault key itself is
//! kept only in wrapped (encrypted) form, in `lock.json`, and can be unwrapped two ways:
//!
//! * **Master password + key picture.** The password goes through Argon2id (64 MiB, 3 passes); the
//!   result is mixed with a 256-bit secret hidden inside a picture the person keeps on their phone.
//!   Both are needed. On a trusted computer the picture's secret can be remembered in the OS credential
//!   store, so day to day only the password is typed.
//! * **Recovery code.** 120 random bits, printed once on the recovery sheet, also through Argon2id.
//!   It is the way back in if the password or the picture is lost, and is replaced after use.
//!
//! Wrapping is XChaCha20-Poly1305, so a wrong password or picture is detected, never "decrypts" to junk.
//! Each app gets its own key derived from the vault key with HKDF, so Clean's database key is not the vault key.
//!
//! Nothing here can reset a forgotten password and lost recovery code. That is the point.

mod picture;

use anyhow::{anyhow, bail, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub use picture::{make_picture, read_picture, PictureSecret};

const FORMAT: u32 = 1;
/// Argon2id cost. About half a second on an ordinary laptop: slow for guessing, fine for unlocking.
const M_COST_KIB: u32 = 64 * 1024;
const T_COST: u32 = 3;

/// The key everything is encrypted with. Wiped from memory when dropped.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct VaultKey([u8; 32]);

impl std::fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultKey(hidden)")
    }
}

impl VaultKey {
    /// A key for one purpose, like "clean.db". Different purposes never share a key.
    pub fn derive(&self, purpose: &str) -> Zeroizing<[u8; 32]> {
        let hk = hkdf::Hkdf::<Sha256>::new(Some(b"sompio-vault-v1"), &self.0);
        let mut out = Zeroizing::new([0u8; 32]);
        hk.expand(purpose.as_bytes(), out.as_mut()).expect("32 bytes is a valid HKDF length");
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Kdf {
    m_kib: u32,
    t: u32,
    p: u32,
    salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Wrapped {
    nonce: String,
    data: String,
}

/// What's saved in lock.json. Holds no secret that works on its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    format: u32,
    password_kdf: Kdf,
    /// Vault key wrapped with password + picture
    main: Wrapped,
    /// First bytes of the picture secret's hash, to tell a wrong picture from a wrong password
    picture_id: String,
    recovery_kdf: Kdf,
    /// Vault key wrapped with the recovery code
    recovery: Wrapped,
    created_ms: i64,
    /// YubiKeys that stand in for the key picture
    #[serde(default)]
    hardware_keys: Vec<HardwareKey>,
}

/// A YubiKey added to the lock. Its HMAC answer to `challenge` opens a copy of the picture's
/// secret, which then works with the password exactly like the picture does.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HardwareKey {
    serial: Option<u32>,
    label: String,
    slot: u8,
    challenge: String,
    picture: Wrapped,
    added_ms: i64,
}

/// What the app shows about an added YubiKey, and what it needs to ask it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareKeyInfo {
    pub index: usize,
    pub serial: Option<u32>,
    pub label: String,
    pub slot: u8,
    #[serde(skip)]
    pub challenge: [u8; 32],
    pub added_ms: i64,
}

fn hardware_wrap_key(response: &[u8; 20], challenge: &[u8]) -> Zeroizing<[u8; 32]> {
    let hk = hkdf::Hkdf::<Sha256>::new(Some(challenge), response);
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(b"sompio-lock-yubikey-v1", out.as_mut()).expect("valid length");
    out
}

/// What setup hands back: the lock file to save, the picture to give the person, and the code to print.
pub struct Setup {
    pub file: LockFile,
    pub vault: VaultKey,
    pub picture_png: Vec<u8>,
    pub picture_secret: PictureSecret,
    pub recovery_code: Zeroizing<String>,
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|e| anyhow!("no randomness available: {e}"))?;
    Ok(b)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        bail!("lock file is damaged");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| anyhow!("lock file is damaged")))
        .collect()
}

fn new_kdf() -> Result<Kdf> {
    Ok(Kdf { m_kib: M_COST_KIB, t: T_COST, p: 1, salt: hex(&random::<16>()?) })
}

fn stretch(secret: &[u8], kdf: &Kdf) -> Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|e| anyhow!("{e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon.hash_password_into(secret, &unhex(&kdf.salt)?, out.as_mut()).map_err(|e| anyhow!("{e}"))?;
    Ok(out)
}

/// Password and picture together make the wrapping key; neither alone is enough.
fn main_key(password: &str, picture: &PictureSecret, kdf: &Kdf) -> Result<Zeroizing<[u8; 32]>> {
    let stretched = stretch(password.as_bytes(), kdf)?;
    let hk = hkdf::Hkdf::<Sha256>::new(Some(picture.as_bytes()), stretched.as_ref());
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(b"sompio-lock-main-v1", out.as_mut()).expect("valid length");
    Ok(out)
}

fn wrap(key: &Zeroizing<[u8; 32]>, vault: &VaultKey) -> Result<Wrapped> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|e| anyhow!("{e}"))?;
    let nonce = random::<24>()?;
    let data = cipher.encrypt(&nonce.into(), vault.0.as_slice()).map_err(|_| anyhow!("encryption failed"))?;
    Ok(Wrapped { nonce: hex(&nonce), data: hex(&data) })
}

fn unwrap(key: &Zeroizing<[u8; 32]>, w: &Wrapped) -> Option<VaultKey> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).ok()?;
    let nonce: [u8; 24] = unhex(&w.nonce).ok()?.try_into().ok()?;
    let plain = Zeroizing::new(cipher.decrypt(&nonce.into(), unhex(&w.data).ok()?.as_slice()).ok()?);
    let arr: [u8; 32] = plain.as_slice().try_into().ok()?;
    Some(VaultKey(arr))
}

fn picture_id(p: &PictureSecret) -> String {
    hex(&Sha256::digest(p.as_bytes())[..6])
}

// Recovery codes: 24 Crockford base32 characters in six groups, like 7KQ2-XR9D-4M8T-PLWA-0G3H-ZE5N.

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn new_recovery_code() -> Result<Zeroizing<String>> {
    let bytes = random::<15>()?; // 120 bits
    let mut chars = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            chars.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    let groups: Vec<&str> = (0..6).map(|i| &chars[i * 4..i * 4 + 4]).collect();
    Ok(Zeroizing::new(groups.join("-")))
}

/// Accepts the code however it's typed: lower case, spaces, O for 0, I or L for 1.
fn normalize_code(code: &str) -> Result<Zeroizing<String>> {
    let s: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect();
    if s.len() != 24 || !s.bytes().all(|b| ALPHABET.contains(&b)) {
        bail!("A recovery code has 24 letters and numbers, in six groups of four. Check it and try again.");
    }
    Ok(Zeroizing::new(s))
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Checks a new master password before it's used. Returns why it isn't good enough.
pub fn password_problem(password: &str) -> Option<&'static str> {
    let len = password.chars().count();
    if len < 10 {
        return Some("Use at least 10 characters. A few unrelated words work well.");
    }
    let distinct = password.chars().collect::<std::collections::HashSet<_>>().len();
    if distinct < 5 {
        return Some("That password repeats too much. Mix it up a little.");
    }
    let lower = password.to_lowercase();
    for weak in ["password", "wachtwoord", "123456789", "qwertyuiop", "sompio"] {
        if lower.contains(weak) && len < 16 {
            return Some("That password is easy to guess. Try a few unrelated words.");
        }
    }
    None
}

impl LockFile {
    /// Creates a new lock: a fresh vault key, a new key picture and a recovery code.
    pub fn create(password: &str) -> Result<Setup> {
        if let Some(p) = password_problem(password) {
            bail!("{p}");
        }
        let vault = VaultKey(random::<32>()?);
        let picture_secret = PictureSecret::new()?;
        Self::build(password, vault, picture_secret)
    }

    /// Like `create`, with a key picture made (and saved by the person) beforehand.
    pub fn create_with_picture(password: &str, picture_secret: PictureSecret) -> Result<Setup> {
        if let Some(p) = password_problem(password) {
            bail!("{p}");
        }
        let vault = VaultKey(random::<32>()?);
        Self::build(password, vault, picture_secret)
    }

    fn build(password: &str, vault: VaultKey, picture_secret: PictureSecret) -> Result<Setup> {
        let password_kdf = new_kdf()?;
        let main = wrap(&main_key(password, &picture_secret, &password_kdf)?, &vault)?;
        let recovery_code = new_recovery_code()?;
        let recovery_kdf = new_kdf()?;
        let recovery = wrap(&stretch(normalize_code(&recovery_code)?.as_bytes(), &recovery_kdf)?, &vault)?;
        let file = LockFile {
            format: FORMAT,
            password_kdf,
            main,
            picture_id: picture_id(&picture_secret),
            recovery_kdf,
            recovery,
            created_ms: now_ms(),
            // A new picture secret means old YubiKey entries no longer fit; they're added again.
            hardware_keys: Vec::new(),
        };
        let picture_png = make_picture(&picture_secret)?;
        Ok(Setup { file, vault, picture_png, picture_secret, recovery_code })
    }

    /// Opens the lock with the password and the picture's secret.
    pub fn unlock(&self, password: &str, picture: &PictureSecret) -> Result<VaultKey> {
        if picture_id(picture) != self.picture_id {
            bail!("That's not the key picture for this Sompio. Use the picture you saved when you set up the lock.");
        }
        unwrap(&main_key(password, picture, &self.password_kdf)?, &self.main)
            .ok_or_else(|| anyhow!("That password isn't right. Check Caps Lock and try again."))
    }

    /// Opens the lock with the recovery code, then sets a new password, picture and recovery code.
    /// The old picture and code stop working.
    pub fn recover(&self, code: &str, new_password: &str) -> Result<Setup> {
        let normalized = normalize_code(code)?;
        let vault = unwrap(&stretch(normalized.as_bytes(), &self.recovery_kdf)?, &self.recovery)
            .ok_or_else(|| anyhow!("That recovery code isn't right. Check each group of four and try again."))?;
        if let Some(p) = password_problem(new_password) {
            bail!("{p}");
        }
        Self::build(new_password, vault, PictureSecret::new()?)
    }

    /// A new password (same picture, same recovery code), after unlocking with the old one.
    pub fn change_password(&mut self, old: &str, new: &str, picture: &PictureSecret) -> Result<()> {
        let vault = self.unlock(old, picture)?;
        if let Some(p) = password_problem(new) {
            bail!("{p}");
        }
        self.password_kdf = new_kdf()?;
        self.main = wrap(&main_key(new, picture, &self.password_kdf)?, &vault)?;
        Ok(())
    }

    /// A fresh random challenge for adding a YubiKey.
    pub fn new_challenge() -> Result<[u8; 32]> {
        random::<32>()
    }

    /// Adds a YubiKey: its answer to `challenge` will open the picture's secret. Needs the
    /// picture (or the remembered copy) to prove it belongs to this lock.
    pub fn add_hardware_key(&mut self, picture: &PictureSecret, challenge: [u8; 32], response: [u8; 20], slot: u8, serial: Option<u32>, label: &str) -> Result<()> {
        if picture_id(picture) != self.picture_id {
            bail!("That's not the key picture for this Sompio.");
        }
        if serial.is_some() && self.hardware_keys.iter().any(|k| k.serial == serial) {
            bail!("This YubiKey is already added.");
        }
        let picture_copy = VaultKey(*picture.as_bytes());
        let wrapped = wrap(&hardware_wrap_key(&response, &challenge), &picture_copy)?;
        let label = if label.trim().is_empty() { serial.map(|s| format!("YubiKey {s}")).unwrap_or_else(|| "YubiKey".into()) } else { label.trim().to_string() };
        self.hardware_keys.push(HardwareKey { serial, label, slot, challenge: hex(&challenge), picture: wrapped, added_ms: now_ms() });
        Ok(())
    }

    pub fn hardware_keys(&self) -> Vec<HardwareKeyInfo> {
        self.hardware_keys
            .iter()
            .enumerate()
            .filter_map(|(index, k)| {
                let challenge: [u8; 32] = unhex(&k.challenge).ok()?.try_into().ok()?;
                Some(HardwareKeyInfo { index, serial: k.serial, label: k.label.clone(), slot: k.slot, challenge, added_ms: k.added_ms })
            })
            .collect()
    }

    /// The picture's secret, from a YubiKey's answer. Then unlock with the password as usual.
    pub fn picture_from_hardware(&self, index: usize, response: &[u8; 20]) -> Result<PictureSecret> {
        let k = self.hardware_keys.get(index).ok_or_else(|| anyhow!("That YubiKey isn't added to this Sompio."))?;
        let opened = unwrap(&hardware_wrap_key(response, &unhex(&k.challenge)?), &k.picture)
            .ok_or_else(|| anyhow!("This YubiKey isn't the one added to this Sompio, or its slot {} was set up again.", k.slot))?;
        PictureSecret::from_hex(&hex(&opened.0)).ok_or_else(|| anyhow!("lock file is damaged"))
    }

    pub fn remove_hardware_key(&mut self, index: usize) -> Result<()> {
        if index >= self.hardware_keys.len() {
            bail!("That YubiKey isn't added to this Sompio.");
        }
        self.hardware_keys.remove(index);
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Option<LockFile>> {
        match std::fs::read(path) {
            Ok(b) => {
                let f: LockFile = serde_json::from_slice(&b).context("The lock file is damaged")?;
                if f.format > FORMAT {
                    bail!("This lock was made by a newer Sompio. Update the app to open it.");
                }
                Ok(Some(f))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod hardware_tests {
    use super::*;

    #[test]
    fn a_yubikey_stands_in_for_the_picture() {
        let setup = LockFile::create("correct horse battery staple").unwrap();
        let mut file = setup.file;
        let ch = LockFile::new_challenge().unwrap();
        let answer = [42u8; 20];
        file.add_hardware_key(&setup.picture_secret, ch, answer, 2, Some(1234), "").unwrap();
        assert!(file.add_hardware_key(&setup.picture_secret, ch, answer, 2, Some(1234), "").is_err(), "no duplicates");
        assert!(file.add_hardware_key(&PictureSecret::new().unwrap(), ch, answer, 2, Some(99), "").is_err(), "needs the right picture");
        let keys = file.hardware_keys();
        assert_eq!(keys[0].label, "YubiKey 1234");
        assert_eq!(keys[0].challenge, ch);
        let pic = file.picture_from_hardware(0, &answer).unwrap();
        assert!(file.unlock("correct horse battery staple", &pic).is_ok());
        assert!(file.picture_from_hardware(0, &[1u8; 20]).is_err(), "another key's answer fails");
        // Survives a password change and a save/load
        file.change_password("correct horse battery staple", "new horse battery staple", &pic).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock.json");
        file.save(&path).unwrap();
        let loaded = LockFile::load(&path).unwrap().unwrap();
        let pic2 = loaded.picture_from_hardware(0, &answer).unwrap();
        assert!(loaded.unlock("new horse battery staple", &pic2).is_ok());
        // Recovery makes a new picture, so YubiKeys are added again afterwards
        let rec = loaded.recover(&setup.recovery_code, "third horse battery staple").unwrap();
        assert!(rec.file.hardware_keys().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PW: &str = "fell lantern quiet 42";

    #[test]
    fn setup_unlock_and_wrong_inputs() {
        let s = LockFile::create(PW).unwrap();
        let pic = read_picture(&s.picture_png).unwrap();
        let v = s.file.unlock(PW, &pic).unwrap();
        assert_eq!(v.derive("clean.db").as_ref(), s.vault.derive("clean.db").as_ref());
        assert_ne!(v.derive("clean.db").as_ref(), v.derive("calendar.db").as_ref());
        assert!(s.file.unlock("fell lantern quiet 43", &pic).unwrap_err().to_string().contains("password"));
        let other = PictureSecret::new().unwrap();
        assert!(s.file.unlock(PW, &other).unwrap_err().to_string().contains("not the key picture"));
    }

    #[test]
    fn saved_file_round_trip_has_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock.json");
        let s = LockFile::create(PW).unwrap();
        s.file.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&hex(&s.vault.0)));
        assert!(!text.contains(&hex(s.picture_secret.as_bytes())));
        assert!(!text.contains(PW));
        let loaded = LockFile::load(&path).unwrap().unwrap();
        loaded.unlock(PW, &s.picture_secret).unwrap();
    }

    #[test]
    fn recovery_replaces_everything_but_the_data_key() {
        let s = LockFile::create(PW).unwrap();
        let code = s.recovery_code.to_lowercase().replace('-', " ").replace('0', "o");
        assert!(s.file.recover("AAAA-AAAA-AAAA-AAAA-AAAA-AAAA", "new pass phrase here").is_err());
        let r = s.file.recover(&code, "new pass phrase here").unwrap();
        assert_eq!(r.vault.derive("clean.db").as_ref(), s.vault.derive("clean.db").as_ref(), "same data key, data still opens");
        assert_ne!(r.recovery_code.as_str(), s.recovery_code.as_str());
        let new_pic = read_picture(&r.picture_png).unwrap();
        r.file.unlock("new pass phrase here", &new_pic).unwrap();
        assert!(r.file.unlock("new pass phrase here", &s.picture_secret).is_err(), "the old picture stops working");
        assert!(r.file.recover(&s.recovery_code, "another pass phrase").is_err(), "the old code stops working");
    }

    #[test]
    fn change_password() {
        let mut s = LockFile::create(PW).unwrap();
        s.file.change_password(PW, "a different phrase now", &s.picture_secret).unwrap();
        assert!(s.file.unlock(PW, &s.picture_secret).is_err());
        s.file.unlock("a different phrase now", &s.picture_secret).unwrap();
        // Recovery code still works
        s.file.recover(&s.recovery_code, "third phrase for luck").unwrap();
    }

    #[test]
    fn weak_passwords_are_refused() {
        assert!(LockFile::create("short").is_err());
        assert!(LockFile::create("aaaaaaaaaaaa").is_err());
        assert!(LockFile::create("password123!").is_err());
        assert!(password_problem("correct horse battery").is_none());
    }
}
