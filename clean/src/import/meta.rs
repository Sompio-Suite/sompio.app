//! Facebook and Instagram "Download your information" exports, in JSON format.
//! Both come from Meta and share the message layout: messages/inbox/<chat>/message_1.json.
//!
//! Meta writes text as UTF-8 bytes dressed up as Latin-1 ("cafÃ©" for "café"); `fix` undoes that.
//! Your own name comes from the profile file in the export, the names under "Your names",
//! or failing both, the one person who is in every chat.

use super::{file_name_lower, split_parent, Ctx, ImportProgress, ImportSummary, Source, Writer};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Which {
    Facebook,
    Instagram,
}

/// Undoes Meta's double encoding when the text is in fact UTF-8.
pub(crate) fn fix(s: &str) -> String {
    if s.chars().all(|c| (c as u32) < 256) {
        let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
        if let Ok(good) = String::from_utf8(bytes) {
            return good;
        }
    }
    s.to_string()
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for p in path {
        cur = match cur {
            Value::Array(a) => a.first()?.get(p)?,
            _ => cur.get(p)?,
        };
    }
    cur.as_str()
}

fn ts_at(v: &Value, keys: &[&str]) -> i64 {
    for k in keys {
        if let Some(n) = v.get(k).and_then(|x| x.as_i64()) {
            // Seconds or milliseconds?
            return if n > 100_000_000_000 { n } else { n * 1000 };
        }
    }
    0
}

/// Every object anywhere in `v` that has a `string_map_data` (Instagram's generic record shape).
fn string_maps<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            if let Some(sm) = m.get("string_map_data") {
                out.push(sm);
            }
            m.values().for_each(|x| string_maps(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| string_maps(x, out)),
        _ => {}
    }
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    ctx: &Ctx,
    progress: &mut dyn FnMut(ImportProgress),
    which: Which,
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    let lower = |n: &String| n.to_ascii_lowercase();

    // Who you are
    let mut me: Vec<String> = ctx.my_names.clone();
    for f in names {
        let l = lower(f);
        let v: Option<Value> = if l.ends_with("profile_information.json") || l.ends_with("personal_information/personal_information.json") {
            serde_json::from_slice(&src.read(f)?).ok()
        } else {
            None
        };
        if let Some(v) = v {
            for p in [&["profile_v2", "name", "full_name"][..], &["profile", "name", "full_name"][..],
                      &["profile_user", "string_map_data", "Name", "value"][..], &["profile_user", "string_map_data", "Username", "value"][..]] {
                if let Some(n) = str_at(&v, p) {
                    me.push(fix(n));
                }
            }
        }
    }

    // Messages
    let threads: Vec<String> = names
        .iter()
        .filter(|n| {
            let l = lower(n);
            l.contains("messages/") && file_name_lower(n).starts_with("message_") && l.ends_with(".json")
        })
        .cloned()
        .collect();
    let mut parsed: Vec<(String, Value)> = Vec::new();
    for (i, f) in threads.iter().enumerate() {
        progress(ImportProgress { phase: "Reading messages", done: i, total: threads.len() });
        if let Ok(v) = serde_json::from_slice::<Value>(&src.read(f)?) {
            parsed.push((split_parent(f).0.to_string(), v));
        }
    }
    if me.is_empty() && parsed.len() >= 2 {
        // You're in every chat you have, so the most common participant is you.
        let mut count: HashMap<String, usize> = HashMap::new();
        for (_, v) in &parsed {
            for p in v.get("participants").and_then(|p| p.as_array()).into_iter().flatten() {
                if let Some(n) = p.get("name").and_then(|n| n.as_str()) {
                    *count.entry(fix(n)).or_default() += 1;
                }
            }
        }
        if let Some((n, c)) = count.into_iter().max_by_key(|(_, c)| *c) {
            if c * 2 > parsed.len() {
                me.push(n);
            }
        }
    }
    let is_me = |n: &str| me.iter().any(|m| m.eq_ignore_ascii_case(n.trim()));
    let label = if which == Which::Facebook { "Messenger" } else { "Instagram" };
    if !parsed.is_empty() && me.is_empty() {
        summary.warnings.push(format!(
            "{label} messages were skipped because Sompio couldn't tell which are yours. Add your name under Sources > Your names and import again."
        ));
    } else {
        for (folder, v) in &parsed {
            let participants = v.get("participants").and_then(|p| p.as_array()).map_or(0, |a| a.len());
            let title = v.get("title").and_then(|t| t.as_str()).map(fix).unwrap_or_else(|| "Chat".to_string());
            let thread = split_parent(folder).1.to_string();
            let mut channel = None;
            let mut seen: HashMap<i64, usize> = HashMap::new();
            for m in v.get("messages").and_then(|m| m.as_array()).into_iter().flatten() {
                let sender = m.get("sender_name").and_then(|s| s.as_str()).map(fix).unwrap_or_default();
                let Some(content) = m.get("content").and_then(|c| c.as_str()).map(fix) else { continue };
                if !is_me(&sender) || content.trim().is_empty() {
                    continue;
                }
                let cid = match channel {
                    Some(c) => c,
                    None => {
                        let kind = if participants > 2 { "group_dm" } else { "dm" };
                        let c = w.channel(&format!("chat:{thread}"), &format!("{label}: {title}"), kind, None, None)?;
                        channel = Some(c);
                        c
                    }
                };
                let ts = ts_at(m, &["timestamp_ms"]);
                let n = seen.entry(ts).or_default();
                *n += 1;
                w.message(&format!("{thread}/{ts}/{n}"), cid, ts, &content, "", None)?;
            }
        }
    }

    // Posts and comments you wrote
    progress(ImportProgress { phase: "Reading posts and comments", done: 0, total: 1 });
    let mut n = 0usize;
    for f in names {
        let l = lower(f);
        let file = file_name_lower(f);
        let is_posts = (which == Which::Facebook && file.starts_with("your_posts") && l.ends_with(".json"))
            || (which == Which::Instagram && l.contains("content/") && file.starts_with("posts_") && l.ends_with(".json"));
        let is_comments = file.ends_with(".json")
            && (file == "comments.json" || file.starts_with("post_comments") || file.contains("reels_comments"))
            && l.contains("comment");
        if !is_posts && !is_comments {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&src.read(f)?) else { continue };
        let mut add = |text: String, ts: i64, kind: &str, w: &mut Writer| -> Result<()> {
            if text.trim().is_empty() {
                return Ok(());
            }
            let (ext, name, ckind) = if kind == "post" { ("posts", "Your posts", "posts") } else { ("comments", "Your comments", "community") };
            let cid = w.channel(ext, name, ckind, None, None)?;
            n += 1;
            w.message(&format!("{kind}/{ts}/{n}"), cid, ts, &text, "", None)
        };
        if which == Which::Facebook {
            let items: Vec<&Value> = match &v {
                Value::Array(a) => a.iter().collect(),
                Value::Object(o) => o.values().filter_map(|x| x.as_array()).flatten().collect(),
                _ => vec![],
            };
            for it in items {
                let ts = ts_at(it, &["timestamp"]);
                for d in it.get("data").and_then(|d| d.as_array()).into_iter().flatten() {
                    if let Some(p) = d.get("post").and_then(|p| p.as_str()) {
                        add(fix(p), ts, "post", w)?;
                    }
                    if let Some(c) = d.get("comment").and_then(|c| c.get("comment")).and_then(|c| c.as_str()) {
                        add(fix(c), ts, "comment", w)?;
                    }
                }
            }
        } else if is_posts {
            for it in v.as_array().into_iter().flatten() {
                let caption = it.get("title").and_then(|t| t.as_str()).filter(|t| !t.is_empty())
                    .or_else(|| str_at(it, &["media", "title"]));
                if let Some(c) = caption {
                    let ts = ts_at(it, &["creation_timestamp"]).max(it.get("media").and_then(|m| m.get(0)).map_or(0, |m| ts_at(m, &["creation_timestamp"])));
                    add(fix(c), ts, "post", w)?;
                }
            }
        } else {
            let mut maps = Vec::new();
            string_maps(&v, &mut maps);
            for sm in maps {
                if let Some(c) = sm.get("Comment").and_then(|c| c.get("value")).and_then(|c| c.as_str()) {
                    let ts = sm.get("Time").map_or(0, |t| ts_at(t, &["timestamp"]));
                    add(fix(c), ts, "comment", w)?;
                }
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixes_meta_encoding() {
        assert_eq!(super::fix("caf\u{00c3}\u{00a9}"), "café");
        assert_eq!(super::fix("plain"), "plain");
        assert_eq!(super::fix("already é fine 😀"), "already é fine 😀");
    }
}
