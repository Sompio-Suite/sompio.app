//! X (Twitter) archive (Settings > Your account > Download an archive of your data).
//! The data sits in JavaScript files like `data/tweets.js` that wrap a JSON array.
//! Your own posts are imported (not reposts of others), plus direct messages you sent.

use super::{ImportProgress, ImportSummary, Source, Writer};
use crate::time::{parse_iso, parse_twitter};
use anyhow::{anyhow, Result};
use serde_json::Value;

/// data/tweets.js, data/tweet.js, data/tweets-part1.js … (but not tweetdeck.js)
pub(crate) fn is_tweets_file(name_lower: &str) -> bool {
    let Some(rest) = name_lower.rsplit("data/").next().filter(|_| name_lower.contains("data/")) else { return false };
    let stem = rest.trim_end_matches(".js");
    if !rest.ends_with(".js") {
        return false;
    }
    let base = stem.split("-part").next().unwrap_or(stem);
    (base == "tweets" || base == "tweet") && stem[base.len()..].trim_start_matches("-part").chars().all(|c| c.is_ascii_digit())
}

fn is_dm_file(name_lower: &str) -> bool {
    let Some(rest) = name_lower.rsplit("data/").next().filter(|_| name_lower.contains("data/")) else { return false };
    rest.starts_with("direct-messages") && rest.ends_with(".js") && !rest.contains("header")
}

/// `window.YTD.tweets.part0 = [ ... ]` -> the array
fn parse_js(bytes: &[u8]) -> Result<Vec<Value>> {
    let text = String::from_utf8_lossy(bytes);
    let start = text.find('[').ok_or_else(|| anyhow!("no data found"))?;
    Ok(serde_json::from_str(text[start..].trim().trim_end_matches(';'))?)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    let lower = |n: &String| n.to_ascii_lowercase();

    // Who you are
    let (mut account_id, mut username) = (String::new(), String::new());
    if let Some(f) = names.iter().find(|n| lower(n).ends_with("data/account.js")) {
        if let Some(acc) = parse_js(&src.read(f)?)?.first().and_then(|v| v.get("account")) {
            account_id = s(acc, "accountId").to_string();
            username = s(acc, "username").to_string();
        }
    }

    // Posts
    let tweet_files: Vec<String> = names.iter().filter(|n| is_tweets_file(&lower(n))).cloned().collect();
    let total = tweet_files.len() + 1;
    for (i, f) in tweet_files.iter().enumerate() {
        progress(ImportProgress { phase: "Importing posts", done: i, total });
        let channel = w.channel("posts", "Your posts", "posts", None, None)?;
        for item in parse_js(&src.read(f)?)? {
            let t = item.get("tweet").unwrap_or(&item);
            let id = s(t, "id_str");
            let text = if s(t, "full_text").is_empty() { s(t, "text") } else { s(t, "full_text") };
            if id.is_empty() || text.starts_with("RT @") {
                continue; // reposts are someone else's words
            }
            let ts = parse_twitter(s(t, "created_at")).unwrap_or(0);
            let who = if username.is_empty() { "i/web".to_string() } else { username.clone() };
            let link = format!("https://x.com/{who}/status/{id}");
            w.message(&format!("post_{id}"), channel, ts, text, "", Some(&link))?;
        }
    }

    // Direct messages you sent
    let dm_files: Vec<String> = names.iter().filter(|n| is_dm_file(&lower(n))).cloned().collect();
    if !dm_files.is_empty() && account_id.is_empty() {
        summary.warnings.push("Direct messages were skipped because the archive has no account.js to say which are yours.".to_string());
    } else {
        for f in &dm_files {
            let group = lower(f).contains("group");
            for conv in parse_js(&src.read(f)?)? {
                let Some(c) = conv.get("dmConversation") else { continue };
                let conv_id = s(c, "conversationId").to_string();
                let mut channel = None;
                for m in c.get("messages").and_then(|m| m.as_array()).into_iter().flatten() {
                    let Some(mc) = m.get("messageCreate") else { continue };
                    if s(mc, "senderId") != account_id {
                        continue;
                    }
                    let cid = match channel {
                        Some(c) => c,
                        None => {
                            let name = if group { "Group chat" } else { "Direct message" };
                            let kind = if group { "group_dm" } else { "dm" };
                            let c = w.channel(&format!("dm:{conv_id}"), name, kind, None, None)?;
                            channel = Some(c);
                            c
                        }
                    };
                    let ts = parse_iso(s(mc, "createdAt")).unwrap_or(0);
                    let link = format!("https://x.com/messages/{conv_id}");
                    w.message(&format!("dm_{}", s(mc, "id")), cid, ts, s(mc, "text"), "", Some(&link))?;
                }
            }
        }
    }
    progress(ImportProgress { phase: "Importing posts", done: total, total });
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::is_tweets_file;
    #[test]
    fn tweet_files() {
        assert!(is_tweets_file("twitter-2024/data/tweets.js"));
        assert!(is_tweets_file("data/tweet.js"));
        assert!(is_tweets_file("data/tweets-part2.js"));
        assert!(!is_tweets_file("data/tweetdeck.js"));
        assert!(!is_tweets_file("data/tweets-media/1.jpg"));
    }
}
