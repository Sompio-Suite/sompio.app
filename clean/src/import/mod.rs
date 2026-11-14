//! Reads data exports from each platform, as a .zip or an extracted folder.
//!
//! `import_package` looks at the files inside, works out which platform the export is from,
//! and replaces that platform's data (other platforms are left alone). Only messages the
//! user wrote themselves are imported: other people's messages are none of our business.

mod discord;
mod reddit;
mod telegram;
mod x;
mod whatsapp;
mod meta;
mod tiktok;
mod snapchat;
mod linkedin;
mod mbox;

use crate::Store;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportSummary {
    pub platform: String,
    pub channels: usize,
    pub messages: usize,
    pub servers: usize,
    pub warnings: Vec<String>,
}

pub fn platform_label(platform: &str) -> &'static str {
    match platform {
        "discord" => "Discord",
        "reddit" => "Reddit",
        "telegram" => "Telegram",
        "x" => "X (Twitter)",
        "whatsapp" => "WhatsApp",
        "facebook" => "Facebook",
        "instagram" => "Instagram",
        "tiktok" => "TikTok",
        "snapchat" => "Snapchat",
        "linkedin" => "LinkedIn",
        "email" => "Email",
        "pc" => "This computer",
        _ => "Other",
    }
}

// ---------------------------------------------------------------------------------
// Reading a .zip or a folder

/// Where the export lives: a zip read entry by entry (never loaded whole), or a folder.
pub(crate) enum Source {
    Zip { archive: zip::ZipArchive<BufReader<File>>, names: HashMap<String, String> },
    Dir { names: HashMap<String, PathBuf> },
    /// A single file that isn't a zip: a WhatsApp .txt, a Gmail .mbox, a TikTok .json.
    File { name: String, path: PathBuf },
}

/// Folders that are huge and hold nothing we need.
fn is_skipped_folder(component: &str) -> bool {
    matches!(
        component.to_ascii_lowercase().as_str(),
        "activity" | "activities" | "analytics" | "tns" | "reporting" | "modeling" | "__macosx"
    )
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if path.is_dir() {
            let mut names = HashMap::new();
            walk(path, path, &mut names)?;
            Ok(Source::Dir { names })
        } else {
            let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
            let archive = match zip::ZipArchive::new(BufReader::new(file)) {
                Ok(a) => a,
                Err(_) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    return Ok(Source::File { name, path: path.to_path_buf() });
                }
            };
            let names = archive
                .file_names()
                .filter_map(|n| n.ok().map(|n| n.to_string()))
                .filter(|n| !n.ends_with('/'))
                .map(|n| (n.replace('\\', "/"), n))
                .filter(|(n, _)| !n.split('/').any(is_skipped_folder))
                .collect();
            Ok(Source::Zip { archive, names })
        }
    }

    pub(crate) fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = match self {
            Source::Zip { names, .. } => names.keys().cloned().collect(),
            Source::Dir { names } => names.keys().cloned().collect(),
            Source::File { name, .. } => vec![name.clone()],
        };
        v.sort();
        v
    }

    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self {
            Source::Zip { archive, names } => {
                let real = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?;
                archive.by_name(real)?.read_to_end(&mut buf)?;
            }
            Source::Dir { names } => {
                let p = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?;
                File::open(p)?.read_to_end(&mut buf)?;
            }
            Source::File { path, .. } => {
                File::open(path)?.read_to_end(&mut buf)?;
            }
        }
        Ok(buf)
    }

    /// Streams one file instead of loading it whole (a Gmail mailbox can be gigabytes).
    pub(crate) fn with_reader<T>(&mut self, name: &str, f: impl FnOnce(&mut dyn Read) -> Result<T>) -> Result<T> {
        match self {
            Source::Zip { archive, names } => {
                let real = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?.clone();
                let mut entry = archive.by_name(&real)?;
                f(&mut entry)
            }
            Source::Dir { names } => {
                let p = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?;
                f(&mut BufReader::new(File::open(p)?))
            }
            Source::File { path, .. } => f(&mut BufReader::new(File::open(path)?)),
        }
    }
}

fn walk(root: &Path, dir: &Path, out: &mut HashMap<String, PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_dir() {
            if !is_skipped_folder(&name) {
                walk(root, &path, out)?;
            }
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.insert(rel.to_string_lossy().replace('\\', "/"), path);
        }
    }
    Ok(())
}

pub(crate) fn split_parent(name: &str) -> (&str, &str) {
    match name.rfind('/') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => ("", name),
    }
}

pub(crate) fn file_name_lower(name: &str) -> String {
    split_parent(name).1.to_ascii_lowercase()
}

/// IDs written as JSON numbers (older exports) or strings (newer ones), as text.
pub(crate) fn value_to_id(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

pub(crate) fn get_ci<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a Value> {
    obj.get(key).or_else(|| obj.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v))
}

pub(crate) fn value_to_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>().join(" "),
        Some(other) => other.to_string(),
    }
}

/// Reads a CSV into header-name lookups. Column names match case-insensitively.
pub(crate) struct Csv {
    headers: Vec<String>,
    pub rows: Vec<csv::StringRecord>,
}

impl Csv {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
        let headers = rdr.headers()?.iter().map(|h| h.trim().trim_start_matches('\u{feff}').to_ascii_lowercase()).collect();
        let rows = rdr.records().collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self { headers, rows })
    }
    pub(crate) fn col(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name)
    }
    pub(crate) fn get<'a>(&self, row: &'a csv::StringRecord, name: &str) -> &'a str {
        self.col(name).and_then(|c| row.get(c)).unwrap_or("")
    }
}

// ---------------------------------------------------------------------------------
// Writing into the database

/// Collects one platform's channels and messages inside a single transaction.
pub(crate) struct Writer<'a> {
    tx: rusqlite::Transaction<'a>,
    platform: &'static str,
    channels: HashMap<String, i64>,
    pub(crate) messages: usize,
}

impl<'a> Writer<'a> {
    /// Starts a transaction that first clears this platform's previous import.
    /// If anything fails, the old data is still there.
    pub(crate) fn new(store: &'a mut Store, platform: &'static str) -> Result<Self> {
        let tx = store.conn.transaction()?;
        // Real data replaces the sample data entirely.
        let demo: bool = tx.query_row("SELECT COUNT(*) > 0 FROM meta WHERE key = 'demo' AND value = '1'", [], |r| r.get(0))?;
        if demo {
            tx.execute_batch(
                "DELETE FROM findings; DELETE FROM reviews; DELETE FROM ignored; DELETE FROM messages;
                 DELETE FROM channels; DELETE FROM sources; DELETE FROM meta WHERE key IN ('demo', 'scanned_at_ms');",
            )?;
        }
        tx.execute(
            "DELETE FROM findings WHERE message_id IN
               (SELECT m.id FROM messages m JOIN channels c ON c.id = m.channel_id WHERE c.platform = ?1)",
            [platform],
        )?;
        tx.execute("DELETE FROM messages WHERE channel_id IN (SELECT id FROM channels WHERE platform = ?1)", [platform])?;
        tx.execute("DELETE FROM channels WHERE platform = ?1", [platform])?;
        Ok(Self { tx, platform, channels: HashMap::new(), messages: 0 })
    }

    pub(crate) fn channel(
        &mut self,
        ext_id: &str,
        name: &str,
        kind: &str,
        guild_id: Option<&str>,
        guild_name: Option<&str>,
    ) -> Result<i64> {
        if let Some(id) = self.channels.get(ext_id) {
            return Ok(*id);
        }
        self.tx.execute(
            "INSERT INTO channels(platform, ext_id, name, kind, guild_id, guild_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(platform, ext_id) DO UPDATE SET name = excluded.name",
            (self.platform, ext_id, name, kind, guild_id, guild_name),
        )?;
        let id: i64 = self.tx.query_row(
            "SELECT id FROM channels WHERE platform = ?1 AND ext_id = ?2",
            (self.platform, ext_id),
            |r| r.get(0),
        )?;
        self.channels.insert(ext_id.to_string(), id);
        Ok(id)
    }

    pub(crate) fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub(crate) fn message(
        &mut self,
        ext_id: &str,
        channel: i64,
        ts: i64,
        content: &str,
        attachments: &str,
        link: Option<&str>,
    ) -> Result<()> {
        let uid = format!("{}:{}", self.platform, ext_id);
        let mut stmt = self.tx.prepare_cached(
            "INSERT OR IGNORE INTO messages(uid, channel_id, ts, content, attachments, link) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        self.messages += stmt.execute((uid, channel, ts, content, attachments, link))?;
        Ok(())
    }

    /// Commits, then updates per-channel counts, the search index and the sources list.
    pub(crate) fn finish(self, store_origin: &str) -> Result<()> {
        let platform = self.platform;
        self.tx.execute(
            "UPDATE channels SET
                message_count = (SELECT COUNT(*) FROM messages m WHERE m.channel_id = channels.id),
                first_ts      = (SELECT MIN(ts) FROM messages m WHERE m.channel_id = channels.id),
                last_ts       = (SELECT MAX(ts) FROM messages m WHERE m.channel_id = channels.id)
             WHERE platform = ?1",
            [platform],
        )?;
        self.tx.execute("DELETE FROM channels WHERE platform = ?1 AND message_count = 0", [platform])?;
        self.tx.execute(
            "INSERT INTO sources(platform, origin, imported_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(platform) DO UPDATE SET origin = excluded.origin, imported_at = excluded.imported_at",
            (platform, store_origin, crate::now_ms()),
        )?;
        self.tx.execute("DELETE FROM meta WHERE key = 'demo'", [])?;
        self.tx.commit()?;
        Ok(())
    }
}

pub(crate) fn after_import(store: &Store) -> Result<()> {
    store.conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild'); PRAGMA optimize;")?;
    store.set_meta("imported_at_ms", &crate::now_ms().to_string())
}

// ---------------------------------------------------------------------------------
// Detecting the platform and importing

/// Works out which platform an export is from by the files inside it.
pub(crate) fn detect(names: &[String]) -> Option<&'static str> {
    let lower: Vec<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
    let has_file = |f: &str| lower.iter().any(|n| file_name_lower(n) == f);
    let has_path = |p: &str| lower.iter().any(|n| n.contains(p));
    if lower.iter().any(|n| n.ends_with("data/account.js") || x::is_tweets_file(n)) {
        return Some("x");
    }
    if has_file("result.json") {
        return Some("telegram");
    }
    if has_file("channel.json") && (has_file("messages.json") || has_file("messages.csv")) {
        return Some("discord");
    }
    if has_path("your_instagram_activity") || has_path("personal_information/personal_information.json") {
        return Some("instagram");
    }
    if has_path("your_facebook_activity") || has_path("messages/inbox/") || has_file("profile_information.json") {
        return Some("facebook");
    }
    if has_file("chat_history.json") {
        return Some("snapchat");
    }
    if has_file("user_data_tiktok.json") || has_file("user_data.json") {
        return Some("tiktok");
    }
    if has_file("profile.csv") && (has_file("messages.csv") || has_file("shares.csv") || has_file("connections.csv")) {
        return Some("linkedin");
    }
    if has_file("comments.csv") || has_file("posts.csv") {
        return Some("reddit");
    }
    if lower.iter().any(|n| n.ends_with(".mbox")) {
        return Some("email");
    }
    if lower.iter().any(|n| whatsapp::is_chat_file(n)) {
        return Some("whatsapp");
    }
    None
}

pub use mbox::Mail as SentMail;

/// Reads one raw email (RFC 822) the way the mailbox importer does: recipients, subject and
/// your own words, without quoted replies. Live email sync uses this so both give the same records.
pub fn parse_sent_mail(raw: &[u8]) -> Option<SentMail> {
    mbox::parse_mail(raw).map(|mut m| {
        m.id = m.id.trim_matches(['<', '>']).to_string();
        m
    })
}

/// A Gmail search link that opens one message by its Message-ID.
pub fn gmail_link(message_id: &str) -> String {
    format!("https://mail.google.com/mail/u/0/#search/rfc822msgid%3A{}", message_id.replace('@', "%40"))
}

/// What importers may need to know beyond the export itself.
pub(crate) struct Ctx {
    /// The names the person goes by in chats (Settings), for exports that don't say which messages are theirs.
    pub my_names: Vec<String>,
}

impl Ctx {
    pub(crate) fn is_me(&self, name: &str) -> bool {
        let n = name.trim();
        !n.is_empty() && self.my_names.iter().any(|m| m.trim().eq_ignore_ascii_case(n))
    }
}

impl Store {
    pub fn my_names(&self) -> Result<Vec<String>> {
        Ok(self.get_meta("my_names")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
    }
    pub fn set_my_names(&self, names: &[String]) -> Result<()> {
        let clean: Vec<String> = names.iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
        self.set_meta("my_names", &serde_json::to_string(&clean)?)
    }
}

/// Imports an export, replacing whatever was imported before from the same platform.
pub fn import_package(
    store: &mut Store,
    path: &Path,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    progress(ImportProgress { phase: "Reading export", done: 0, total: 0 });
    let mut src = Source::open(path)?;
    let names = src.names();
    let platform = detect(&names).ok_or_else(|| {
        anyhow!(
            "Sompio doesn't recognise this file. It reads the data exports from Discord, Reddit, Telegram (JSON), X, WhatsApp, Instagram and Facebook (JSON), TikTok (JSON), Snapchat, LinkedIn and Gmail (Google Takeout). Open the file they sent you, or the folder you extracted it to."
        )
    })?;

    let ctx = Ctx { my_names: store.my_names()? };
    let mut w = Writer::new(store, platform)?;
    let mut summary = match platform {
        "discord" => discord::import(&mut src, &names, &mut w, progress)?,
        "reddit" => reddit::import(&mut src, &names, &mut w, progress)?,
        "telegram" => telegram::import(&mut src, &names, &mut w, progress)?,
        "x" => x::import(&mut src, &names, &mut w, progress)?,
        "whatsapp" => whatsapp::import(&mut src, &names, &mut w, &ctx, progress)?,
        "facebook" => meta::import(&mut src, &names, &mut w, &ctx, progress, meta::Which::Facebook)?,
        "instagram" => meta::import(&mut src, &names, &mut w, &ctx, progress, meta::Which::Instagram)?,
        "tiktok" => tiktok::import(&mut src, &names, &mut w, progress)?,
        "snapchat" => snapchat::import(&mut src, &names, &mut w, progress)?,
        "linkedin" => linkedin::import(&mut src, &names, &mut w, progress)?,
        "email" => mbox::import(&mut src, &names, &mut w, progress)?,
        _ => unreachable!(),
    };
    summary.platform = platform.to_string();
    summary.messages = w.messages;
    summary.channels = w.channel_count();
    if summary.messages == 0 {
        return Err(anyhow!(
            "This {} export has no messages you wrote in it. Check that messages or posts were included when you requested it.",
            platform_label(platform)
        ));
    }
    w.finish(&path.display().to_string())?;
    progress(ImportProgress { phase: "Building search index", done: 1, total: 1 });
    after_import(store)?;
    progress(ImportProgress { phase: "Done", done: 1, total: 1 });
    Ok(summary)
}

// ---------------------------------------------------------------------------------
// Data fetched live from a connected account (see the sompio-connect crate)

#[derive(Debug, Clone)]
pub struct LiveMessage {
    /// The platform's own ID for the message, in the same form the export importer uses,
    /// so decisions carry over between an export and a live sync.
    pub ext_id: String,
    pub ts: i64,
    pub content: String,
    pub link: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LiveChannel {
    pub ext_id: String,
    pub name: String,
    pub kind: String,
    pub messages: Vec<LiveMessage>,
}

/// Stores messages fetched from a connected account, replacing that platform's previous data.
pub fn import_live(store: &mut Store, platform: &'static str, origin: &str, channels: Vec<LiveChannel>) -> Result<ImportSummary> {
    let mut w = Writer::new(store, platform)?;
    for c in &channels {
        if c.messages.is_empty() {
            continue;
        }
        let cid = w.channel(&c.ext_id, &c.name, &c.kind, None, None)?;
        for m in &c.messages {
            w.message(&m.ext_id, cid, m.ts, &m.content, "", m.link.as_deref())?;
        }
    }
    let summary = ImportSummary {
        platform: platform.to_string(),
        channels: w.channel_count(),
        messages: w.messages,
        ..Default::default()
    };
    w.finish(origin)?;
    after_import(store)?;
    Ok(summary)
}
