//! Reddit's data export (Settings > Privacy > Request your data). A zip of CSV files.
//!
//! posts.csv and comments.csv only hold things you wrote. messages.csv and
//! chat_history.csv hold both sides, so only rows sent from your own username are kept;
//! the username comes from statistics.csv. Without it, private messages are skipped.

use super::{file_name_lower, Csv, ImportProgress, ImportSummary, Source, Writer};
use crate::time::parse_iso;
use anyhow::Result;

fn find<'a>(names: &'a [String], file: &str) -> Option<&'a String> {
    names.iter().find(|n| file_name_lower(n) == file)
}

fn full_link(permalink: &str) -> Option<String> {
    let p = permalink.trim();
    if p.is_empty() {
        None
    } else if p.starts_with("http") {
        Some(p.to_string())
    } else {
        Some(format!("https://www.reddit.com{p}"))
    }
}

fn own_username(src: &mut Source, names: &[String]) -> Result<Option<String>> {
    let Some(f) = find(names, "statistics.csv") else { return Ok(None) };
    let csv = Csv::parse(&src.read(f)?)?;
    Ok(csv
        .rows
        .iter()
        .find(|r| csv.get(r, "statistic").trim().eq_ignore_ascii_case("account name"))
        .map(|r| csv.get(r, "value").trim().to_string())
        .filter(|s| !s.is_empty()))
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    progress(ImportProgress { phase: "Importing messages", done: 0, total: 4 });

    // Posts
    if let Some(f) = find(names, "posts.csv") {
        let csv = Csv::parse(&src.read(f)?)?;
        for r in &csv.rows {
            let id = csv.get(r, "id").trim();
            if id.is_empty() {
                continue;
            }
            let sub = csv.get(r, "subreddit").trim();
            let channel = if sub.is_empty() {
                w.channel("posts", "Your Reddit posts", "posts", None, None)?
            } else {
                w.channel(&format!("r/{sub}"), &format!("r/{sub}"), "community", None, None)?
            };
            let mut text = csv.get(r, "title").trim().to_string();
            let body = csv.get(r, "body").trim();
            if !body.is_empty() {
                text.push_str("\n\n");
                text.push_str(body);
            }
            let url = csv.get(r, "url").trim();
            let permalink = csv.get(r, "permalink");
            if !url.is_empty() && !url.contains("/comments/") {
                text.push_str("\n\n");
                text.push_str(url);
            }
            let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
            w.message(&format!("t3_{id}"), channel, ts, &text, "", full_link(permalink).as_deref())?;
        }
    }
    progress(ImportProgress { phase: "Importing messages", done: 1, total: 4 });

    // Comments
    if let Some(f) = find(names, "comments.csv") {
        let csv = Csv::parse(&src.read(f)?)?;
        for r in &csv.rows {
            let id = csv.get(r, "id").trim();
            if id.is_empty() {
                continue;
            }
            let sub = csv.get(r, "subreddit").trim();
            let ext = if sub.is_empty() { "comments".to_string() } else { format!("r/{sub}") };
            let name = if sub.is_empty() { "Your Reddit comments".to_string() } else { format!("r/{sub}") };
            let channel = w.channel(&ext, &name, "community", None, None)?;
            let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
            w.message(&format!("t1_{id}"), channel, ts, csv.get(r, "body"), "", full_link(csv.get(r, "permalink")).as_deref())?;
        }
    }
    progress(ImportProgress { phase: "Importing messages", done: 2, total: 4 });

    // Private messages and chats: only what you sent.
    let me = own_username(src, names)?;
    let has_private = find(names, "messages.csv").is_some() || find(names, "chat_history.csv").is_some();
    match &me {
        None if has_private => summary.warnings.push(
            "Private messages were skipped because the export doesn't say which username is yours.".to_string(),
        ),
        Some(me) => {
            if let Some(f) = find(names, "messages.csv") {
                let csv = Csv::parse(&src.read(f)?)?;
                let channel = w.channel("messages", "Reddit private messages", "dm", None, None)?;
                for r in csv.rows.iter().filter(|r| csv.get(r, "from").trim().eq_ignore_ascii_case(me)) {
                    let id = csv.get(r, "id").trim();
                    let mut text = csv.get(r, "subject").trim().to_string();
                    let body = csv.get(r, "body").trim();
                    if !body.is_empty() {
                        if !text.is_empty() {
                            text.push_str("\n\n");
                        }
                        text.push_str(body);
                    }
                    let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
                    w.message(&format!("dm_{id}"), channel, ts, &text, "", full_link(csv.get(r, "permalink")).as_deref())?;
                }
            }
            if let Some(f) = find(names, "chat_history.csv") {
                let csv = Csv::parse(&src.read(f)?)?;
                for r in csv.rows.iter().filter(|r| csv.get(r, "username").trim().eq_ignore_ascii_case(me)) {
                    let id = csv.get(r, "message_id").trim();
                    let chan_url = csv.get(r, "channel_url").trim();
                    let chan_name = csv.get(r, "channel_name").trim();
                    let name = if chan_name.is_empty() { "Reddit chat".to_string() } else { format!("Chat: {chan_name}") };
                    let channel = w.channel(&format!("chat:{chan_url}"), &name, "dm", None, None)?;
                    let ts = parse_iso(csv.get(r, "created_at")).unwrap_or(0);
                    w.message(&format!("chat_{id}"), channel, ts, csv.get(r, "message"), "", None)?;
                }
            }
        }
        None => {}
    }
    progress(ImportProgress { phase: "Importing messages", done: 4, total: 4 });
    Ok(summary)
}
