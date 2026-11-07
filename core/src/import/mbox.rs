//! Email from a mailbox file (.mbox), such as Gmail's Google Takeout export.
//!
//! Only mail you sent is imported: Gmail marks it with an "X-Gmail-Labels: …Sent…" header,
//! and a file whose name says "sent" is taken as sent mail. The mailbox is streamed, never
//! loaded whole, because Takeout files can be many gigabytes.
//! Quoted replies (lines starting with ">", "On … wrote:") are cut off: that's someone else's text.

use super::{file_name_lower, ImportProgress, ImportSummary, Source, Writer};
use anyhow::Result;
use mail_parser::MessageParser;
use std::io::{BufRead, BufReader, Read};

/// Parsed sent email, ready to store.
#[derive(Debug, Clone)]
pub struct Mail {
    pub id: String,
    pub ts: i64,
    pub text: String,
}

/// Turns raw email bytes into the text Veil scans: recipients, subject, and your own words.
pub(crate) fn parse_mail(raw: &[u8]) -> Option<Mail> {
    let msg = MessageParser::default().parse(raw)?;
    let to: Vec<String> = msg
        .to()
        .map(|a| a.iter().filter_map(|x| x.address().map(str::to_string)).collect())
        .unwrap_or_default();
    let subject = msg.subject().unwrap_or("").to_string();
    let body = msg.body_text(0).map(|b| b.to_string()).unwrap_or_default();
    let mut own = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('>') {
            continue;
        }
        if (t.starts_with("On ") && t.ends_with("wrote:")) || t.starts_with("-----Original Message") || t == "________________________________" {
            break; // the rest is the message being replied to
        }
        own.push(line);
    }
    let body = own.join("\n").trim().to_string();
    let mut text = String::new();
    if !to.is_empty() {
        text.push_str(&format!("To: {}\n", to.join(", ")));
    }
    if !subject.is_empty() {
        text.push_str(&format!("Subject: {subject}\n"));
    }
    text.push('\n');
    text.push_str(&body);
    if text.len() > 100_000 {
        let mut cut = 100_000;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    let ts = msg.date().map(|d| d.to_timestamp() * 1000).unwrap_or(0);
    let id = msg.message_id().map(str::to_string).unwrap_or_else(|| format!("{ts}-{}", subject.len()));
    Some(Mail { id, ts, text })
}

/// Splits an mbox stream into messages and calls `f` with each one's raw bytes.
fn each_message(r: &mut dyn Read, mut f: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    let mut reader = BufReader::with_capacity(1 << 20, r);
    let mut current: Vec<u8> = Vec::new();
    let mut line = Vec::new();
    let mut prev_blank = true;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if prev_blank && line.starts_with(b"From ") {
            if !current.is_empty() {
                f(&current)?;
                current.clear();
            }
            prev_blank = false;
            continue; // the separator line isn't part of the message
        }
        prev_blank = line == b"\n" || line == b"\r\n";
        // mboxrd escapes ">From " inside messages
        if line.starts_with(b">From ") {
            current.extend_from_slice(&line[1..]);
        } else {
            current.extend_from_slice(&line);
        }
    }
    if !current.is_empty() {
        f(&current)?;
    }
    Ok(())
}

fn is_sent(raw: &[u8]) -> bool {
    // Look only at the headers.
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len());
    let headers = String::from_utf8_lossy(&raw[..end]);
    headers.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("x-gmail-labels:") && l.split(|c: char| c == ',' || c == ':').any(|p| p.trim() == "sent")
    })
}

pub(super) fn import(
    src: &mut Source,
    names: &[String],
    w: &mut Writer,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    let files: Vec<String> = names.iter().filter(|n| n.to_ascii_lowercase().ends_with(".mbox")).cloned().collect();
    let cid = w.channel("sent", "Sent mail", "posts", None, None)?;
    let mut total = 0usize;
    for f in &files {
        let whole_file_sent = file_name_lower(f).contains("sent");
        src.with_reader(f, |r| {
            each_message(r, |raw| {
                total += 1;
                if total % 2000 == 0 {
                    progress(ImportProgress { phase: "Reading mailbox", done: total, total: 0 });
                }
                if !(whole_file_sent || is_sent(raw)) {
                    return Ok(());
                }
                if let Some(m) = super::parse_sent_mail(raw) {
                    w.message(&m.id, cid, m.ts, &m.text, "", Some(&super::gmail_link(&m.id)))?;
                }
                Ok(())
            })
        })?;
    }
    if w.messages == 0 && total > 0 {
        summary.warnings.push(format!("Read {total} emails, but none were marked as sent by you."));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn splits_and_keeps_only_your_words() {
        let mbox = "From 1@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Sent,Important\nMessage-ID: <a@b>\nTo: bank@example.org\nSubject: Account\nDate: Mon, 1 Jan 2024 10:00:00 +0000\n\nMy IBAN is NL91 ABNA 0417 1643 00\n>From the archives\n\nOn Sun, someone wrote:\n> their private stuff\n\nFrom 2@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Inbox\nSubject: Newsletter\n\nhello\n";
        let mut got = vec![];
        each_message(&mut mbox.as_bytes(), |raw| {
            got.push((is_sent(raw), raw.to_vec()));
            Ok(())
        })
        .unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].0 && !got[1].0);
        let m = parse_mail(&got[0].1).unwrap();
        assert!(m.text.contains("To: bank@example.org") && m.text.contains("NL91"));
        assert!(m.text.contains("From the archives"), "unescaped mboxrd line");
        assert!(!m.text.contains("their private stuff"));
        assert_eq!(m.id, "a@b");
        assert_eq!(m.ts, 1_704_103_200_000);
    }
}
