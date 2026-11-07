//! The exposure scanner: finds personal information you have posted.
//!
//! Every detector is a regex plus, where possible, a real validity check
//! (IBAN checksum, Luhn for cards, digit counts for phone numbers) to keep
//! false alarms low. Results are stored as byte positions only.

use crate::{snowflake_to_ms, Store};
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

    /// What the person should do about it, in plain words.
    pub fn advice(self) -> &'static str {
        match self {
            Kind::Credential => "Change this password or revoke this token now. Deleting the message doesn't undo the leak, because others may already have seen it.",
            Kind::Card => "Contact your bank to block or replace this card, then delete the message.",
            Kind::Iban => "An IBAN alone can't empty your account, but scammers use it to sound convincing. Delete the message unless you share this account number publicly anyway.",
            Kind::WatchTerm => "You told Veil this is sensitive. Decide whether this message needs to go.",
            Kind::Address => "Delete the message unless you're fine with everyone in this chat knowing this address.",
            Kind::Phone => "Delete the message if you don't want this number linked to your Discord account.",
            Kind::Email => "Check whether you want this address linked to your Discord account. If it's a public work address, mark it as not a problem everywhere.",
            Kind::IpAddress => "An IP address can show roughly where you are. Delete the message if this was your home connection.",
            Kind::Postcode => "A postcode narrows down where you live. Low risk on its own, more so next to a street name.",
            Kind::CryptoWallet => "Wallet addresses are public, but tying one to your account reveals its balance and history.",
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

/// Normalised form used for "not a problem anywhere" fingerprints, so
/// "06-12345678" and "06 1234 5678" count as the same number.
fn normalize(kind: Kind, value: &str) -> String {
    match kind {
        Kind::Phone | Kind::Card | Kind::Iban | Kind::Postcode => {
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
    /// Link that opens this exact message in Discord
    pub discord_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingsPage {
    pub total: i64,
    pub offset: i64,
    pub items: Vec<FindingView>,
}

pub(crate) fn discord_url(guild_id: Option<i64>, channel_id: i64, message_id: i64) -> String {
    let guild = guild_id.map(|g| g.to_string()).unwrap_or_else(|| "@me".to_string());
    format!("https://discord.com/channels/{guild}/{channel_id}/{message_id}")
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
                if !ignored.is_empty()
                    && ignored.contains(&(d.kind.key().to_string(), fingerprint(d.kind, &content[d.start..d.end])))
                {
                    continue;
                }
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
     LEFT JOIN channels ch ON ch.id = m.channel_id
     LEFT JOIN reviews r ON r.message_id = f.message_id AND r.kind = f.kind AND r.start_byte = f.start_byte";

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
    pub fn findings(
        &self,
        kind: Option<&str>,
        status: StatusFilter,
        offset: i64,
        limit: i64,
        reveal: bool,
    ) -> Result<FindingsPage> {
        self.findings_inner(kind, status, None, offset, limit, reveal)
    }

    /// One finding with its text shown in full (for revealing a single item).
    pub fn reveal_finding(&self, id: i64) -> Result<Option<FindingView>> {
        Ok(self.findings_inner(None, StatusFilter::All, Some(id), 0, 1, true)?.items.pop())
    }

    fn findings_inner(
        &self,
        kind: Option<&str>,
        status: StatusFilter,
        only_id: Option<i64>,
        offset: i64,
        limit: i64,
        reveal: bool,
    ) -> Result<FindingsPage> {
        let c = &self.conn;
        let filter = format!("(?1 IS NULL OR f.kind = ?1) AND (?2 IS NULL OR f.id = ?2) AND {}", status.sql());
        let total: i64 = c.query_row(
            &format!("SELECT COUNT(*) {FINDING_FROM} WHERE {filter}"),
            params![kind, only_id],
            |r| r.get(0),
        )?;
        let mut stmt = c.prepare_cached(&format!(
            "SELECT f.id, f.message_id, m.channel_id, COALESCE(ch.name, ''), ch.guild_id, m.ts, f.kind,
                    f.start_byte, f.end_byte, m.content, r.status
             {FINDING_FROM}
             WHERE {filter}
             ORDER BY f.severity DESC, m.ts DESC, f.start_byte
             LIMIT ?3 OFFSET ?4"
        ))?;
        type Row = (i64, i64, i64, String, Option<i64>, i64, String, usize, usize, String, Option<String>);
        let rows: Vec<Row> = stmt
            .query_map(params![kind, only_id, limit.clamp(1, 1000), offset.max(0)], |r| {
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
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut items = Vec::with_capacity(rows.len());
        for (id, mid, cid, cname, guild, ts, k, start, end, content, status) in rows {
            let Some(kind) = Kind::from_key(&k) else { continue };
            if end > content.len() || !content.is_char_boundary(start) || !content.is_char_boundary(end) {
                continue;
            }
            let spans = self.spans_for(mid)?;
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
                advice: kind.advice(),
                before,
                matched,
                after,
                masked,
                status,
                discord_url: discord_url(guild, cid, mid),
            });
        }
        Ok(FindingsPage { total, offset, items })
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
        let (mid, kind, start): (i64, String, i64) = self.conn.query_row(
            "SELECT message_id, kind, start_byte FROM findings WHERE id = ?1",
            [finding_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        match status {
            Some(s) => self.conn.execute(
                "INSERT INTO reviews(message_id, kind, start_byte, status) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(message_id, kind, start_byte) DO UPDATE SET status = excluded.status",
                params![mid, kind, start, s],
            )?,
            None => self.conn.execute(
                "DELETE FROM reviews WHERE message_id = ?1 AND kind = ?2 AND start_byte = ?3",
                params![mid, kind, start],
            )?,
        };
        Ok(())
    }

    /// "Not a problem anywhere": stops flagging this exact value (say, a public work email)
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

    /// Forgets every "not a problem anywhere" rule. The items come back on the next scan.
    pub fn clear_ignored(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM ignored", [])?)
    }

    /// Writes a CSV of flagged messages with no message content in it.
    /// Includes a link to each message so it can be opened and deleted by hand.
    pub fn export_flagged_csv(&self, path: &std::path::Path) -> Result<usize> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT m.channel_id, f.message_id, ch.guild_id, MAX(f.severity), GROUP_CONCAT(DISTINCT f.kind),
                    CASE WHEN SUM(r.status IS NULL) > 0 THEN 'to review'
                         WHEN SUM(r.status = 'later') > 0 THEN 'later' ELSE 'done' END
             {FINDING_FROM}
             GROUP BY f.message_id ORDER BY m.channel_id, f.message_id"
        ))?;
        let mut w = csv::Writer::from_path(path)?;
        w.write_record(["channel_id", "message_id", "sent_at_utc_ms", "severity", "kinds", "status", "discord_link"])?;
        let mut n = 0;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        for row in rows {
            let (cid, mid, guild, sev, kinds, status) = row?;
            w.write_record([
                cid.to_string(),
                mid.to_string(),
                snowflake_to_ms(mid).to_string(),
                sev.to_string(),
                kinds,
                status,
                discord_url(guild, cid, mid),
            ])?;
            n += 1;
        }
        w.flush()?;
        Ok(n)
    }
}
