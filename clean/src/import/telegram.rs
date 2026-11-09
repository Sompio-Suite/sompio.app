//! Telegram Desktop's export (Settings > Advanced > Export Telegram data, format JSON).
//! A single `result.json` with every chat. Chats contain both sides of the conversation,
//! so only messages from your own account (named in `personal_information`) are kept.

use super::{file_name_lower, ImportProgress, ImportSummary, Source, Writer};
use crate::time::parse_iso;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Export {
    personal_information: Option<Personal>,
    chats: Option<ChatList>,
    left_chats: Option<ChatList>,
    // A single-chat export has these at the top level instead.
    messages: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Personal {
    user_id: Value,
}

#[derive(Deserialize)]
struct ChatList {
    list: Vec<Chat>,
}

#[derive(Deserialize)]
struct Chat {
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Value,
    #[serde(default)]
    messages: Vec<Msg>,
}

#[derive(Deserialize)]
struct Msg {
    id: Value,
    #[serde(rename = "type")]
    kind: Option<String>,
    date: Option<String>,
    date_unixtime: Option<String>,
    from_id: Option<String>,
    #[serde(default)]
    text: Value,
    file: Option<String>,
    photo: Option<String>,
}

/// Telegram text is a plain string, or a list of strings and formatted pieces.
fn flatten(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| match p {
                Value::String(s) => s.clone(),
                Value::Object(o) => o.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                _ => String::new(),
            })
            .collect(),
        _ => String::new(),
    }
}

fn id_text(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn kind_of(t: &str) -> &'static str {
    match t {
        "personal_chat" | "bot_chat" => "dm",
        "saved_messages" => "saved",
        "private_group" | "private_supergroup" | "public_supergroup" => "group_dm",
        "private_channel" | "public_channel" => "channel",
        _ => "unknown",
    }
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let file = names
        .iter()
        .filter(|n| file_name_lower(n) == "result.json")
        .min_by_key(|n| n.len())
        .ok_or_else(|| anyhow!("result.json not found"))?
        .clone();
    progress(ImportProgress { phase: "Reading Telegram export", done: 0, total: 0 });
    let export: Export = serde_json::from_slice(&src.read(&file)?)?;

    let Some(me) = export.personal_information.as_ref().map(|p| format!("user{}", id_text(&p.user_id))) else {
        let hint = if export.messages.is_some() { " This looks like a single chat's export." } else { "" };
        return Err(anyhow!(
            "Sompio needs Telegram's full account export to know which messages are yours.{hint} In Telegram Desktop, open Settings > Advanced > Export Telegram data, tick Personal information and the chats you want, and choose JSON as the format."
        ));
    };

    let mut chats: Vec<Chat> = Vec::new();
    if let Some(c) = export.chats {
        chats.extend(c.list);
    }
    if let Some(c) = export.left_chats {
        chats.extend(c.list);
    }
    let total = chats.len();
    for (i, chat) in chats.iter().enumerate() {
        progress(ImportProgress { phase: "Importing messages", done: i, total });
        let chat_type = chat.kind.as_deref().unwrap_or("");
        let chat_id = id_text(&chat.id);
        let name = chat.name.clone().unwrap_or_else(|| {
            if chat_type == "saved_messages" { "Saved Messages".to_string() } else { "Telegram chat".to_string() }
        });
        let linkable = matches!(chat_type, "private_supergroup" | "public_supergroup" | "private_channel" | "public_channel");
        let mut channel: Option<i64> = None;
        for m in &chat.messages {
            if m.kind.as_deref() != Some("message") || m.from_id.as_deref() != Some(me.as_str()) {
                continue;
            }
            let mut text = flatten(&m.text);
            for f in [&m.file, &m.photo].into_iter().flatten() {
                if !f.is_empty() && !f.starts_with("(File not included") {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&format!("[file: {}]", f.rsplit('/').next().unwrap_or(f)));
                }
            }
            if text.trim().is_empty() {
                continue;
            }
            let cid = match channel {
                Some(c) => c,
                None => {
                    let c = w.channel(&chat_id, &name, kind_of(chat_type), None, None)?;
                    channel = Some(c);
                    c
                }
            };
            let ts = m
                .date_unixtime
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .map(|s| s * 1000)
                .or_else(|| m.date.as_deref().and_then(parse_iso))
                .unwrap_or(0);
            let mid = id_text(&m.id);
            let link = linkable.then(|| format!("https://t.me/c/{chat_id}/{mid}"));
            w.message(&format!("{chat_id}/{mid}"), cid, ts, &text, "", link.as_deref())?;
        }
    }
    Ok(ImportSummary::default())
}
