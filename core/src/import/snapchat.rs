//! Snapchat's "My Data" export (Settings > My Data, with JSON files).
//! json/chat_history.json holds saved chats per friend; newer exports mark your messages with
//! "IsSender", older ones keep a separate "Sent Saved Chat History" list.

use super::{file_name_lower, ImportProgress, ImportSummary, Source, Writer};
use crate::time::parse_iso;
use anyhow::Result;
use serde_json::Value;

fn s<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(k).and_then(|x| x.as_str()))
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let f = names.iter().find(|n| file_name_lower(n) == "chat_history.json").cloned().unwrap_or_default();
    progress(ImportProgress { phase: "Reading Snapchat chats", done: 0, total: 1 });
    let v: Value = serde_json::from_slice(&src.read(&f)?)?;
    let mut n = 0;
    let Value::Object(map) = v else { return Ok(ImportSummary::default()) };

    if let Some(sent) = map.get("Sent Saved Chat History").and_then(|x| x.as_array()) {
        // Older layout: a list of what you sent.
        for m in sent {
            let friend = s(m, &["To"]).unwrap_or("Snapchat chat").to_string();
            let Some(text) = s(m, &["Text", "Content"]).filter(|t| !t.trim().is_empty()) else { continue };
            let ts = s(m, &["Created"]).and_then(parse_iso).unwrap_or(0);
            let cid = w.channel(&friend, &format!("Snapchat: {friend}"), "dm", None, None)?;
            n += 1;
            w.message(&format!("{friend}/{ts}/{n}"), cid, ts, text, "", None)?;
        }
        return Ok(ImportSummary::default());
    }

    for (friend, msgs) in &map {
        for m in msgs.as_array().into_iter().flatten() {
            if m.get("IsSender").and_then(|x| x.as_bool()) != Some(true) {
                continue;
            }
            let Some(text) = s(m, &["Content", "Text"]).filter(|t| !t.trim().is_empty()) else { continue };
            let ts = s(m, &["Created"]).and_then(parse_iso).unwrap_or(0);
            let kind = if m.get("Conversation Title").and_then(|x| x.as_str()).is_some_and(|t| !t.is_empty()) { "group_dm" } else { "dm" };
            let title = s(m, &["Conversation Title"]).filter(|t| !t.is_empty()).unwrap_or(friend);
            let cid = w.channel(friend, &format!("Snapchat: {title}"), kind, None, None)?;
            n += 1;
            w.message(&format!("{friend}/{ts}/{n}"), cid, ts, text, "", None)?;
        }
    }
    Ok(ImportSummary::default())
}
