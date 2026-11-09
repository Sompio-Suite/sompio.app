//! LinkedIn's data export (Settings > Data privacy > Get a copy of your data). A zip of CSVs:
//! messages.csv (both sides; only rows from you are kept), Shares.csv (your posts),
//! Comments.csv (your comments). Your name comes from Profile.csv.

use super::{file_name_lower, Csv, ImportProgress, ImportSummary, Source, Writer};
use crate::time::parse_iso;
use anyhow::Result;

fn find<'a>(names: &'a [String], file: &str) -> Option<&'a String> {
    names.iter().find(|n| file_name_lower(n) == file)
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    progress(ImportProgress { phase: "Reading LinkedIn export", done: 0, total: 3 });

    let mut me = String::new();
    if let Some(f) = find(names, "profile.csv") {
        let csv = Csv::parse(&src.read(f)?)?;
        if let Some(r) = csv.rows.first() {
            me = format!("{} {}", csv.get(r, "first name").trim(), csv.get(r, "last name").trim()).trim().to_string();
        }
    }

    if let Some(f) = find(names, "messages.csv") {
        if me.is_empty() {
            summary.warnings.push("Messages were skipped because the export has no Profile.csv to say which are yours.".to_string());
        } else {
            let csv = Csv::parse(&src.read(f)?)?;
            for (i, r) in csv.rows.iter().enumerate() {
                if !csv.get(r, "from").trim().eq_ignore_ascii_case(&me) {
                    continue;
                }
                let mut text = csv.get(r, "subject").trim().to_string();
                let body = csv.get(r, "content").trim();
                if !body.is_empty() {
                    if !text.is_empty() {
                        text.push_str("\n\n");
                    }
                    text.push_str(body);
                }
                if text.is_empty() {
                    continue;
                }
                let conv = csv.get(r, "conversation id").trim();
                let title = match csv.get(r, "conversation title").trim() {
                    "" => csv.get(r, "to").trim(),
                    t => t,
                };
                let cid = w.channel(&format!("msg:{conv}"), &format!("LinkedIn: {title}"), "dm", None, None)?;
                let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
                w.message(&format!("msg/{conv}/{ts}/{i}"), cid, ts, &text, "", None)?;
            }
        }
    }
    progress(ImportProgress { phase: "Reading LinkedIn export", done: 1, total: 3 });

    if let Some(f) = find(names, "shares.csv") {
        let csv = Csv::parse(&src.read(f)?)?;
        for (i, r) in csv.rows.iter().enumerate() {
            let text = csv.get(r, "sharecommentary").trim().trim_matches('"');
            if text.is_empty() {
                continue;
            }
            let cid = w.channel("posts", "Your LinkedIn posts", "posts", None, None)?;
            let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
            let link = csv.get(r, "sharelink").trim();
            let link = link.starts_with("https://www.linkedin.com/").then_some(link);
            w.message(&format!("share/{ts}/{i}"), cid, ts, text, "", link)?;
        }
    }
    progress(ImportProgress { phase: "Reading LinkedIn export", done: 2, total: 3 });

    if let Some(f) = find(names, "comments.csv") {
        let csv = Csv::parse(&src.read(f)?)?;
        for (i, r) in csv.rows.iter().enumerate() {
            let text = csv.get(r, "message").trim();
            if text.is_empty() {
                continue;
            }
            let cid = w.channel("comments", "Your LinkedIn comments", "community", None, None)?;
            let ts = parse_iso(csv.get(r, "date")).unwrap_or(0);
            let link = csv.get(r, "link").trim();
            let link = link.starts_with("https://www.linkedin.com/").then_some(link);
            w.message(&format!("comment/{ts}/{i}"), cid, ts, text, "", link)?;
        }
    }
    Ok(summary)
}
