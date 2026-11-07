//! The exposure scanner: finds personal information you have posted.
//!
//! Every detector is a regex plus, where possible, a real validity check
//! (IBAN checksum, Luhn for cards, digit counts for phone numbers) to keep
//! false alarms low. Results are stored as byte positions only.

use crate::{snowflake_to_ms, Store};
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
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Credential,
        Kind::Card,
        Kind::Iban,
        Kind::WatchTerm,
        Kind::Address,
        Kind::Phone,
        Kind::Email,
        Kind::IpAddress,
        Kind::Postcode,
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
        }
    }

    /// 3 = high, 2 = medium, 1 = low
    pub fn severity(self) -> i64 {
        match self {
            Kind::Credential | Kind::Card | Kind::Iban | Kind::WatchTerm | Kind::Address => 3,
            Kind::Phone | Kind::Email | Kind::IpAddress => 2,
            Kind::Postcode | Kind::CryptoWallet => 1,
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
        _ => true,
    }
}

// ---------------------------------------------------------------------------------
// Running a scan and reading results

#[derive(Debug, Clone, Serialize)]
pub struct KindCount {
    pub kind: Kind,
    pub label: &'static str,
    pub severity: i64,
    pub count: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanSummary {
    pub scanned: i64,
    pub flagged_messages: i64,
    pub total_findings: i64,
    pub scanned_at_ms: Option<i64>,
    pub by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingView {
    pub id: i64,
    pub message_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub ts: i64,
    pub kind: Kind,
    pub label: &'static str,
    pub severity: i64,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub masked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingsPage {
    pub total: i64,
    pub offset: i64,
    pub items: Vec<FindingView>,
}

/// Scans every imported message. Replaces the results of any previous scan.
pub fn run_scan(
    store: &mut Store,
    opts: &ScanOptions,
    progress: &mut dyn FnMut(i64, i64),
) -> Result<ScanSummary> {
    let scanner = Scanner::new(opts)?;
    store.set_meta("watch_terms", &serde_json::to_string(&opts.watch_terms)?)?;
    let total: i64 = store.conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;

    let tx = store.conn.transaction()?;
    tx.execute("DELETE FROM findings", [])?;
    {
        let mut read = tx.prepare("SELECT id, content FROM messages WHERE content <> ''")?;
        let mut write = tx.prepare(
            "INSERT INTO findings(message_id, kind, severity, start_byte, end_byte) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut rows = read.query([])?;
        let mut done = 0i64;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let content: String = row.get(1)?;
            for d in scanner.detect(&content) {
                write.execute(params![id, d.kind.key(), d.kind.severity(), d.start as i64, d.end as i64])?;
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
    const CONTEXT: usize = 60;
    let render = |from: usize, to: usize| -> String {
        let mut out = String::new();
        let mut prev_alnum = false;
        for (i, ch) in content[from..to].char_indices() {
            let pos = from + i;
            let sensitive = !reveal && spans.iter().any(|&(s, e)| pos >= s && pos < e);
            if sensitive && ch.is_alphanumeric() {
                // keep the first character of each word so it stays recognisable
                out.push(if prev_alnum { '•' } else { ch });
            } else {
                out.push(ch);
            }
            prev_alnum = ch.is_alphanumeric();
        }
        out
    };
    let mut from = start;
    for (n, (i, _)) in content[..start].char_indices().rev().enumerate() {
        if n >= CONTEXT { break; }
        from = i;
    }
    let mut to = end;
    for (n, (i, ch)) in content[end..].char_indices().enumerate() {
        if n >= CONTEXT { break; }
        to = end + i + ch.len_utf8();
    }
    let mut before = render(from, start);
    let mut after = render(end, to);
    if from > 0 { before.insert(0, '…'); }
    if to < content.len() { after.push('…'); }
    (before.replace('\n', " "), render(start, end), after.replace('\n', " "), !reveal)
}

impl Store {
    pub fn watch_terms(&self) -> Result<Vec<String>> {
        Ok(self
            .get_meta("watch_terms")?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn scan_summary(&self) -> Result<ScanSummary> {
        let c = &self.conn;
        let scanned: i64 = c.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        let flagged: i64 = c.query_row("SELECT COUNT(DISTINCT message_id) FROM findings", [], |r| r.get(0))?;
        let total: i64 = c.query_row("SELECT COUNT(*) FROM findings", [], |r| r.get(0))?;
        let mut stmt = c.prepare("SELECT kind, COUNT(*), COUNT(DISTINCT message_id) FROM findings GROUP BY kind")?;
        let mut counts: HashMap<String, (i64, i64)> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))? {
            let (k, n, m) = row?;
            counts.insert(k, (n, m));
        }
        let by_kind = Kind::ALL
            .into_iter()
            .map(|k| {
                let (count, messages) = counts.get(k.key()).copied().unwrap_or((0, 0));
                KindCount { kind: k, label: k.label(), severity: k.severity(), count, messages }
            })
            .collect();
        Ok(ScanSummary {
            scanned,
            flagged_messages: flagged,
            total_findings: total,
            scanned_at_ms: self.get_meta("scanned_at_ms")?.and_then(|s| s.parse().ok()),
            by_kind,
        })
    }

    /// Findings, most serious and newest first. `kind` filters by Kind::key().
    pub fn findings(&self, kind: Option<&str>, offset: i64, limit: i64, reveal: bool) -> Result<FindingsPage> {
        let c = &self.conn;
        let total: i64 = c.query_row(
            "SELECT COUNT(*) FROM findings WHERE ?1 IS NULL OR kind = ?1",
            [kind],
            |r| r.get(0),
        )?;
        let mut stmt = c.prepare_cached(
            "SELECT f.id, f.message_id, m.channel_id, COALESCE(ch.name, ''), m.ts, f.kind, f.start_byte, f.end_byte, m.content
             FROM findings f
             JOIN messages m ON m.id = f.message_id
             LEFT JOIN channels ch ON ch.id = m.channel_id
             WHERE ?1 IS NULL OR f.kind = ?1
             ORDER BY f.severity DESC, m.ts DESC, f.start_byte
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt
            .query_map(params![kind, limit.clamp(1, 1000), offset.max(0)], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)? as usize,
                    r.get::<_, i64>(7)? as usize,
                    r.get::<_, String>(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut spans_stmt = c.prepare_cached("SELECT start_byte, end_byte FROM findings WHERE message_id = ?1")?;
        let mut items = Vec::with_capacity(rows.len());
        for (id, mid, cid, cname, ts, k, start, end, content) in rows {
            let Some(kind) = Kind::from_key(&k) else { continue };
            if end > content.len() || !content.is_char_boundary(start) || !content.is_char_boundary(end) {
                continue;
            }
            let spans: Vec<(usize, usize)> = spans_stmt
                .query_map([mid], |r| Ok((r.get::<_, i64>(0)? as usize, r.get::<_, i64>(1)? as usize)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let (before, matched, after, masked) = snippet(&content, start, end, &spans, reveal);
            items.push(FindingView {
                id,
                message_id: mid.to_string(),
                channel_id: cid.to_string(),
                channel_name: cname,
                ts,
                kind,
                label: kind.label(),
                severity: kind.severity(),
                before,
                matched,
                after,
                masked,
            });
        }
        Ok(FindingsPage { total, offset, items })
    }

    /// Writes a CSV of flagged message IDs, with no message content in it.
    /// This is the hand-off list for a later clean-up step.
    pub fn export_flagged_csv(&self, path: &std::path::Path) -> Result<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT m.channel_id, f.message_id, MAX(f.severity), GROUP_CONCAT(DISTINCT f.kind)
             FROM findings f JOIN messages m ON m.id = f.message_id
             GROUP BY f.message_id ORDER BY m.channel_id, f.message_id",
        )?;
        let mut w = csv::Writer::from_path(path)?;
        w.write_record(["channel_id", "message_id", "sent_at_utc_ms", "severity", "kinds"])?;
        let mut n = 0;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?))
        })?;
        for row in rows {
            let (cid, mid, sev, kinds) = row?;
            w.write_record([cid.to_string(), mid.to_string(), snowflake_to_ms(mid).to_string(), sev.to_string(), kinds])?;
            n += 1;
        }
        w.flush()?;
        Ok(n)
    }
}
