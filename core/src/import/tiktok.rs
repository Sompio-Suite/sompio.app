//! TikTok's data export (Settings > Account > Download your data, JSON format):
//! one big user_data_tiktok.json. Section names change between versions, so Veil looks for
//! the parts it needs anywhere in the file: your comments, your direct messages, your bio.

use super::{file_name_lower, ImportProgress, ImportSummary, Source, Writer};
use crate::time::parse_iso;
use anyhow::Result;
use serde_json::Value;

fn find_key<'a>(v: &'a Value, keys: &[&str], out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if keys.iter().any(|w| k.eq_ignore_ascii_case(w)) {
                    out.push(x);
                } else {
                    find_key(x, keys, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| find_key(x, keys, out)),
        _ => {}
    }
}

fn get_ci<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, x)| x.as_str())
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let f = names
        .iter()
        .find(|n| matches!(file_name_lower(n).as_str(), "user_data_tiktok.json" | "user_data.json"))
        .cloned()
        .unwrap_or_default();
    progress(ImportProgress { phase: "Reading TikTok export", done: 0, total: 1 });
    let v: Value = serde_json::from_slice(&src.read(&f)?)?;
    let mut summary = ImportSummary::default();

    let mut found = Vec::new();
    find_key(&v, &["userName", "username"], &mut found);
    let me = found.iter().find_map(|x| x.as_str()).unwrap_or("").to_string();

    // Bio
    let mut bios = Vec::new();
    find_key(&v, &["bioDescription"], &mut bios);
    if let Some(bio) = bios.iter().find_map(|b| b.as_str()).filter(|b| !b.trim().is_empty()) {
        let cid = w.channel("profile", "Your TikTok profile", "posts", None, None)?;
        w.message("bio", cid, 0, bio, "", None)?;
    }

    // Comments
    let mut lists = Vec::new();
    find_key(&v, &["CommentsList"], &mut lists);
    let mut n = 0;
    for c in lists.iter().filter_map(|l| l.as_array()).flatten() {
        let Some(text) = get_ci(c, "comment") else { continue };
        let ts = get_ci(c, "date").and_then(parse_iso).unwrap_or(0);
        let cid = w.channel("comments", "Your TikTok comments", "community", None, None)?;
        n += 1;
        w.message(&format!("comment/{ts}/{n}"), cid, ts, text, "", None)?;
    }

    // Direct messages you sent
    let mut histories = Vec::new();
    find_key(&v, &["ChatHistory"], &mut histories);
    if !histories.is_empty() && me.is_empty() {
        summary.warnings.push("Direct messages were skipped because the export doesn't include your username.".to_string());
    }
    for h in histories.iter().filter_map(|h| h.as_object()) {
        for (chat, msgs) in h {
            let other = chat.trim_end_matches(':').rsplit(" with ").next().unwrap_or(chat).trim().to_string();
            for m in msgs.as_array().into_iter().flatten() {
                if get_ci(m, "from").map(|f| f.eq_ignore_ascii_case(&me)) != Some(true) {
                    continue;
                }
                let Some(text) = get_ci(m, "content").filter(|t| !t.trim().is_empty()) else { continue };
                let ts = get_ci(m, "date").and_then(parse_iso).unwrap_or(0);
                let cid = w.channel(&format!("dm:{other}"), &format!("TikTok: {other}"), "dm", None, None)?;
                n += 1;
                w.message(&format!("dm/{other}/{ts}/{n}"), cid, ts, text, "", None)?;
            }
        }
    }
    Ok(summary)
}
