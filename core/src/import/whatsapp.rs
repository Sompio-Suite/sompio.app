//! WhatsApp chat exports (in a chat: More > Export chat). Each chat is a text file, sometimes
//! zipped with its media. Lines look like:
//!   Android: "31/12/2020, 21:41 - Mara: see you"
//!   iPhone:  "[31/12/2020, 21:41:05] Mara: see you"
//! Date order and 12/24-hour clocks follow the phone's language, so both are worked out per file.
//!
//! WhatsApp doesn't say which messages are yours. In a one-to-one chat the file is named after
//! the other person, so the other sender is you. In groups, Veil uses the names set under
//! "Your names" on the Sources screen.

use super::{split_parent, Ctx, ImportProgress, ImportSummary, Source, Writer};
use crate::time::civil_to_ms;
use anyhow::{bail, Result};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};

pub(crate) fn is_chat_file(name_lower: &str) -> bool {
    let file = split_parent(name_lower).1;
    file.ends_with(".txt") && (file.contains("whatsapp") || file == "_chat.txt")
}

struct Line {
    a: i64,
    b: i64,
    y: i64,
    h: i64,
    m: i64,
    s: i64,
    pm: Option<bool>,
    sender: String,
    text: String,
}

fn parse_lines(text: &str) -> Vec<Line> {
    let re = Regex::new(
        r"^\[?(\d{1,2})[/.\-](\d{1,2})[/.\-](\d{2,4}),?\s+(\d{1,2})[:.](\d{2})(?:[:.](\d{2}))?\s?([AaPp])?\.?\s?(?:[Mm]\.?)?\]?\s*(?:[-–]\s)?(.*)$",
    )
    .unwrap();
    let mut out: Vec<Line> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_start_matches(['\u{200e}', '\u{200f}', '\u{feff}']).trim_end_matches('\r');
        if let Some(c) = re.captures(line) {
            let n = |i: usize| c.get(i).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let rest = c.get(8).map(|m| m.as_str()).unwrap_or("");
            let Some((sender, text)) = rest.split_once(": ") else {
                // A system line ("Messages are end-to-end encrypted", "Mara joined"): not a message.
                out.push(Line { a: 0, b: 0, y: 0, h: 0, m: 0, s: 0, pm: None, sender: String::new(), text: String::new() });
                continue;
            };
            out.push(Line {
                a: n(1),
                b: n(2),
                y: n(3),
                h: n(4),
                m: n(5),
                s: n(6),
                pm: c.get(7).map(|m| m.as_str().eq_ignore_ascii_case("p")),
                sender: sender.trim_start_matches(['\u{200e}', '~', ' ']).trim().to_string(),
                text: text.to_string(),
            });
        } else if let Some(last) = out.last_mut() {
            // A message that runs over several lines.
            if !last.sender.is_empty() {
                last.text.push('\n');
                last.text.push_str(line);
            }
        }
    }
    out.retain(|l| !l.sender.is_empty());
    out
}

fn is_media_placeholder(t: &str) -> bool {
    let t = t.trim();
    t.starts_with('<') && t.ends_with('>') || t.ends_with("(file attached)") || t == "null" || t.contains("omitted")
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    ctx: &Ctx,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let files: Vec<String> = names.iter().filter(|n| is_chat_file(&n.to_ascii_lowercase())).cloned().collect();
    let mut summary = ImportSummary::default();
    let mut groups_skipped = 0;
    for (i, f) in files.iter().enumerate() {
        progress(ImportProgress { phase: "Importing messages", done: i, total: files.len() });
        let text = String::from_utf8_lossy(&src.read(f)?).into_owned();
        let lines = parse_lines(&text);
        if lines.is_empty() {
            continue;
        }
        // Chat name from the file name: "WhatsApp Chat with Mara.txt" (or the folder for "_chat.txt").
        let (parent, file) = split_parent(f);
        let stem = file.trim_end_matches(".txt");
        let chat = match stem.find(" with ").or_else(|| stem.find(" mit ")).or_else(|| stem.find(" con ")).or_else(|| stem.find(" met ")) {
            Some(i) => stem[i..].splitn(3, ' ').nth(2).unwrap_or(stem).to_string(),
            None if stem == "_chat" => split_parent(parent).1.trim_start_matches("WhatsApp Chat - ").to_string(),
            None => stem.to_string(),
        };
        let senders: BTreeSet<&str> = lines.iter().map(|l| l.sender.as_str()).collect();
        let me: Option<String> = if let Some(n) = senders.iter().find(|s| ctx.is_me(s)) {
            Some(n.to_string())
        } else if senders.len() == 2 && senders.contains(chat.as_str()) {
            senders.iter().find(|s| **s != chat).map(|s| s.to_string())
        } else {
            None
        };
        let Some(me) = me else {
            groups_skipped += 1;
            continue;
        };

        // Day-first or month-first? Any first number above 12 settles it; so does a 12-hour clock (US style).
        let day_first = if lines.iter().any(|l| l.a > 12) {
            true
        } else if lines.iter().any(|l| l.b > 12) {
            false
        } else {
            !lines.iter().any(|l| l.pm.is_some())
        };
        let kind = if senders.len() > 2 { "group_dm" } else { "dm" };
        let cid = w.channel(&chat, &format!("WhatsApp: {chat}"), kind, None, None)?;
        let mut seen: HashMap<i64, usize> = HashMap::new();
        for l in lines.iter().filter(|l| l.sender == me) {
            if is_media_placeholder(&l.text) {
                continue;
            }
            let (d, mo) = if day_first { (l.a, l.b) } else { (l.b, l.a) };
            let y = if l.y < 100 { 2000 + l.y } else { l.y };
            let h = match l.pm {
                Some(true) if l.h < 12 => l.h + 12,
                Some(false) if l.h == 12 => 0,
                _ => l.h,
            };
            let ts = civil_to_ms(y, mo, d, h, l.m, l.s);
            let n = seen.entry(ts).or_default();
            *n += 1;
            w.message(&format!("{chat}|{ts}|{n}"), cid, ts, &l.text, "", None)?;
        }
    }
    if groups_skipped > 0 {
        if w.messages == 0 {
            bail!(
                "WhatsApp doesn't say which messages are yours in group chats. Add the name you use in WhatsApp under Sources > Your names, then import again."
            );
        }
        summary.warnings.push(format!(
            "{groups_skipped} group chat(s) were skipped because Veil couldn't tell which messages are yours. Add your WhatsApp name under Your names and import again."
        ));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_both_layouts() {
        let android = "31/12/2020, 21:41 - Messages and calls are end-to-end encrypted.\n31/12/2020, 21:41 - Mara: hi\n1/1/21, 9:05 - Sam: line one\nline two\n";
        let l = parse_lines(android);
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].sender, "Sam");
        assert_eq!(l[1].text, "line one\nline two");
        let ios = "\u{200e}[12/31/20, 9:41:05 PM] Sam: hello\n[1/1/21, 12:05:00 AM] Mara: hey";
        let l = parse_lines(ios);
        assert_eq!((l[0].a, l[0].b, l[0].h, l[0].pm), (12, 31, 9, Some(true)));
        assert_eq!(l[1].sender, "Mara");
    }
}
