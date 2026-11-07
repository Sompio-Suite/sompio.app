//! Discord's "Request all of my data" package.
//!
//! Discord has changed the layout over the years and folder names can be localized,
//! so nothing here depends on exact folder names. A channel is any folder that holds
//! `channel.json` plus `messages.json` (newer packages) or `messages.csv` (older ones),
//! and channel names come from the `index.json` next to those folders.
//! The package only ever contains messages you sent.

use super::{get_ci, split_parent, value_to_id, value_to_text, Csv, ImportProgress, ImportSummary, Source, Writer};
use crate::snowflake_to_ms;
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Default)]
struct ChannelFiles {
    channel_json: Option<String>,
    messages_json: Option<String>,
    messages_csv: Option<String>,
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
        kind
    } else if has_guild {
        "server"
    } else if recipients > 2 {
        "group_dm"
    } else if recipients > 0 {
        "dm"
    } else {
        "unknown"
    }
}

struct Raw {
    id: String,
    content: String,
    attachments: String,
}

fn parse_json(bytes: &[u8]) -> Result<Vec<Raw>> {
    let list: Vec<serde_json::Map<String, Value>> = serde_json::from_slice(bytes)?;
    Ok(list
        .iter()
        .filter_map(|m| {
            Some(Raw {
                id: value_to_id(get_ci(m, "ID")?)?,
                content: value_to_text(get_ci(m, "Contents")),
                attachments: value_to_text(get_ci(m, "Attachments")),
            })
        })
        .collect())
}

fn parse_csv(bytes: &[u8]) -> Result<Vec<Raw>> {
    let csv = Csv::parse(bytes)?;
    Ok(csv
        .rows
        .iter()
        .filter(|r| !csv.get(r, "id").trim().is_empty())
        .map(|r| Raw {
            id: csv.get(r, "id").trim().to_string(),
            content: csv.get(r, "contents").to_string(),
            attachments: csv.get(r, "attachments").to_string(),
        })
        .collect())
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let mut folders: BTreeMap<String, ChannelFiles> = BTreeMap::new();
    for name in names {
        let (parent, file) = split_parent(name);
        let f = match file.to_ascii_lowercase().as_str() {
            "channel.json" | "messages.json" | "messages.csv" => folders.entry(parent.to_string()).or_default(),
            _ => continue,
        };
        match file.to_ascii_lowercase().as_str() {
            "channel.json" => f.channel_json = Some(name.clone()),
            "messages.json" => f.messages_json = Some(name.clone()),
            _ => f.messages_csv = Some(name.clone()),
        }
    }
    folders.retain(|_, f| f.messages_json.is_some() || f.messages_csv.is_some());

    // Channel names from the index.json next to the channel folders.
    let mut index_names: HashMap<String, String> = HashMap::new();
    let parents: BTreeSet<String> = folders.keys().map(|k| split_parent(k).0.to_string()).collect();
    let name_set: HashSet<&String> = names.iter().collect();
    for parent in parents {
        let idx = if parent.is_empty() { "index.json".to_string() } else { format!("{parent}/index.json") };
        if !name_set.contains(&idx) {
            continue;
        }
        if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&src.read(&idx)?) {
            for (k, v) in map {
                if let Some(n) = v.as_str() {
                    index_names.insert(k, n.to_string());
                }
            }
        }
    }

    let mut summary = ImportSummary::default();
    let mut guilds = HashSet::new();
    let total = folders.len();
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
            .or_else(|| Some(folder_name.trim_start_matches(['c', 'C']).to_string()).filter(|s| s.parse::<u64>().is_ok()));
        let Some(channel_id) = channel_id else {
            summary.warnings.push(format!("{folder}: skipped, no channel ID"));
            continue;
        };
        let guild = get_ci(&meta, "guild").and_then(|g| g.as_object());
        let guild_id = guild.and_then(|g| get_ci(g, "id")).and_then(value_to_id);
        let guild_name = guild.and_then(|g| get_ci(g, "name")).and_then(|v| v.as_str()).map(str::to_string);
        let recipients = get_ci(&meta, "recipients").and_then(|r| r.as_array()).map_or(0, |a| a.len());
        let kind = channel_kind(get_ci(&meta, "type"), guild.is_some(), recipients);
        if let Some(g) = &guild_id {
            guilds.insert(g.clone());
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
        let cid = w.channel(&channel_id, &name, kind, guild_id.as_deref(), guild_name.as_deref())?;

        let parsed = match (&files.messages_json, &files.messages_csv) {
            (Some(n), _) => parse_json(&src.read(n)?),
            (None, Some(n)) => parse_csv(&src.read(n)?),
            _ => continue,
        };
        match parsed {
            Ok(msgs) => {
                let guild_part = guild_id.as_deref().unwrap_or("@me");
                for m in msgs {
                    let ts = m.id.parse::<i64>().map(snowflake_to_ms).unwrap_or(0);
                    let link = format!("https://discord.com/channels/{guild_part}/{channel_id}/{}", m.id);
                    w.message(&m.id, cid, ts, &m.content, &m.attachments, Some(&link))?;
                }
            }
            Err(e) => summary.warnings.push(format!("{name}: messages could not be read ({e})")),
        }
        summary.channels += 1;
    }
    summary.servers = guilds.len();
    Ok(summary)
}
