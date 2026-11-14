//! The exposure scanner: finds personal information you have posted.
//!
//! Every detector is a regex plus, where possible, a real validity check
//! (IBAN checksum, Luhn for cards, digit counts for phone numbers) to keep
//! false alarms low. Results are stored as byte positions only.

use crate::Store;
use sha2::{Digest, Sha256};
use anyhow::Result;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Credential,
    Card,
    Iban,
    WatchTerm,
    Address,
    Phone,
    Email,
    IpAddress,
    Postcode,
    CryptoWallet,
    SensitiveFile,
    IdNumber,
    Passport,
    BackupCode,
    BirthDate,
    NumberPlate,
    Location,
}

impl Kind {
    pub const ALL: [Kind; 17] = [
        Kind::Credential,
        Kind::BackupCode,
        Kind::IdNumber,
        Kind::Passport,
        Kind::Card,
        Kind::Iban,
        Kind::WatchTerm,
        Kind::Address,
        Kind::Phone,
        Kind::Email,
        Kind::Location,
        Kind::IpAddress,
        Kind::BirthDate,
        Kind::Postcode,
        Kind::SensitiveFile,
        Kind::NumberPlate,
        Kind::CryptoWallet,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Credential => "credential",
            Kind::Card => "card",
            Kind::Iban => "iban",
            Kind::WatchTerm => "watch_term",
            Kind::Address => "address",
            Kind::Phone => "phone",
            Kind::Email => "email",
            Kind::IpAddress => "ip_address",
            Kind::Postcode => "postcode",
            Kind::CryptoWallet => "crypto_wallet",
            Kind::SensitiveFile => "sensitive_file",
            Kind::IdNumber => "id_number",
            Kind::Passport => "passport",
            Kind::BackupCode => "backup_code",
            Kind::BirthDate => "birth_date",
            Kind::NumberPlate => "number_plate",
            Kind::Location => "location",
        }
    }

    pub fn from_key(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.key() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Credential => "Passwords, tokens & keys",
            Kind::Card => "Payment cards",
            Kind::Iban => "Bank accounts (IBAN)",
            Kind::WatchTerm => "Your watch terms",
            Kind::Address => "Street addresses",
            Kind::Phone => "Phone numbers",
            Kind::Email => "Email addresses",
            Kind::IpAddress => "IP addresses",
            Kind::Postcode => "Postcodes",
            Kind::CryptoWallet => "Crypto wallets",
            Kind::SensitiveFile => "Sensitive files on this PC",
            Kind::IdNumber => "National ID numbers",
            Kind::Passport => "Passport & ID card numbers",
            Kind::BackupCode => "Backup codes & recovery phrases",
            Kind::BirthDate => "Dates of birth",
            Kind::NumberPlate => "Number plates",
            Kind::Location => "Exact locations",
        }
    }

    /// What the person should do about it, in plain words.
    pub fn advice(self) -> &'static str {
        match self {
            Kind::Credential => "Change this password or revoke this token now. Deleting the message doesn't undo the leak, because others may already have seen it.",
            Kind::Card => "Contact your bank to block or replace this card, then delete the message.",
            Kind::Iban => "An IBAN alone can't empty your account, but scammers use it to sound convincing. Delete the message unless you share this account number publicly anyway.",
            Kind::WatchTerm => "You told Sompio this is sensitive. Decide whether this message needs to go.",
            Kind::Address => "Delete the message unless you're fine with everyone in this chat knowing this address.",
            Kind::Phone => "Delete the message if you don't want this number linked to your account there.",
            Kind::Email => "Check whether you want this address linked to your account there. If it's a public work address, mark it as not a problem everywhere.",
            Kind::IpAddress => "An IP address can show roughly where you are. Delete the message if this was your home connection.",
            Kind::Postcode => "A postcode narrows down where you live. Low risk on its own, more so next to a street name.",
            Kind::CryptoWallet => "Wallet addresses are public, but tying one to your account reveals its balance and history.",
            Kind::SensitiveFile => "This file's name suggests an identity document, money matters or secrets. Keep it only if you need it, ideally in an encrypted folder or password manager, and move it to the Recycle Bin if you don't.",
            Kind::IdNumber => "A national ID number (like a BSN, CNP or social security number) is what fraudsters need to open accounts in your name. Delete the message, and only give this number to official bodies, your employer, bank or doctor.",
            Kind::Passport => "With a passport or ID card number, someone can pose as you. Delete the message. If a photo of the document went out too, report it to your town hall or the police and consider replacing the document.",
            Kind::BackupCode => "Backup codes and recovery phrases get around your two-factor login or open a crypto wallet outright. Make new backup codes on that site now (that cancels the old ones), or move the coins to a new wallet, then delete the message.",
            Kind::BirthDate => "Your date of birth is a common security question and a key piece for identity fraud, together with your name. Delete the message if this is your own date of birth.",
            Kind::Location => "This photo or map link shows an exact spot. If it's your home, school or work, delete the post. Before sharing a photo again, remove its location: in Windows, right-click the file, choose Properties, then Details, then Remove Properties and Personal Information.",
            Kind::NumberPlate => "A number plate can be tied to your car, and so to where you park and drive. Delete the message unless it's fine for this chat to know it.",
        }
    }

    /// 3 = high, 2 = medium, 1 = low
    pub fn severity(self) -> i64 {
        match self {
            Kind::Credential | Kind::Card | Kind::Iban | Kind::WatchTerm | Kind::Address => 3,
            Kind::IdNumber | Kind::Passport | Kind::BackupCode => 3,
            Kind::Phone | Kind::Email | Kind::IpAddress | Kind::SensitiveFile | Kind::BirthDate | Kind::Location => 2,
            Kind::Postcode | Kind::CryptoWallet | Kind::NumberPlate => 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Things only you know are sensitive: your real name, street, employer, school...
    pub watch_terms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detection {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
}

pub struct Scanner {
    detectors: Vec<(Kind, Regex)>,
    watch: Option<Regex>,
}

impl Scanner {
    pub fn new(opts: &ScanOptions) -> Result<Self> {
        let d = |k: Kind, p: &str| -> Result<(Kind, Regex)> { Ok((k, Regex::new(p)?)) };
        let detectors = vec![
            d(Kind::Credential, r"\b[A-Za-z0-9_-]{24,28}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,40}\b")?, // Discord token / JWT-like
            d(Kind::Credential, r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}")?, // JWT
            d(Kind::Credential, r"\b(?:ghp|gho|ghu|ghs)_[A-Za-z0-9]{36}\b|\bgithub_pat_[A-Za-z0-9_]{50,}\b")?,
            d(Kind::Credential, r"\bAKIA[0-9A-Z]{16}\b|\bsk-[A-Za-z0-9_-]{20,}|\bxox[abprs]-[A-Za-z0-9-]{10,}")?,
            d(Kind::Credential, r"(?i)\b(?:password|passwd|pwd|pass|wachtwoord|passwort|parola|parolă)\s*(?:is\s*)?[:=]\s*\S{4,}")?,
            d(Kind::Card, r"\b[0-9](?:[ -]?[0-9]){12,18}\b")?,
            d(Kind::Iban, r"\b[A-Z]{2}[0-9]{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,3})?\b")?,
            // Dutch / German / Belgian compound street names: "Prinsengracht 263", "Hauptstraße 5"
            d(Kind::Address, r"(?i)\b\p{L}{2}[\p{L}'-]*(?:straat|laan|weg|gracht|plein|kade|dijk|singel|steeg|strasse|straße|gasse|allee)\s+[0-9]{1,4}[a-z]?\b")?,
            // English: "221B Baker Street" (capitalised name words, to skip "5 minutes down the road")
            d(Kind::Address, r"\b[0-9]{1,5}[A-Za-z]?\s+(?:[A-Z][\p{L}'.-]*\s+){1,3}(?i:street|avenue|ave|road|rd|boulevard|blvd|lane)\b\.?")?,
            // Romanian: "Strada Lipscani nr. 12", "Bd. Unirii 5"
            d(Kind::Address, r"(?i)\b(?:strada|str\.|bulevardul|bd\.|calea|aleea|[șşs]oseaua|splaiul)\s+\p{L}[\p{L} .-]{1,40}?,?\s*(?:nr\.?\s*)?[0-9]{1,4}\b")?,
            d(Kind::Phone, r"(?:\+|\b00)[1-9][0-9 ().-]{7,18}[0-9]")?,
            d(Kind::Phone, r"\b0[1-9][0-9 -]{7,12}[0-9]\b")?,
            d(Kind::Phone, r"\(?\b[2-9][0-9]{2}\)?[ .-][0-9]{3}[ .-][0-9]{4}\b")?,
            d(Kind::Email, r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b")?,
            d(Kind::IpAddress, r"\b(?:(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])\b")?,
            d(Kind::Postcode, r"\b[1-9][0-9]{3} ?[A-Z]{2}\b")?, // Dutch postcode
            d(Kind::CryptoWallet, r"\bbc1[a-z0-9]{25,59}\b|\b0x[a-fA-F0-9]{40}\b")?,
            // Dutch BSN, only next to its name (nine-digit numbers are everywhere); checked with the eleven test
            d(Kind::IdNumber, r"(?i)\b(?:bsn|burgerservicenummer|sofi-?nummer|citizen service number)\b\W{0,5}(?:(?:is|nr|nummer|number)\.?\s*:?\s*)?[0-9](?:[ .]?[0-9]){8}\b")?,
            // US social security number, 123-45-6789
            d(Kind::IdNumber, r"\b[0-8][0-9]{2}-[0-9]{2}-[0-9]{4}\b")?,
            // Romanian CNP: 13 digits with a check digit and a real birth date inside
            d(Kind::IdNumber, r"\b[1-8][0-9]{12}\b")?,
            d(Kind::Passport, r"(?i:\b(?:passport|paspoort|pa[sș]aport|reisepass|id[ -]?card|identity card|identiteitskaart|id-kaart)(?:\s+(?:no|nr|number|nummer|num[aă]r))?\.?\s*(?:is\s+)?[:#]?\s*)[A-Z0-9]{6,9}\b")?,
            d(Kind::BackupCode, r"(?i:\b(?:backup|recovery|herstel|reserve|2fa|two[- ]factor)[ -]?(?:codes?|keys?)\b[^\n]{0,20}?)[A-Za-z0-9]{4,6}[- ]?[A-Za-z0-9]{4,6}\b")?,
            d(Kind::BackupCode, r"(?i)\b(?:seed|recovery|secret|mnemonic|herstel)\s*(?:phrase|words|zin)\b\W{0,10}(?:is\s*)?:?\s*(?:[a-z]{3,8}\s+){11,23}[a-z]{3,8}\b")?,
            d(Kind::BirthDate, r"(?i:\b(?:born(?:\s+on)?|date of birth|dob|d\.o\.b\.?|birthday(?:\s+is)?|geboren(?:\s+op|\s+am)?|geboortedatum|geb\.|n[ăa]scut[ăa]?(?:\s+(?:la|pe|[îi]n))?|data na[șs]terii|geburtsdatum)\s*:?\s*)(?:[0-3]?[0-9][-/. ][01]?[0-9][-/. ](?:19|20)[0-9]{2}|(?:19|20)[0-9]{2}-[01][0-9]-[0-3][0-9]|[0-3]?[0-9](?:st|nd|rd|th)?\s+(?i:jan|feb|maart|mar|mrt|apr|may|mei|mai|jun|iun|jul|iul|aug|sep|oct|okt|nov|dec|ian)[a-zăâîșț]*\.?,?\s+(?:19|20)[0-9]{2}|(?i:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s+[0-3]?[0-9](?:st|nd|rd|th)?,?\s+(?:19|20)[0-9]{2})\b")?,
            // The place a photo was taken (written by the PC scan), or a map link to an exact spot
            d(Kind::Location, r"\[Photo location: -?[0-9]{1,2}\.[0-9]{3,6}, -?[0-9]{1,3}\.[0-9]{3,6}\]")?,
            d(Kind::Location, r"(?i)\b(?:maps\.google\.[a-z.]+|google\.[a-z.]+/maps|maps\.apple\.com|openstreetmap\.org)/?\S*?[@=/#](?:map=[0-9]{1,2}/)?-?[0-9]{1,2}\.[0-9]{4,}[,/]-?[0-9]{1,3}\.[0-9]{4,}")?,
            // Dutch number plates in their official dash patterns (no vowels since 1978), or after the word
            d(Kind::NumberPlate, r"\b(?:[A-Z]{2}-[0-9]{2}-[A-Z]{2}|[A-Z]{2}-[A-Z]{2}-[0-9]{2}|[0-9]{2}-[A-Z]{2}-[A-Z]{2}|[0-9]{2}-[A-Z]{3}-[0-9]|[0-9]-[A-Z]{3}-[0-9]{2}|[A-Z]{2}-[0-9]{3}-[A-Z]|[A-Z]-[0-9]{3}-[A-Z]{2}|[A-Z]{3}-[0-9]{2}-[A-Z]|[A-Z]-[0-9]{2}-[A-Z]{3}|[0-9]-[A-Z]{2}-[0-9]{3}|[0-9]{3}-[A-Z]{2}-[0-9])\b")?,
            d(Kind::NumberPlate, r"(?i:\b(?:kenteken|licen[cs]e plate|number plate|plate number|nummernschild|num[aă]r de [îi]nmatriculare)(?:\s+(?:is|nr|number))?\s*:?\s*)[A-Z0-9]{1,3}[- ]?[A-Z0-9]{2,4}[- ]?[A-Z0-9]{1,3}\b")?,
        ];

        let terms: Vec<String> = opts
            .watch_terms
            .iter()
            .map(|t| t.trim())
            .filter(|t| t.chars().count() >= 2)
            .map(|t| {
                let esc = regex::escape(t);
                let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                let pre = if word(t.chars().next()) { r"\b" } else { "" };
                let post = if word(t.chars().last()) { r"\b" } else { "" };
                format!("{pre}{esc}{post}")
            })
            .collect();
        let watch = if terms.is_empty() {
            None
        } else {
            Some(Regex::new(&format!("(?i)(?:{})", terms.join("|")))?)
        };
        Ok(Self { detectors, watch })
    }

    pub fn detect(&self, text: &str) -> Vec<Detection> {
        let mut found = Vec::new();
        if let Some(w) = &self.watch {
            found.extend(w.find_iter(text).map(|m| Detection { kind: Kind::WatchTerm, start: m.start(), end: m.end() }));
        }
        for (kind, re) in &self.detectors {
            for m in re.find_iter(text) {
                if validate(*kind, text, m.start(), m.end()) {
                    found.push(Detection { kind: *kind, start: m.start(), end: m.end() });
                }
            }
        }
        dedupe(found)
    }
}

/// Drops exact duplicates, and anything sitting inside a match that is at least as serious
/// (the digits inside an IBAN are not also a phone number).
fn dedupe(mut v: Vec<Detection>) -> Vec<Detection> {
    v.sort_by_key(|d| (d.start, std::cmp::Reverse(d.end), std::cmp::Reverse(d.kind.severity())));
    v.dedup_by(|a, b| a.kind == b.kind && a.start == b.start && a.end == b.end);
    let snapshot = v.clone();
    v.retain(|d| {
        !snapshot.iter().any(|o| {
            o != d
                && o.start <= d.start
                && o.end >= d.end
                && (o.end - o.start) > (d.end - d.start)
                && o.kind.severity() >= d.kind.severity()
        })
    });
    v
}

fn digits(s: &str) -> Vec<u32> {
    s.chars().filter_map(|c| c.to_digit(10)).collect()
}

fn luhn(ds: &[u32]) -> bool {
    let sum: u32 = ds
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| if i % 2 == 1 { let x = d * 2; if x > 9 { x - 9 } else { x } } else { d })
        .sum();
    sum % 10 == 0
}

fn iban_valid(s: &str) -> bool {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !(15..=34).contains(&compact.len()) {
        return false;
    }
    let rearranged = format!("{}{}", &compact[4..], &compact[..4]);
    let mut rem: u64 = 0;
    for c in rearranged.chars() {
        let v = match c {
            '0'..='9' => c as u64 - '0' as u64,
            'A'..='Z' => c as u64 - 'A' as u64 + 10,
            _ => return false,
        };
        rem = if v >= 10 { (rem * 100 + v) % 97 } else { (rem * 10 + v) % 97 };
    }
    rem == 1
}

fn validate(kind: Kind, text: &str, start: usize, end: usize) -> bool {
    let m = &text[start..end];
    let before = &text[..start];
    let after = &text[end..];
    match kind {
        Kind::Card => {
            // Discord mentions and links are full of long numbers; skip those.
            if before.ends_with("<@") || before.ends_with("<@!") || before.ends_with("<@&")
                || before.ends_with("<#") || before.ends_with(':') || before.ends_with('/')
                || after.starts_with('>')
            {
                return false;
            }
            let ds = digits(m);
            let separated = m.contains(' ') || m.contains('-');
            let len_ok = if separated { (13..=19).contains(&ds.len()) } else { ds.len() == 15 || ds.len() == 16 };
            let prefix: String = ds.iter().take(4).map(|d| char::from_digit(*d, 10).unwrap()).collect();
            let p2: u32 = prefix[..2].parse().unwrap_or(0);
            let p4: u32 = prefix.parse().unwrap_or(0);
            let brand = prefix.starts_with('4')
                || (51..=55).contains(&p2)
                || (2221..=2720).contains(&p4)
                || p2 == 34 || p2 == 37
                || p4 == 6011 || p2 == 65 || p2 == 35;
            len_ok && brand && luhn(&ds)
        }
        Kind::Iban => iban_valid(m),
        Kind::Phone => {
            let n = digits(m).len();
            let next_is_digit = after.chars().next().is_some_and(|c| c.is_ascii_digit());
            (9..=15).contains(&n) && !next_is_digit && !before.ends_with(['/', '=', '#'])
        }
        Kind::IpAddress => {
            if before.ends_with(['v', 'V', '.']) || after.starts_with('.') {
                return false; // version numbers like v1.2.3.4
            }
            let o: Vec<u32> = m.split('.').filter_map(|p| p.parse().ok()).collect();
            !(o[0] == 0 || o[0] == 10 || o[0] == 127 || o[0] >= 224
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 169 && o[1] == 254))
        }
        Kind::Postcode => {
            let letters = &m[m.len() - 2..];
            !matches!(letters, "SA" | "SD" | "SS" | "AM" | "PM" | "GB" | "MB" | "KB" | "TB" | "HZ")
                && !after.starts_with(|c: char| c.is_alphanumeric())
        }
        Kind::IdNumber => {
            let ds = digits(m);
            if m.contains('-') {
                // US SSN: never 000, 666 or 9xx in the first part, 00 or 0000 in the others
                let area = ds[0] * 100 + ds[1] * 10 + ds[2];
                return area != 0 && area != 666 && !(ds[3] == 0 && ds[4] == 0) && ds[5..].iter().any(|d| *d != 0)
                    && !before.ends_with(|c: char| c.is_ascii_digit() || c == '-') && !after.starts_with('-');
            }
            match ds.len() {
                9 => elfproef(&ds),
                13 => cnp_valid(&ds),
                _ => false,
            }
        }
        Kind::Passport => {
            let tail = m.rsplit(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or("");
            tail.chars().filter(|c| c.is_ascii_digit()).count() >= 2
        }
        Kind::BackupCode => {
            if m.to_lowercase().contains("phrase") || m.to_lowercase().contains("words") || m.to_lowercase().contains("zin") {
                // A real recovery phrase is only word-list words, never everyday ones like "the" or "you"
                const NOT_SEED: [&str; 14] = ["the", "and", "you", "your", "should", "with", "this", "that", "for", "are", "was", "have", "what", "don"];
                let words: Vec<&str> = m.split_whitespace().collect();
                let tail = &words[words.len().saturating_sub(12)..];
                return !tail.iter().any(|w| NOT_SEED.contains(&w.to_lowercase().as_str()));
            }
            let code = m.rsplit(|c: char| c.is_whitespace() || c == ':').next().unwrap_or("");
            code.chars().any(|c| c.is_ascii_digit()) && code.chars().any(|c| c.is_ascii_alphabetic())
                || code.chars().filter(|c| c.is_ascii_digit()).count() >= 8
        }
        Kind::NumberPlate => {
            // The plate itself: everything after the last lowercase letter or colon of the keyword
            let cut = m.char_indices().filter(|(_, c)| c.is_lowercase() || *c == ':').last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
            let tail = &m[cut..];
            let letters = tail.chars().filter(|c| c.is_ascii_alphabetic()).count();
            let nums = tail.chars().filter(|c| c.is_ascii_digit()).count();
            let keyword = m.chars().any(|c| c.is_lowercase());
            letters >= 1 && nums >= 1 && (5..=8).contains(&(letters + nums))
                && (keyword || !tail.chars().any(|c| "AEIOU".contains(c)))
                && !before.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
                && !after.starts_with('-')
        }
        _ => true,
    }
}

/// The Dutch eleven test for a BSN: 9·d1 + 8·d2 + … + 2·d8 − d9 is a multiple of 11.
fn elfproef(ds: &[u32]) -> bool {
    if ds.len() != 9 || ds.iter().all(|d| *d == ds[0]) {
        return false;
    }
    let sum: i64 = ds[..8].iter().enumerate().map(|(i, d)| (9 - i as i64) * *d as i64).sum::<i64>() - ds[8] as i64;
    sum % 11 == 0
}

/// Romanian personal numeric code: sex/century digit, a real date, and the check digit.
fn cnp_valid(ds: &[u32]) -> bool {
    if ds.len() != 13 {
        return false;
    }
    let month = ds[3] * 10 + ds[4];
    let day = ds[5] * 10 + ds[6];
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }
    const W: [u32; 12] = [2, 7, 9, 1, 4, 6, 3, 5, 8, 2, 7, 9];
    let r = ds[..12].iter().zip(W).map(|(d, w)| d * w).sum::<u32>() % 11;
    ds[12] == if r == 10 { 1 } else { r }
}


// ---------------------------------------------------------------------------------
// Masking helpers (shared with query.rs)

/// Hides a sensitive span but keeps the first character of each word, so it stays recognisable:
/// "jan@mail.com" -> "j••@m•••.c••".
pub(crate) fn mask_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_alnum = false;
    for ch in s.chars() {
        out.push(if ch.is_alphanumeric() && prev_alnum { '•' } else { ch });
        prev_alnum = ch.is_alphanumeric();
    }
    out
}

/// Sorted, merged byte ranges that are safe to slice on.
pub(crate) fn merge_spans(content: &str, mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    spans.retain(|&(s, e)| s < e && e <= content.len() && content.is_char_boundary(s) && content.is_char_boundary(e));
    spans.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// Normalised form used for "never flag this again" fingerprints, so
/// "06-12345678" and "06 1234 5678" count as the same number.
fn normalize(kind: Kind, value: &str) -> String {
    match kind {
        Kind::Phone | Kind::Card | Kind::Iban | Kind::Postcode | Kind::IdNumber | Kind::NumberPlate => {
            value.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
        }
        _ => value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase(),
    }
}

fn fingerprint(kind: Kind, value: &str) -> String {
    let mut h = Sha256::new();
    h.update(kind.key().as_bytes());
    h.update([0]);
    h.update(normalize(kind, value).as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------------
// Files from the PC scan: the first line is the file's path, the rest its text.

fn word_at(hay: &str, i: usize, len: usize) -> bool {
    let before = hay[..i].chars().next_back();
    let after = hay[i + len..].chars().next();
    !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
}

/// Flags a file by its name: identity documents and money matters, or key files.
fn detect_file_name(path_line: &str) -> Vec<Detection> {
    let lower = path_line.to_lowercase();
    // Only the file's own name counts, not the folders it sits in.
    let name_start = path_line.rfind('/').map(|i| i + 1).unwrap_or(0);
    let name = path_line[name_start..].to_lowercase();
    if crate::pcscan::is_key_file(&lower) {
        return vec![Detection { kind: Kind::Credential, start: name_start, end: path_line.len() }];
    }
    // Treat _ - . as spaces so "Passport_scan.jpg" and "bank-statement.pdf" match.
    let spaced: String = name.chars().map(|c| if matches!(c, '_' | '-' | '.') { ' ' } else { c }).collect();
    for word in crate::pcscan::SENSITIVE_NAME_WORDS {
        let mut from = 0;
        while let Some(pos) = spaced[from..].find(word) {
            let i = from + pos;
            if word_at(&spaced, i, word.len()) {
                // The whole file name is the finding, so "never flag this again" applies
                // to this file only, not to every file with the same word in its name.
                return vec![Detection { kind: Kind::SensitiveFile, start: name_start, end: path_line.len() }];
            }
            from = i + word.len();
        }
    }
    vec![]
}

/// What to scan in a message: the whole text, or for PC files the name and the body separately.
fn detect_message(scanner: &Scanner, platform: &str, content: &str) -> Vec<Detection> {
    if platform != "pc" {
        return scanner.detect(content);
    }
    let first_end = content.find('\n').unwrap_or(content.len());
    let mut found = detect_file_name(&content[..first_end]);
    let body_start = content.find("\n\n").map(|i| i + 2).unwrap_or(content.len());
    found.extend(scanner.detect(&content[body_start..]).into_iter().map(|d| Detection {
        start: d.start + body_start,
        end: d.end + body_start,
        ..d
    }));
    found
}

// ---------------------------------------------------------------------------------
// Running a scan and reading results

#[derive(Debug, Clone, Serialize)]
pub struct KindCount {
    pub kind: Kind,
    pub label: &'static str,
    pub severity: i64,
    pub advice: &'static str,
    pub count: i64,
    /// findings still to review (not decided, or marked for later)
    pub open: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanSummary {
    pub scanned: i64,
    pub flagged_messages: i64,
    pub total_findings: i64,
    /// findings with no decision yet
    pub open: i64,
    pub later: i64,
    /// marked deleted or not a problem
    pub done: i64,
    pub ignored_rules: i64,
    pub scanned_at_ms: Option<i64>,
    pub by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingView {
    pub id: i64,
    pub message_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub platform: String,
    pub ts: i64,
    pub kind: Kind,
    pub label: &'static str,
    pub severity: i64,
    pub advice: &'static str,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub masked: bool,
    /// deleted | ok | later, or None when not decided yet
    pub status: Option<String>,
    /// Where to open it on its platform, when it has a web address
    pub link: Option<String>,
    /// For files found on this PC: the full path
    pub file_path: Option<String>,
    /// How many findings in the same list have this exact value (1 = only here)
    pub repeats: i64,
}

/// Every place one value was found, for acting on all of them at once.
#[derive(Debug, Clone, Serialize)]
pub struct GroupInfo {
    pub ids: Vec<i64>,
    /// The message each finding is in, so a message is only deleted once
    pub message_ids: Vec<String>,
    /// The platform of each finding, in the same order as `ids`
    pub member_platforms: Vec<String>,
    pub platforms: Vec<GroupPlatform>,
    pub first_ts: i64,
    pub last_ts: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupPlatform {
    pub platform: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingsPage {
    pub total: i64,
    pub offset: i64,
    pub items: Vec<FindingView>,
}

/// Scans every imported message. Replaces the results of any previous scan;
/// decisions the user already made are kept.
pub fn run_scan(
    store: &mut Store,
    opts: &ScanOptions,
    progress: &mut dyn FnMut(i64, i64),
) -> Result<ScanSummary> {
    let scanner = Scanner::new(opts)?;
    store.set_meta("watch_terms", &serde_json::to_string(&opts.watch_terms)?)?;
    let total: i64 = store.conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
    let ignored: std::collections::HashSet<(String, String)> = {
        let mut stmt = store.conn.prepare("SELECT kind, fingerprint FROM ignored")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };

    let tx = store.conn.transaction()?;
    tx.execute("DELETE FROM findings", [])?;
    {
        let mut read = tx.prepare(
            "SELECT m.id, m.content, c.platform FROM messages m JOIN channels c ON c.id = m.channel_id WHERE m.content <> ''",
        )?;
        let mut write = tx.prepare(
            "INSERT INTO findings(message_id, kind, severity, start_byte, end_byte, fp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let mut rows = read.query([])?;
        let mut done = 0i64;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let content: String = row.get(1)?;
            let platform: String = row.get(2)?;
            for d in detect_message(&scanner, &platform, &content) {
                let fp = fingerprint(d.kind, &content[d.start..d.end]);
                if ignored.contains(&(d.kind.key().to_string(), fp.clone())) {
                    continue;
                }
                write.execute(params![id, d.kind.key(), d.kind.severity(), d.start as i64, d.end as i64, fp])?;
            }
            done += 1;
            if done % 5000 == 0 {
                progress(done, total);
            }
        }
        progress(total, total);
    }
    tx.commit()?;
    store.set_meta("scanned_at_ms", &crate::now_ms().to_string())?;
    store.scan_summary()
}

/// Builds a short preview around the match. If `reveal` is false, every sensitive
/// span in the preview is masked, not just this one, so nothing leaks on screen.
fn snippet(content: &str, start: usize, end: usize, spans: &[(usize, usize)], reveal: bool) -> (String, String, String, bool) {
    const CONTEXT: usize = 70;
    let render = |from: usize, to: usize| -> String {
        if reveal {
            return content[from..to].to_string();
        }
        let mut out = String::new();
        let mut prev_alnum = false;
        for (i, ch) in content[from..to].char_indices() {
            let pos = from + i;
            let sensitive = spans.iter().any(|&(s, e)| pos >= s && pos < e);
            out.push(if sensitive && ch.is_alphanumeric() && prev_alnum { '•' } else { ch });
            prev_alnum = ch.is_alphanumeric();
        }
        out
    };
    let mut from = start;
    for (n, (i, _)) in content[..start].char_indices().rev().enumerate() {
        if n >= CONTEXT {
            break;
        }
        from = i;
    }
    let mut to = end;
    for (n, (i, ch)) in content[end..].char_indices().enumerate() {
        if n >= CONTEXT {
            break;
        }
        to = end + i + ch.len_utf8();
    }
    let mut before = render(from, start);
    let mut after = render(end, to);
    if from > 0 {
        before.insert(0, '…');
    }
    if to < content.len() {
        after.push('…');
    }
    (before.replace('\n', " "), render(start, end), after.replace('\n', " "), !reveal)
}

/// What to list on the Review screen.
#[derive(Debug, Clone)]
pub struct FindingsFilter<'a> {
    pub kind: Option<&'a str>,
    pub platform: Option<&'a str>,
    pub status: StatusFilter,
    /// One row per distinct value (the newest place it appears)
    pub group: bool,
    /// Only findings with the same value as this one
    pub same_as: Option<i64>,
    pub only_id: Option<i64>,
}

impl Default for FindingsFilter<'_> {
    fn default() -> Self {
        Self { kind: None, platform: None, status: StatusFilter::Open, group: false, same_as: None, only_id: None }
    }
}

/// Which findings to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    /// nothing decided yet
    Open,
    Later,
    /// deleted or not a problem
    Done,
    All,
}

impl StatusFilter {
    pub fn parse(s: &str) -> StatusFilter {
        match s {
            "later" => StatusFilter::Later,
            "done" => StatusFilter::Done,
            "all" => StatusFilter::All,
            _ => StatusFilter::Open,
        }
    }
    fn sql(self) -> &'static str {
        match self {
            StatusFilter::Open => "r.status IS NULL",
            StatusFilter::Later => "r.status = 'later'",
            StatusFilter::Done => "r.status IN ('deleted', 'ok')",
            StatusFilter::All => "1",
        }
    }
}

const FINDING_FROM: &str = "FROM findings f
     JOIN messages m ON m.id = f.message_id
     JOIN channels ch ON ch.id = m.channel_id
     LEFT JOIN reviews r ON r.uid = m.uid AND r.kind = f.kind AND r.start_byte = f.start_byte";

impl Store {
    pub fn watch_terms(&self) -> Result<Vec<String>> {
        Ok(self
            .get_meta("watch_terms")?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn scan_summary(&self) -> Result<ScanSummary> {
        let c = &self.conn;
        let one = |sql: &str| -> Result<i64> { Ok(c.query_row(sql, [], |r| r.get(0))?) };
        let mut stmt = c.prepare(&format!(
            "SELECT f.kind, COUNT(*), SUM(CASE WHEN r.status IS NULL OR r.status = 'later' THEN 1 ELSE 0 END)
             {FINDING_FROM} GROUP BY f.kind"
        ))?;
        let mut counts: HashMap<String, (i64, i64)> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))? {
            let (k, n, open) = row?;
            counts.insert(k, (n, open));
        }
        let by_kind = Kind::ALL
            .into_iter()
            .map(|k| {
                let (count, open) = counts.get(k.key()).copied().unwrap_or((0, 0));
                KindCount { kind: k, label: k.label(), severity: k.severity(), advice: k.advice(), count, open }
            })
            .collect();
        let status_count = |filter: StatusFilter| one(&format!("SELECT COUNT(*) {FINDING_FROM} WHERE {}", filter.sql()));
        Ok(ScanSummary {
            scanned: one("SELECT COUNT(*) FROM messages")?,
            flagged_messages: one("SELECT COUNT(DISTINCT message_id) FROM findings")?,
            total_findings: one("SELECT COUNT(*) FROM findings")?,
            open: status_count(StatusFilter::Open)?,
            later: status_count(StatusFilter::Later)?,
            done: status_count(StatusFilter::Done)?,
            ignored_rules: one("SELECT COUNT(*) FROM ignored")?,
            scanned_at_ms: self.get_meta("scanned_at_ms")?.and_then(|s| s.parse().ok()),
            by_kind,
        })
    }

    /// Findings, most serious and newest first. `kind` filters by Kind::key().
    pub fn findings(&self, kind: Option<&str>, status: StatusFilter, offset: i64, limit: i64, reveal: bool) -> Result<FindingsPage> {
        self.findings_filtered(&FindingsFilter { kind, status, ..Default::default() }, offset, limit, reveal)
    }

    pub fn findings_for_platform(
        &self,
        kind: Option<&str>,
        platform: Option<&str>,
        status: StatusFilter,
        offset: i64,
        limit: i64,
        reveal: bool,
    ) -> Result<FindingsPage> {
        self.findings_filtered(&FindingsFilter { kind, platform, status, ..Default::default() }, offset, limit, reveal)
    }

    /// One finding with its text shown in full (for revealing a single item).
    pub fn reveal_finding(&self, id: i64) -> Result<Option<FindingView>> {
        let f = FindingsFilter { status: StatusFilter::All, only_id: Some(id), ..Default::default() };
        Ok(self.findings_filtered(&f, 0, 1, true)?.items.pop())
    }

    pub fn findings_filtered(&self, q: &FindingsFilter, offset: i64, limit: i64, reveal: bool) -> Result<FindingsPage> {
        let c = &self.conn;
        // `base` numbers each value's findings, newest first, and counts them, so grouping
        // can keep one row per value and every row knows how often its value appears.
        let base = format!(
            "WITH base AS (
               SELECT f.id, f.message_id, m.channel_id, ch.name AS cname, ch.platform, m.ts, f.kind, f.severity,
                      f.start_byte, f.end_byte, m.content, r.status, m.link, m.attachments,
                      ROW_NUMBER() OVER (PARTITION BY f.kind, f.fp ORDER BY m.ts DESC, f.id) AS rn,
                      COUNT(*) OVER (PARTITION BY f.kind, f.fp) AS repeats
               {FINDING_FROM}
               WHERE (?1 IS NULL OR f.kind = ?1) AND (?5 IS NULL OR ch.platform = ?5) AND {}
                 AND (?7 IS NULL OR (f.kind, f.fp) = (SELECT kind, fp FROM findings WHERE id = ?7))
             )",
            q.status.sql()
        );
        let keep = "(?2 IS NULL OR id = ?2) AND (?6 = 0 OR rn = 1)";
        let group = q.group as i64;
        let (lim, off) = (limit.clamp(1, 1000), offset.max(0));
        let total: i64 = c.query_row(
            &format!("{base} SELECT COUNT(*) FROM base WHERE {keep} AND ?3 = ?3 AND ?4 = ?4"),
            params![q.kind, q.only_id, 0, 0, q.platform, group, q.same_as],
            |r| r.get(0),
        )?;
        let mut stmt = c.prepare_cached(&format!(
            "{base} SELECT id, message_id, channel_id, cname, platform, ts, kind, start_byte, end_byte, content, status, link,
                           attachments, repeats
             FROM base WHERE {keep}
             ORDER BY severity DESC, repeats * (?6) DESC, ts DESC, start_byte
             LIMIT ?3 OFFSET ?4"
        ))?;
        type Row = (i64, i64, i64, String, String, i64, String, usize, usize, String, Option<String>, Option<String>, String, i64);
        let rows: Vec<Row> = stmt
            .query_map(params![q.kind, q.only_id, lim, off, q.platform, group, q.same_as], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get::<_, i64>(7)? as usize,
                    r.get::<_, i64>(8)? as usize,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut items = Vec::with_capacity(rows.len());
        for (id, mid, cid, cname, platform, ts, k, start, end, content, status, link, attachments, repeats) in rows {
            let Some(kind) = Kind::from_key(&k) else { continue };
            if end > content.len() || !content.is_char_boundary(start) || !content.is_char_boundary(end) {
                continue;
            }
            let spans = self.spans_for(mid)?;
            let (before, matched, after, masked) = snippet(&content, start, end, &spans, reveal);
            let file_path = (platform == "pc").then_some(attachments);
            items.push(FindingView {
                id,
                message_id: mid.to_string(),
                channel_id: cid.to_string(),
                channel_name: cname,
                platform,
                ts,
                kind,
                label: kind.label(),
                severity: kind.severity(),
                advice: kind.advice(),
                before,
                matched,
                after,
                masked,
                status,
                link,
                file_path,
                repeats,
            });
        }
        Ok(FindingsPage { total, offset, items })
    }

    /// Every finding with the same value as `finding_id`, within one status list.
    pub fn group_info(&self, finding_id: i64, status: StatusFilter) -> Result<GroupInfo> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT f.id, f.message_id, ch.platform, m.ts {FINDING_FROM}
             WHERE (f.kind, f.fp) = (SELECT kind, fp FROM findings WHERE id = ?1) AND {}
             ORDER BY m.ts",
            status.sql()
        ))?;
        let rows: Vec<(i64, i64, String, i64)> =
            stmt.query_map([finding_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut platforms: Vec<GroupPlatform> = Vec::new();
        for (_, _, p, _) in &rows {
            match platforms.iter_mut().find(|x| &x.platform == p) {
                Some(x) => x.count += 1,
                None => platforms.push(GroupPlatform { platform: p.clone(), count: 1 }),
            }
        }
        platforms.sort_by(|a, b| b.count.cmp(&a.count));
        Ok(GroupInfo {
            first_ts: rows.first().map_or(0, |r| r.3),
            last_ts: rows.last().map_or(0, |r| r.3),
            ids: rows.iter().map(|r| r.0).collect(),
            message_ids: rows.iter().map(|r| r.1.to_string()).collect(),
            member_platforms: rows.iter().map(|r| r.2.clone()).collect(),
            platforms,
        })
    }

    /// Records one decision for many findings at once. Returns what each had before, for undo.
    pub fn set_status_many(&mut self, ids: &[i64], status: Option<&str>) -> Result<Vec<(i64, Option<String>)>> {
        if let Some(s) = status {
            anyhow::ensure!(matches!(s, "deleted" | "ok" | "later"), "unknown status {s}");
        }
        let tx = self.conn.transaction()?;
        let mut before = Vec::with_capacity(ids.len());
        {
            let mut get = tx.prepare_cached(
                "SELECT m.uid, f.kind, f.start_byte, r.status FROM findings f JOIN messages m ON m.id = f.message_id
                 LEFT JOIN reviews r ON r.uid = m.uid AND r.kind = f.kind AND r.start_byte = f.start_byte WHERE f.id = ?1",
            )?;
            let mut set = tx.prepare_cached(
                "INSERT INTO reviews(uid, kind, start_byte, status) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, kind, start_byte) DO UPDATE SET status = excluded.status",
            )?;
            let mut clear = tx.prepare_cached("DELETE FROM reviews WHERE uid = ?1 AND kind = ?2 AND start_byte = ?3")?;
            for &id in ids {
                let row: Option<(String, String, i64, Option<String>)> = get
                    .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                    .map(Some)
                    .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })?;
                let Some((uid, kind, start, prev)) = row else { continue };
                match status {
                    Some(s) => set.execute(params![uid, kind, start, s])?,
                    None => clear.execute(params![uid, kind, start])?,
                };
                before.push((id, prev));
            }
        }
        tx.commit()?;
        Ok(before)
    }

    /// Puts decisions back the way `set_status_many` found them.
    pub fn restore_statuses(&mut self, previous: &[(i64, Option<String>)]) -> Result<()> {
        for status in [None, Some("deleted"), Some("ok"), Some("later")] {
            let ids: Vec<i64> = previous.iter().filter(|(_, p)| p.as_deref() == status).map(|(id, _)| *id).collect();
            if !ids.is_empty() {
                self.set_status_many(&ids, status)?;
            }
        }
        Ok(())
    }

    /// Fills in value fingerprints for findings stored before they existed.
    pub(crate) fn backfill_fingerprints(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut read = tx.prepare("SELECT f.id, f.kind, f.start_byte, f.end_byte, m.content FROM findings f JOIN messages m ON m.id = f.message_id")?;
            let mut write = tx.prepare("UPDATE findings SET fp = ?2 WHERE id = ?1")?;
            let mut rows = read.query([])?;
            while let Some(r) = rows.next()? {
                let (id, k, s, e, content): (i64, String, i64, i64, String) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
                if let (Some(kind), Some(v)) = (Kind::from_key(&k), content.get(s as usize..e as usize)) {
                    write.execute(params![id, fingerprint(kind, v)])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn spans_for(&self, message_id: i64) -> Result<Vec<(usize, usize)>> {
        let mut stmt = self.conn.prepare_cached("SELECT start_byte, end_byte FROM findings WHERE message_id = ?1")?;
        let spans = stmt
            .query_map([message_id], |r| Ok((r.get::<_, i64>(0)? as usize, r.get::<_, i64>(1)? as usize)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(spans)
    }

    /// Records a decision for one finding: "deleted", "ok", "later", or None to undo.
    pub fn set_status(&self, finding_id: i64, status: Option<&str>) -> Result<()> {
        if let Some(s) = status {
            anyhow::ensure!(matches!(s, "deleted" | "ok" | "later"), "unknown status {s}");
        }
        let (uid, kind, start): (String, String, i64) = self.conn.query_row(
            "SELECT m.uid, f.kind, f.start_byte FROM findings f JOIN messages m ON m.id = f.message_id WHERE f.id = ?1",
            [finding_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        match status {
            Some(s) => self.conn.execute(
                "INSERT INTO reviews(uid, kind, start_byte, status) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, kind, start_byte) DO UPDATE SET status = excluded.status",
                params![uid, kind, start, s],
            )?,
            None => self.conn.execute(
                "DELETE FROM reviews WHERE uid = ?1 AND kind = ?2 AND start_byte = ?3",
                params![uid, kind, start],
            )?,
        };
        Ok(())
    }

    /// Moves a file found by the PC scan to the Recycle Bin, and marks everything
    /// found in it as deleted. Returns the path that was moved.
    pub fn recycle_file(&self, finding_id: i64) -> Result<String> {
        let (message_id, platform, path): (i64, String, String) = self.conn.query_row(
            "SELECT m.id, c.platform, m.attachments FROM findings f
             JOIN messages m ON m.id = f.message_id JOIN channels c ON c.id = m.channel_id WHERE f.id = ?1",
            [finding_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        anyhow::ensure!(platform == "pc", "only files on this PC can be moved to the Recycle Bin");
        crate::pcscan::move_to_recycle_bin(&path)?;
        self.conn.execute(
            "INSERT INTO reviews(uid, kind, start_byte, status)
             SELECT m.uid, f.kind, f.start_byte, 'deleted' FROM findings f JOIN messages m ON m.id = f.message_id
             WHERE f.message_id = ?1
             ON CONFLICT(uid, kind, start_byte) DO UPDATE SET status = 'deleted'",
            [message_id],
        )?;
        Ok(path)
    }

    /// What a finding points at, for deleting it on its platform: (platform, message uid).
    pub fn delete_target(&self, finding_id: i64) -> Result<(String, String)> {
        Ok(self.conn.query_row(
            "SELECT c.platform, m.uid FROM findings f
             JOIN messages m ON m.id = f.message_id JOIN channels c ON c.id = m.channel_id WHERE f.id = ?1",
            [finding_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    /// After a message was deleted on its platform: marks everything found in it as deleted.
    pub fn mark_deleted(&self, finding_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO reviews(uid, kind, start_byte, status)
             SELECT m.uid, f.kind, f.start_byte, 'deleted' FROM findings f JOIN messages m ON m.id = f.message_id
             WHERE f.message_id = (SELECT message_id FROM findings WHERE id = ?1)
             ON CONFLICT(uid, kind, start_byte) DO UPDATE SET status = 'deleted'",
            [finding_id],
        )?;
        Ok(())
    }

    /// The full path of a PC-scan file a finding is in, for "Show in folder".
    pub fn file_path_for(&self, finding_id: i64) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT CASE WHEN c.platform = 'pc' THEN m.attachments END FROM findings f
             JOIN messages m ON m.id = f.message_id JOIN channels c ON c.id = m.channel_id WHERE f.id = ?1",
            [finding_id],
            |r| r.get(0),
        )?)
    }

    /// "Never flag this again": stops flagging this exact value (say, a public work email)
    /// now and in future scans. Returns how many findings were cleared.
    pub fn ignore_everywhere(&mut self, finding_id: i64) -> Result<usize> {
        let (kind_key, start, end, content): (String, usize, usize, String) = self.conn.query_row(
            "SELECT f.kind, f.start_byte, f.end_byte, m.content FROM findings f JOIN messages m ON m.id = f.message_id WHERE f.id = ?1",
            [finding_id],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize, r.get::<_, i64>(2)? as usize, r.get(3)?)),
        )?;
        let kind = Kind::from_key(&kind_key).ok_or_else(|| anyhow::anyhow!("unknown kind"))?;
        let value = content.get(start..end).ok_or_else(|| anyhow::anyhow!("finding no longer matches its message"))?;
        let fp = fingerprint(kind, value);

        let tx = self.conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO ignored(kind, fingerprint) VALUES (?1, ?2)", params![kind_key, fp])?;
        let doomed: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT f.id, f.start_byte, f.end_byte, m.content FROM findings f JOIN messages m ON m.id = f.message_id WHERE f.kind = ?1",
            )?;
            let rows = stmt.query_map([&kind_key], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize, r.get::<_, i64>(2)? as usize, r.get::<_, String>(3)?))
            })?;
            let mut ids = Vec::new();
            for row in rows {
                let (id, s, e, text) = row?;
                if text.get(s..e).is_some_and(|v| fingerprint(kind, v) == fp) {
                    ids.push(id);
                }
            }
            ids
        };
        {
            let mut del = tx.prepare("DELETE FROM findings WHERE id = ?1")?;
            for id in &doomed {
                del.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(doomed.len())
    }

    /// Forgets every "never flag this again" rule. The items come back on the next scan.
    pub fn clear_ignored(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM ignored", [])?)
    }

    /// Writes a CSV of flagged messages with no message text in it: IDs, dates, status
    /// and where to find each one (a web link, or a file path for files on this PC).
    pub fn export_flagged_csv(&self, path: &std::path::Path) -> Result<usize> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT ch.platform, ch.name, m.uid, m.ts, MAX(f.severity), GROUP_CONCAT(DISTINCT f.kind),
                    CASE WHEN SUM(r.status IS NULL) > 0 THEN 'to review'
                         WHEN SUM(r.status = 'later') > 0 THEN 'later' ELSE 'done' END,
                    COALESCE(m.link, CASE WHEN ch.platform = 'pc' THEN m.attachments END, '')
             {FINDING_FROM}
             GROUP BY f.message_id ORDER BY ch.platform, ch.name, m.ts"
        ))?;
        let mut w = csv::Writer::from_path(path)?;
        w.write_record(["platform", "where", "id", "sent_at_utc_ms", "severity", "kinds", "status", "link_or_path"])?;
        let mut n = 0;
        let rows = stmt.query_map([], |r| {
            Ok([
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?.to_string(),
                r.get::<_, i64>(4)?.to_string(),
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ])
        })?;
        for row in rows {
            w.write_record(row?)?;
            n += 1;
        }
        w.flush()?;
        Ok(n)
    }
}
