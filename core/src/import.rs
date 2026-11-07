//! Reads Discord's "Request all of my data" package, as a .zip or an extracted folder.
//!
//! Discord has changed the layout over the years, and folder names can be localized,
//! so nothing here depends on exact folder names. A channel is any folder that holds
//! `channel.json` plus `messages.json` (newer packages) or `messages.csv` (older ones),
//! and channel names come from the `index.json` that sits next to those folders.

use crate::{snowflake_to_ms, Store};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
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
    pub channels: usize,
    pub messages: usize,
    pub servers: usize,
    pub warnings: Vec<String>,
}

/// Where the package lives: a zip read entry by entry (never loaded whole), or a folder.
enum Source {
    Zip {
        archive: zip::ZipArchive<BufReader<File>>,
        /// normalized name -> name as stored in the zip
        names: HashMap<String, String>,
    },
    Dir {
        root: PathBuf,
        names: HashMap<String, PathBuf>,
    },
}

/// Folders that are huge and hold nothing we need.
fn is_skipped_folder(component: &str) -> bool {
    matches!(
        component.to_ascii_lowercase().as_str(),
        "activity" | "activities" | "analytics" | "tns" | "reporting" | "modeling"
    )
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        if path.is_dir() {
            let mut names = HashMap::new();
            walk(path, path, &mut names)?;
            Ok(Source::Dir { root: path.to_path_buf(), names })
        } else {
            let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
            let archive = zip::ZipArchive::new(BufReader::new(file))
                .context("this file is not a readable zip archive")?;
            let names = archive
                .file_names()
                .filter_map(|n| n.ok().map(|n| n.to_string()))
                .filter(|n| !n.ends_with('/'))
                .map(|n| (n.replace('\\', "/"), n.to_string()))
                .filter(|(n, _)| !n.split('/').any(is_skipped_folder))
                .collect();
            Ok(Source::Zip { archive, names })
        }
    }

    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = match self {
            Source::Zip { names, .. } => names.keys().cloned().collect(),
            Source::Dir { names, .. } => names.keys().cloned().collect(),
        };
        v.sort();
        v
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self {
            Source::Zip { archive, names } => {
                let real = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?;
                archive.by_name(real)?.read_to_end(&mut buf)?;
            }
            Source::Dir { root, names } => {
                let p = names.get(name).ok_or_else(|| anyhow!("missing {name}"))?;
                let _ = root;
                File::open(p)?.read_to_end(&mut buf)?;
            }
        }
        Ok(buf)
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
            let rel = rel.to_string_lossy().replace('\\', "/");
            out.insert(rel, path);
        }
    }
    Ok(())
}

#[derive(Default)]
struct ChannelFiles {
    channel_json: Option<String>,
    messages_json: Option<String>,
    messages_csv: Option<String>,
}

fn split_parent(name: &str) -> (&str, &str) {
    match name.rfind('/') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => ("", name),
    }
}

/// Accepts IDs written as JSON numbers (older packages) or strings (newer ones).
fn value_to_id(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|u| u as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn get_ci<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a Value> {
    obj.get(key)
        .or_else(|| obj.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v))
}

fn value_to_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect::<Vec<_>>()
            .join(" "),
        Some(other) => other.to_string(),
    }
}

fn channel_kind(type_value: Option<&Value>, has_guild: bool, recipients: usize) -> &'static str {
    let kind = match type_value {
        Some(Value::Number(n)) => match n.as_i64().unwrap_or(-1) {
            1 => "dm",
            3 => "group_dm",
            10 | 11 | 12 => "thread",
            0 | 2 | 4 | 5 | 13 | 14 | 15 | 16 => "server",
            _ => "unknown",
        },
        Some(Value::String(s)) => {
            let s = s.to_ascii_uppercase();
            if s == "DM" {
                "dm"
            } else if s == "GROUP_DM" {
                "group_dm"
            } else if s.contains("THREAD") {
                "thread"
            } else if s.starts_with("GUILD") {
                "server"
            } else {
                "unknown"
            }
        }
        _ => "unknown",
    };
    if kind != "unknown" {
        return kind;
    }
    if has_guild {
        "server"
    } else if recipients > 2 {
        "group_dm"
    } else if recipients > 0 {
        "dm"
    } else {
        "unknown"
    }
}

struct RawMessage {
    id: i64,
    content: String,
    attachments: String,
}

fn parse_messages_json(bytes: &[u8]) -> Result<Vec<RawMessage>> {
    let list: Vec<serde_json::Map<String, Value>> = serde_json::from_slice(bytes)?;
    Ok(list
        .iter()
        .filter_map(|m| {
            Some(RawMessage {
                id: value_to_id(get_ci(m, "ID")?)?,
                content: value_to_text(get_ci(m, "Contents")),
                attachments: value_to_text(get_ci(m, "Attachments")),
            })
        })
        .collect())
}

fn parse_messages_csv(bytes: &[u8]) -> Result<Vec<RawMessage>> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = rdr.headers()?.clone();
    let col = |name: &str| headers.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    let (id_c, content_c, att_c) = (col("ID"), col("Contents"), col("Attachments"));
    let id_c = id_c.ok_or_else(|| anyhow!("messages.csv has no ID column"))?;
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let Some(id) = rec.get(id_c).and_then(|s| s.trim().parse().ok()) else { continue };
        out.push(RawMessage {
            id,
            content: content_c.and_then(|c| rec.get(c)).unwrap_or("").to_string(),
            attachments: att_c.and_then(|c| rec.get(c)).unwrap_or("").to_string(),
        });
    }
    Ok(out)
}

/// Imports a data package, replacing whatever was imported before.
pub fn import_package(
    store: &mut Store,
    path: &Path,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    progress(ImportProgress { phase: "Reading package", done: 0, total: 0 });
    let mut src = Source::open(path)?;
    let names = src.names();

    // 1. Find channel folders, wherever they are.
    let mut folders: BTreeMap<String, ChannelFiles> = BTreeMap::new();
    for name in &names {
        let (parent, file) = split_parent(name);
        let slot = match file.to_ascii_lowercase().as_str() {
            "channel.json" => 0,
            "messages.json" => 1,
            "messages.csv" => 2,
            _ => continue,
        };
        let f = folders.entry(parent.to_string()).or_default();
        match slot {
            0 => f.channel_json = Some(name.clone()),
            1 => f.messages_json = Some(name.clone()),
            _ => f.messages_csv = Some(name.clone()),
        }
    }
    folders.retain(|_, f| f.messages_json.is_some() || f.messages_csv.is_some());
    if folders.is_empty() {
        return Err(anyhow!(
            "No messages found. Make sure this is the zip Discord emailed you (Settings > Data & Privacy > Request all of my data) and that Messages was ticked."
        ));
    }

    // 2. Channel names from the index.json next to the channel folders.
    let mut index_names: HashMap<i64, String> = HashMap::new();
    let parents: std::collections::BTreeSet<String> =
        folders.keys().map(|k| split_parent(k).0.to_string()).collect();
    for parent in parents {
        let idx = if parent.is_empty() { "index.json".to_string() } else { format!("{parent}/index.json") };
        if !names.contains(&idx) {
            continue;
        }
        if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&src.read(&idx)?) {
            for (k, v) in map {
                if let (Ok(id), Some(n)) = (k.parse::<i64>(), v.as_str()) {
                    index_names.insert(id, n.to_string());
                }
            }
        }
    }

    // 3. Replace previous data and import.
    let mut summary = ImportSummary::default();
    store.wipe()?;
    let total = folders.len();
    let tx = store.conn.transaction()?;
    {
        let mut ins_channel = tx.prepare(
            "INSERT OR REPLACE INTO channels(id, name, kind, guild_id, guild_name) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut ins_msg = tx.prepare(
            "INSERT OR IGNORE INTO messages(id, channel_id, ts, content, attachments) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut guilds = std::collections::HashSet::new();

        for (i, (folder, files)) in folders.iter().enumerate() {
            progress(ImportProgress { phase: "Importing messages", done: i, total });
            let folder_name = split_parent(folder).1;

            let meta: serde_json::Map<String, Value> = match &files.channel_json {
                Some(n) => match serde_json::from_slice::<Value>(&src.read(n)?) {
                    Ok(Value::Object(m)) => m,
                    _ => {
                        summary.warnings.push(format!("{folder}: channel.json could not be read"));
                        Default::default()
                    }
                },
                None => Default::default(),
            };

            let channel_id = get_ci(&meta, "id")
                .and_then(value_to_id)
                .or_else(|| folder_name.trim_start_matches(['c', 'C']).parse().ok());
            let Some(channel_id) = channel_id else {
                summary.warnings.push(format!("{folder}: skipped, no channel ID"));
                continue;
            };

            let guild = get_ci(&meta, "guild").and_then(|g| g.as_object());
            let guild_id = guild.and_then(|g| get_ci(g, "id")).and_then(value_to_id);
            let guild_name = guild.and_then(|g| get_ci(g, "name")).and_then(|v| v.as_str()).map(str::to_string);
            let recipients = get_ci(&meta, "recipients").and_then(|r| r.as_array()).map_or(0, |a| a.len());
            let kind = channel_kind(get_ci(&meta, "type"), guild.is_some(), recipients);
            if let Some(g) = guild_id {
                guilds.insert(g);
            }

            let name = index_names
                .get(&channel_id)
                .cloned()
                .or_else(|| {
                    get_ci(&meta, "name").and_then(|v| v.as_str()).map(|n| match &guild_name {
                        Some(g) => format!("{n} in {g}"),
                        None => n.to_string(),
                    })
                })
                .unwrap_or_else(|| match kind {
                    "dm" => "Direct message".to_string(),
                    "group_dm" => "Group chat".to_string(),
                    _ => format!("Channel {channel_id}"),
                });

            ins_channel.execute((channel_id, &name, kind, guild_id, &guild_name))?;

            let parsed = if let Some(n) = &files.messages_json {
                parse_messages_json(&src.read(n)?)
            } else {
                parse_messages_csv(&src.read(files.messages_csv.as_ref().unwrap())?)
            };
            match parsed {
                Ok(msgs) => {
                    for m in msgs {
                        let ts = snowflake_to_ms(m.id);
                        summary.messages += ins_msg.execute((m.id, channel_id, ts, &m.content, &m.attachments))?;
                    }
                }
                Err(e) => summary.warnings.push(format!("{name}: messages could not be read ({e})")),
            }
            summary.channels += 1;
        }
        summary.servers = guilds.len();
    }
    tx.commit()?;

    progress(ImportProgress { phase: "Building search index", done: total, total });
    store.conn.execute_batch(
        "UPDATE channels SET
            message_count = (SELECT COUNT(*) FROM messages m WHERE m.channel_id = channels.id),
            first_ts      = (SELECT MIN(ts) FROM messages m WHERE m.channel_id = channels.id),
            last_ts       = (SELECT MAX(ts) FROM messages m WHERE m.channel_id = channels.id);
         INSERT INTO messages_fts(messages_fts) VALUES('rebuild');
         PRAGMA optimize;",
    )?;
    store.set_meta("imported_at_ms", &crate::now_ms().to_string())?;
    progress(ImportProgress { phase: "Done", done: total, total });
    Ok(summary)
}
