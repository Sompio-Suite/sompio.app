//! Sompio licence keys.
//!
//! A key says which apps it unlocks and for how many days, and carries our Ed25519 signature,
//! so the app can check it completely offline. Only the key maker (and later the website) holds
//! the signing key; the app ships with the public half, which can check signatures but not make them.
//!
//! Layout of the 15 signed bytes:
//!   version (1) · apps (1, bit flags) · days (2) · issued day (2, days since 2024-01-01) · flags (1) · serial (8)
//! followed by the 64-byte signature, written in Crockford base32 with a "SOMPIO-" prefix.
//!
//! Keys stack: a new key's days start where the current ones end. Nothing renews by itself.

use anyhow::{anyhow, bail, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

const VERSION: u8 = 1;
const BODY_LEN: usize = 15;
const KEY_LEN: usize = BODY_LEN + 64;
const PREFIX: &str = "SOMPIO";
const DAY_MS: i64 = 86_400_000;
/// 2024-01-01 in days since 1970-01-01.
const EPOCH_DAY: i64 = 19_723;
/// A key has to be entered within this many days of being made, so old keys don't circulate forever.
pub const ACTIVATE_WITHIN_DAYS: i64 = 365;
pub const TRIAL_DAYS: i64 = 14;
/// A "forever" key: the most days a key can hold (about 179 years). Shown as "Forever".
pub const FOREVER_DAYS: u16 = u16::MAX;

/// "forever" or "31 days", for messages and file names.
pub fn length_label(days: u16) -> String {
    if days == FOREVER_DAYS { "forever".into() } else { format!("{days} days") }
}

/// "forever" or a number of days, as typed in the key maker.
pub fn parse_days(s: &str) -> Option<u16> {
    match s.trim().to_ascii_lowercase().as_str() {
        "forever" | "lifetime" => Some(FOREVER_DAYS),
        "month" => Some(31),
        "year" => Some(365),
        d => d.parse::<u16>().ok().filter(|n| *n > 0),
    }
}
/// How far the clock may go backwards before Sompio stops trusting it (time zones, small corrections).
const CLOCK_SLACK_MS: i64 = 36 * 3_600_000;

/// Serials of keys we've seen shared publicly. Blocked in the next release.
const REVOKED: &[&str] = &[];

/// The public half of the signing key, written by the key maker before each build.
const PUBLIC_KEY_HEX: &str = include_str!("../public.key");

// Apps

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum App {
    Clean,
    Calendar,
    Vault,
    Voice,
}

impl App {
    pub const ALL: [App; 4] = [App::Clean, App::Calendar, App::Vault, App::Voice];
    fn bit(self) -> u8 {
        match self {
            App::Clean => 1,
            App::Calendar => 2,
            App::Vault => 4,
            App::Voice => 8,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            App::Clean => "Trace",
            App::Calendar => "Calendar",
            App::Vault => "Vault",
            App::Voice => "Voice",
        }
    }
    pub fn parse(s: &str) -> Option<App> {
        App::ALL.into_iter().find(|a| a.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Which apps a key unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Apps(u8);

impl Apps {
    pub const SUITE: Apps = Apps(0b1111);
    pub fn of(list: &[App]) -> Apps {
        Apps(list.iter().fold(0, |b, a| b | a.bit()))
    }
    pub fn contains(self, a: App) -> bool {
        self.0 & a.bit() != 0
    }
    pub fn list(self) -> Vec<App> {
        App::ALL.into_iter().filter(|a| self.contains(*a)).collect()
    }
    pub fn is_suite(self) -> bool {
        self.0 & 0b1111 == 0b1111
    }
    /// "Sompio Suite", "Clean", "Clean + Vault"
    pub fn label(self) -> String {
        if self.is_suite() {
            "Sompio Suite".into()
        } else {
            self.list().iter().map(|a| a.name()).collect::<Vec<_>>().join(" + ")
        }
    }
    /// "suite", "clean", "clean,vault"
    pub fn parse(s: &str) -> Option<Apps> {
        if s.trim().eq_ignore_ascii_case("suite") {
            return Some(Apps::SUITE);
        }
        let list: Option<Vec<App>> = s.split([',', '+']).map(App::parse).collect();
        list.filter(|l| !l.is_empty()).map(|l| Apps::of(&l))
    }
}

// Keys

/// What a key holds, once its signature checks out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub apps: Apps,
    pub days: u16,
    /// Milliseconds since 1970, midnight UTC of the day it was made
    pub issued_ms: i64,
    /// Made for testing; the app labels it as such
    pub test: bool,
    pub serial: [u8; 8],
}

impl Key {
    pub fn serial_hex(&self) -> String {
        self.serial.iter().map(|b| format!("{b:02x}")).collect()
    }
    /// The last moment this key can be entered.
    pub fn activate_by_ms(&self) -> i64 {
        self.issued_ms + ACTIVATE_WITHIN_DAYS * DAY_MS
    }
    fn body(&self) -> [u8; BODY_LEN] {
        let mut b = [0u8; BODY_LEN];
        b[0] = VERSION;
        b[1] = self.apps.0;
        b[2..4].copy_from_slice(&self.days.to_le_bytes());
        let day = (self.issued_ms.div_euclid(DAY_MS) - EPOCH_DAY).clamp(0, u16::MAX as i64) as u16;
        b[4..6].copy_from_slice(&day.to_le_bytes());
        b[6] = self.test as u8;
        b[7..15].copy_from_slice(&self.serial);
        b
    }
}

/// Makes a new random key. Only the key maker and the website call this.
pub fn issue(signing: &SigningKey, apps: Apps, days: u16, now_ms: i64, test: bool) -> Result<String> {
    if apps.0 == 0 {
        bail!("Choose at least one app");
    }
    if days == 0 {
        bail!("A key needs at least one day");
    }
    let mut serial = [0u8; 8];
    getrandom::getrandom(&mut serial).map_err(|e| anyhow!("no randomness: {e}"))?;
    let key = Key { apps, days, issued_ms: now_ms.div_euclid(DAY_MS) * DAY_MS, test, serial };
    let body = key.body();
    let sig = signing.sign(&body);
    let mut bytes = Vec::with_capacity(KEY_LEN);
    bytes.extend_from_slice(&body);
    bytes.extend_from_slice(&sig.to_bytes());
    Ok(format_key(&bytes))
}

/// Reads and checks a key. Errors are written for the person typing it in.
pub fn parse(text: &str, public: &VerifyingKey) -> Result<Key> {
    let cleaned: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    let Some(data) = cleaned.strip_prefix(PREFIX) else {
        bail!("That doesn't look like a Sompio key. Keys start with SOMPIO-.");
    };
    let bytes = base32_decode(data).ok_or_else(|| anyhow!("That key has a character that doesn't belong. Copy it again, all of it."))?;
    if bytes.len() != KEY_LEN {
        bail!("That key is {} it should be. Copy it again, all of it.", if bytes.len() < KEY_LEN { "shorter than" } else { "longer than" });
    }
    let (body, sig) = bytes.split_at(BODY_LEN);
    let sig = Signature::from_slice(sig).map_err(|_| anyhow!("That key isn't valid."))?;
    public
        .verify(body, &sig)
        .map_err(|_| anyhow!("That key isn't valid. Check you copied all of it; if it still fails, contact us with your account number."))?;
    if body[0] != VERSION {
        bail!("This key is for a newer version of Sompio. Update the app, then add it again.");
    }
    let day = u16::from_le_bytes([body[4], body[5]]) as i64;
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&body[7..15]);
    Ok(Key {
        apps: Apps(body[1]),
        days: u16::from_le_bytes([body[2], body[3]]),
        issued_ms: (EPOCH_DAY + day) * DAY_MS,
        test: body[6] & 1 == 1,
        serial,
    })
}

/// The public key built into this copy of Sompio.
pub fn public_key() -> Result<VerifyingKey> {
    public_key_from_hex(PUBLIC_KEY_HEX)
}

pub fn public_key_from_hex(hex: &str) -> Result<VerifyingKey> {
    let bytes = from_hex(hex.trim()).ok_or_else(|| anyhow!("public key is not hex"))?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("public key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&arr).map_err(|e| anyhow!("bad public key: {e}"))
}

pub fn new_signing_key() -> Result<SigningKey> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| anyhow!("no randomness: {e}"))?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn signing_key_from_hex(hex: &str) -> Result<SigningKey> {
    let bytes = from_hex(hex.trim()).ok_or_else(|| anyhow!("signing key is not hex"))?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("signing key must be 32 bytes"))?;
    Ok(SigningKey::from_bytes(&arr))
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

// Crockford base32: no I, L, O or U, so keys can't be misread.

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn format_key(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char);
    }
    let groups: Vec<String> = out.as_bytes().chunks(8).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
    format!("{PREFIX}-{}", groups.join("-"))
}

fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.chars() {
        let c = match c {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let v = ALPHABET.iter().position(|&a| a as char == c)? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// What's unlocked on this device

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Activation {
    key: String,
    activated_at: i64,
}

/// The keys entered on this device, plus the trial and the latest time seen.
/// Stored as a small JSON file next to the app's data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Book {
    #[serde(default)]
    keys: Vec<Activation>,
    #[serde(default)]
    trial_started: Option<i64>,
    #[serde(default)]
    last_seen: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub app: App,
    pub name: &'static str,
    pub active: bool,
    /// When access ends (ms since 1970), if it has ever been unlocked
    pub ends_at: Option<i64>,
    /// Unlocked only by the free trial
    pub trial_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyView {
    pub serial: String,
    pub label: String,
    pub days: u16,
    pub activated_at: i64,
    pub test: bool,
    /// The key no longer checks out (blocked, or the file was changed)
    pub invalid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub apps: Vec<AppStatus>,
    pub keys: Vec<KeyView>,
    pub trial_ends_at: Option<i64>,
    /// The PC's clock is behind a time Sompio has already seen
    pub clock_behind: bool,
    pub now: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Added {
    pub label: String,
    pub days: u16,
    pub test: bool,
    pub status: Status,
}

impl Book {
    pub fn load(path: &std::path::Path) -> Book {
        std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// The time to trust: never earlier than a time already seen, so setting the clock back gains nothing.
    fn now(&mut self, clock: i64) -> i64 {
        self.last_seen = self.last_seen.max(clock);
        self.last_seen
    }

    /// Starts the free trial the first time Sompio runs on a device.
    pub fn start_trial(&mut self, clock: i64) {
        if self.trial_started.is_none() {
            let now = self.now(clock);
            self.trial_started = Some(now);
        }
    }

    /// Adds a key. The days count from the end of what's already unlocked.
    pub fn add(&mut self, text: &str, clock: i64, public: &VerifyingKey) -> Result<Added> {
        let key = parse(text, public)?;
        let now = self.now(clock);
        if REVOKED.contains(&key.serial_hex().as_str()) {
            bail!("This key has been blocked because it was shared publicly. Contact us with your account number if you bought it.");
        }
        if self.keys.iter().any(|a| parse(&a.key, public).map(|k| k.serial == key.serial).unwrap_or(false)) {
            bail!("This key is already added on this device.");
        }
        if now > key.activate_by_ms() {
            bail!("This key is more than a year old and can no longer be added. Contact us with your account number.");
        }
        self.keys.push(Activation { key: text.trim().to_string(), activated_at: now });
        Ok(Added { label: key.apps.label(), days: key.days, test: key.test, status: self.status(clock, public) })
    }

    pub fn status(&mut self, clock: i64, public: &VerifyingKey) -> Status {
        let clock_behind = clock + CLOCK_SLACK_MS < self.last_seen;
        let now = self.now(clock);
        let trial_end = self.trial_started.map(|t| t + TRIAL_DAYS * DAY_MS);
        let mut ends: [Option<i64>; 4] = [None; 4];
        let mut by_key = [false; 4];
        let mut keys = Vec::new();
        let mut order: Vec<&Activation> = self.keys.iter().collect();
        order.sort_by_key(|a| a.activated_at);
        for a in order {
            let parsed = parse(&a.key, public).ok().filter(|k| !REVOKED.contains(&k.serial_hex().as_str()));
            match parsed {
                Some(k) => {
                    for (i, app) in App::ALL.into_iter().enumerate() {
                        if k.apps.contains(app) {
                            // From the later of: when it was added, the end of earlier keys, the end of the trial
                            let start = a.activated_at.max(ends.get(i).copied().flatten().unwrap_or(0)).max(trial_end.unwrap_or(0));
                            if let Some(e) = ends.get_mut(i) {
                                *e = Some(start + k.days as i64 * DAY_MS);
                            }
                            if let Some(b) = by_key.get_mut(i) {
                                *b = true;
                            }
                        }
                    }
                    keys.push(KeyView { serial: k.serial_hex(), label: k.apps.label(), days: k.days, activated_at: a.activated_at, test: k.test, invalid: false });
                }
                None => keys.push(KeyView { serial: String::new(), label: "Unknown key".into(), days: 0, activated_at: a.activated_at, test: false, invalid: true }),
            }
        }
        let apps = App::ALL
            .into_iter()
            .enumerate()
            .map(|(i, app)| {
                let key_end = ends.get(i).copied().flatten();
                let end = match (key_end, trial_end) {
                    (Some(k), Some(t)) => Some(k.max(t)),
                    (k, t) => k.or(t),
                };
                let paid = key_end.is_some_and(|e| e > now) && by_key.get(i).copied().unwrap_or(false);
                AppStatus { app, name: app.name(), active: end.is_some_and(|e| e > now), ends_at: end, trial_only: !paid }
            })
            .collect();
        keys.reverse();
        Status { apps, keys, trial_ends_at: trial_end, clock_behind, now }
    }

    /// Is this app usable right now?
    pub fn allows(&mut self, app: App, clock: i64, public: &VerifyingKey) -> bool {
        self.status(clock, public).apps.iter().any(|s| s.app == app && s.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_762_646_400_000; // 2025-11-09

    fn pair() -> (SigningKey, VerifyingKey) {
        let s = new_signing_key().unwrap();
        let p = s.verifying_key();
        (s, p)
    }

    #[test]
    fn round_trip_and_tamper() {
        let (s, p) = pair();
        let text = issue(&s, Apps::of(&[App::Clean, App::Vault]), 30, T0, true).unwrap();
        assert!(text.starts_with("SOMPIO-"));
        let k = parse(&text, &p).unwrap();
        assert_eq!((k.apps.label().as_str(), k.days, k.test), ("Trace + Vault", 30, true));
        assert_eq!(k.issued_ms, T0.div_euclid(DAY_MS) * DAY_MS);
        // Lower case, spaces and missing dashes are fine
        assert!(parse(&text.to_lowercase().replace('-', " "), &p).is_ok());
        // One changed character breaks it
        let mut chars: Vec<char> = text.chars().collect();
        let i = 20;
        chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
        let bad: String = chars.into_iter().collect();
        assert!(parse(&bad, &p).unwrap_err().to_string().contains("isn't valid"));
        // Another signer's key is refused
        let (_, other) = pair();
        assert!(parse(&text, &other).is_err());
        assert!(parse("hello", &p).unwrap_err().to_string().contains("SOMPIO"));
        assert!(parse(&text[..60], &p).unwrap_err().to_string().contains("shorter"));
    }

    #[test]
    fn keys_stack_and_expire() {
        let (s, p) = pair();
        let mut book = Book::default();
        let a = issue(&s, Apps::SUITE, 30, T0, false).unwrap();
        let b = issue(&s, Apps::of(&[App::Vault]), 90, T0, false).unwrap();
        book.add(&a, T0, &p).unwrap();
        assert!(book.add(&a, T0, &p).unwrap_err().to_string().contains("already"));
        book.add(&b, T0 + DAY_MS, &p).unwrap();
        let st = book.status(T0 + 2 * DAY_MS, &p);
        let vault = st.apps.iter().find(|x| x.app == App::Vault).unwrap();
        assert_eq!(vault.ends_at, Some(T0 + 120 * DAY_MS), "Vault: 30 + 90 days, stacked");
        let clean = st.apps.iter().find(|x| x.app == App::Clean).unwrap();
        assert_eq!(clean.ends_at, Some(T0 + 30 * DAY_MS));
        assert!(clean.active && !clean.trial_only);
        assert!(!book.allows(App::Clean, T0 + 31 * DAY_MS, &p));
        assert!(book.allows(App::Vault, T0 + 31 * DAY_MS, &p));
    }

    #[test]
    fn forever_keys_and_day_words() {
        let (s, p) = pair();
        let mut book = Book::default();
        book.add(&issue(&s, Apps::SUITE, 31, T0, false).unwrap(), T0, &p).unwrap();
        book.add(&issue(&s, Apps::SUITE, FOREVER_DAYS, T0, false).unwrap(), T0, &p).unwrap();
        // 100 years on, still unlocked
        assert!(book.allows(App::Vault, T0 + 36_500 * DAY_MS, &p));
        assert_eq!(parse_days("forever"), Some(FOREVER_DAYS));
        assert_eq!(parse_days(" Year "), Some(365));
        assert_eq!(parse_days("month"), Some(31));
        assert_eq!(parse_days("0"), None);
        assert_eq!(length_label(FOREVER_DAYS), "forever");
        assert_eq!(length_label(31), "31 days");
    }

    #[test]
    fn clock_cannot_be_turned_back() {
        let (s, p) = pair();
        let mut book = Book::default();
        book.add(&issue(&s, Apps::SUITE, 30, T0, false).unwrap(), T0, &p).unwrap();
        assert!(!book.allows(App::Clean, T0 + 40 * DAY_MS, &p));
        // Setting the clock back to day 5 doesn't bring it back
        let st = book.status(T0 + 5 * DAY_MS, &p);
        assert!(st.clock_behind);
        assert!(!st.apps.iter().any(|a| a.active));
    }

    #[test]
    fn trial_then_key() {
        let (s, p) = pair();
        let mut book = Book::default();
        book.start_trial(T0);
        book.start_trial(T0 + 5 * DAY_MS); // only the first counts
        let st = book.status(T0 + DAY_MS, &p);
        assert!(st.apps.iter().all(|a| a.active && a.trial_only));
        assert!(!book.allows(App::Clean, T0 + 15 * DAY_MS, &p));
        // A key bought during the trial starts when the trial ends
        let mut book2 = Book::default();
        book2.start_trial(T0);
        book2.add(&issue(&s, Apps::of(&[App::Clean]), 30, T0, false).unwrap(), T0 + 2 * DAY_MS, &p).unwrap();
        let clean = book2.status(T0 + 3 * DAY_MS, &p).apps.into_iter().find(|a| a.app == App::Clean).unwrap();
        assert_eq!(clean.ends_at, Some(T0 + 44 * DAY_MS));
    }

    #[test]
    fn old_keys_and_edited_files() {
        let (s, p) = pair();
        let old = issue(&s, Apps::SUITE, 30, T0, false).unwrap();
        let mut book = Book::default();
        assert!(book.add(&old, T0 + 400 * DAY_MS, &p).unwrap_err().to_string().contains("year old"));
        // A key edited inside the saved file stops counting
        let mut book = Book::default();
        book.add(&old, T0, &p).unwrap();
        let json = serde_json::to_string(&book).unwrap().replace("SOMPIO-", "SOMPIO-ZZ");
        let mut tampered: Book = serde_json::from_str(&json).unwrap();
        let st = tampered.status(T0, &p);
        assert!(st.keys[0].invalid && !st.apps.iter().any(|a| a.active));
    }

    #[test]
    fn built_in_public_key_parses() {
        public_key().unwrap();
    }
}
