//! Auto-type: which entry belongs to the window in front, and what to type, in KeePass's
//! sequence language ({USERNAME}{TAB}{PASSWORD}{ENTER}, {DELAY 500}, ^a for Ctrl+A…).

use crate::store::{field, EntrySummary, Vault, DEFAULT_SEQUENCE, OTP};
use keepass::db::fields;
use serde::Serialize;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Key {
    Tab, Enter, Space, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, Escape,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Typed as characters, never as shortcuts
    Text(String),
    Key { key: Key, mods: Mods, times: u32 },
    /// A character with modifiers held, like ^a (Ctrl+A)
    Combo { ch: char, mods: Mods },
    /// Wait this many milliseconds
    Delay(u64),
    /// Pause between every keystroke from here on
    KeyDelay(u64),
    /// A field to type: USERNAME, PASSWORD, TITLE, URL, NOTES, TOTP, or S:Name
    Field(String),
    /// Clear the field first: select all and delete
    ClearField,
}

fn key_name(name: &str) -> Option<Key> {
    Some(match name {
        "TAB" => Key::Tab,
        "ENTER" | "~" => Key::Enter,
        "SPACE" => Key::Space,
        "BACKSPACE" | "BS" | "BKSP" => Key::Backspace,
        "DELETE" | "DEL" => Key::Delete,
        "INSERT" | "INS" => Key::Insert,
        "UP" => Key::Up,
        "DOWN" => Key::Down,
        "LEFT" => Key::Left,
        "RIGHT" => Key::Right,
        "HOME" => Key::Home,
        "END" => Key::End,
        "PGUP" => Key::PageUp,
        "PGDN" => Key::PageDown,
        "ESC" => Key::Escape,
        f if f.starts_with('F') && f.len() <= 3 => {
            let n: u8 = f[1..].parse().ok()?;
            if (1..=16).contains(&n) { Key::F(n) } else { return None }
        }
        _ => return None,
    })
}

/// Reads a sequence. Errors say what's wrong in plain words.
pub fn parse(seq: &str) -> Result<Vec<Step>, String> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut mods = Mods::default();
    let chars: Vec<char> = seq.chars().collect();
    let mut i = 0;
    let flush = |text: &mut String, out: &mut Vec<Step>| {
        if !text.is_empty() {
            out.push(Step::Text(std::mem::take(text)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '+' | '^' | '%' => {
                flush(&mut text, &mut out);
                match c {
                    '+' => mods.shift = true,
                    '^' => mods.ctrl = true,
                    _ => mods.alt = true,
                }
                i += 1;
                continue;
            }
            '~' => {
                flush(&mut text, &mut out);
                out.push(Step::Key { key: Key::Enter, mods, times: 1 });
            }
            '{' => {
                let end = chars[i + 1..].iter().position(|&x| x == '}').map(|p| p + i + 1);
                // "{}}" is a literal closing brace
                let end = if chars.get(i + 1) == Some(&'}') && chars.get(i + 2) == Some(&'}') { Some(i + 2) } else { end };
                let Some(end) = end else { return Err("a { has no closing }".into()) };
                let inner: String = chars[i + 1..end].iter().collect();
                i = end;
                let (name, arg) = match inner.split_once([' ', '=']) {
                    Some((n, a)) if !n.is_empty() => (n.to_string(), Some(a.trim().to_string())),
                    _ => (inner.clone(), None),
                };
                let upper = name.to_uppercase();
                if inner.len() == 1 && "+^%~(){}[]".contains(inner.as_str()) {
                    if mods != Mods::default() {
                        flush(&mut text, &mut out);
                        out.push(Step::Combo { ch: inner.chars().next().unwrap_or(' '), mods });
                    } else {
                        text.push_str(&inner);
                    }
                } else if let Some(field) = upper.strip_prefix("S:").map(|_| format!("S:{}", &inner[2..])) {
                    flush(&mut text, &mut out);
                    out.push(Step::Field(field));
                } else if ["USERNAME", "PASSWORD", "TITLE", "URL", "NOTES", "TOTP"].contains(&upper.as_str()) && arg.is_none() {
                    flush(&mut text, &mut out);
                    out.push(Step::Field(upper));
                } else if upper == "DELAY" {
                    flush(&mut text, &mut out);
                    let ms: u64 = arg.as_deref().and_then(|a| a.parse().ok()).filter(|m| *m <= 10_000).ok_or("{DELAY} needs milliseconds, up to 10000, like {DELAY 500}")?;
                    out.push(if inner.contains('=') { Step::KeyDelay(ms) } else { Step::Delay(ms) });
                } else if upper == "CLEARFIELD" {
                    flush(&mut text, &mut out);
                    out.push(Step::ClearField);
                } else if let Some(key) = key_name(&upper) {
                    flush(&mut text, &mut out);
                    let times: u32 = match arg {
                        Some(a) => a.parse().ok().filter(|n| (1..=100).contains(n)).ok_or(format!("{{{name} {a}}} needs a count from 1 to 100"))?,
                        None => 1,
                    };
                    out.push(Step::Key { key, mods, times });
                } else {
                    return Err(format!("{{{inner}}} isn't something auto-type knows"));
                }
            }
            _ => {
                if mods != Mods::default() {
                    flush(&mut text, &mut out);
                    out.push(Step::Combo { ch: c, mods });
                } else {
                    text.push(c);
                }
            }
        }
        mods = Mods::default();
        i += 1;
    }
    flush(&mut text, &mut out);
    Ok(out)
}

/// KeePass window patterns: case-insensitive, * matches anything; without a * it must match whole.
pub fn window_matches(pattern: &str, title: &str) -> bool {
    let p = pattern.trim().to_lowercase();
    let t = title.to_lowercase();
    if p.is_empty() {
        return false;
    }
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 1 {
        return t == p;
    }
    let mut pos = 0;
    for (n, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match t[pos..].find(part) {
            Some(found) if n > 0 || found == 0 => pos += found + part.len(),
            _ => return false,
        }
    }
    parts.last().is_some_and(|l| l.is_empty()) || t.ends_with(parts.last().unwrap_or(&""))
}

/// "github" from https://www.github.com/login: what usually shows in a browser tab title.
fn site_name(url: &str) -> Option<String> {
    let host = url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#', ':']).next()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let labels: Vec<&str> = host.split('.').collect();
    let name = if labels.len() >= 2 { labels[labels.len() - 2] } else { labels[0] };
    (name.len() >= 3).then(|| name.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub entry: EntrySummary,
    pub sequence: String,
    /// Why it matched, for the chooser
    pub because: &'static str,
}

/// The entries for the window in front, best first.
pub fn matches(v: &Vault, window_title: &str) -> Vec<Match> {
    let t = window_title.to_lowercase();
    let mut out = Vec::new();
    for e in v.live_entries() {
        let at = e.autotype.clone();
        if at.as_ref().is_some_and(|a| !a.enabled) || e.parent().enable_autotype == Some(false) {
            continue;
        }
        let user = field(&e, fields::USERNAME);
        let default = at
            .as_ref()
            .and_then(|a| a.default_sequence.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| e.parent().default_autotype_sequence.clone().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| if user.is_empty() { "{PASSWORD}{ENTER}".into() } else { DEFAULT_SEQUENCE.into() });
        let assoc = at.as_ref().and_then(|a| a.associations.iter().find(|x| window_matches(&x.window, window_title)));
        let title = field(&e, fields::TITLE).to_lowercase();
        let (rank, because, seq) = if let Some(a) = assoc {
            (0, "window rule", if a.sequence.is_empty() { default } else { a.sequence.clone() })
        } else if title.len() >= 3 && t.contains(&title) {
            (1, "title", default)
        } else if site_name(field(&e, fields::URL)).is_some_and(|s| t.contains(&s)) {
            (2, "website", default)
        } else {
            continue;
        };
        out.push((rank, Match { entry: v.summary_of(&e), sequence: seq, because }));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.entry.title.to_lowercase().cmp(&b.1.entry.title.to_lowercase())));
    out.into_iter().map(|(_, m)| m).collect()
}

/// Fills in the fields, giving the final keystrokes. Secrets are wiped when dropped.
pub struct Typing(pub Vec<Step>);

impl Drop for Typing {
    fn drop(&mut self) {
        for s in &mut self.0 {
            if let Step::Text(t) = s {
                zeroize::Zeroize::zeroize(t);
            }
        }
    }
}

pub fn resolve(v: &mut Vault, entry_id: &str, sequence: &str) -> anyhow::Result<Typing> {
    let steps = parse(sequence).map_err(|m| anyhow::anyhow!("Auto-type sequence: {m}"))?;
    let mut out = Vec::with_capacity(steps.len());
    for s in steps {
        match s {
            Step::Field(f) => {
                let value: Zeroizing<String> = match f.as_str() {
                    "USERNAME" => v.reveal(entry_id, fields::USERNAME)?,
                    "PASSWORD" => v.reveal(entry_id, fields::PASSWORD)?,
                    "TITLE" => v.reveal(entry_id, fields::TITLE)?,
                    "URL" => v.reveal(entry_id, fields::URL)?,
                    "NOTES" => v.reveal(entry_id, fields::NOTES)?,
                    "TOTP" => Zeroizing::new(v.totp(entry_id)?.map(|t| t.code).ok_or_else(|| anyhow::anyhow!("This entry has no 2FA set up."))?),
                    other => v.reveal(entry_id, other.strip_prefix("S:").unwrap_or(other))?,
                };
                // Line breaks in notes become Enter presses
                for (n, line) in value.split('\n').enumerate() {
                    if n > 0 {
                        out.push(Step::Key { key: Key::Enter, mods: Mods::default(), times: 1 });
                    }
                    let line = line.trim_end_matches('\r');
                    if !line.is_empty() {
                        out.push(Step::Text(line.to_string()));
                    }
                }
            }
            other => out.push(other),
        }
    }
    let _ = OTP;
    Ok(Typing(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{AutoTypeSettings, EntryInput};

    #[test]
    fn parses_keepass_sequences() {
        let s = parse("{USERNAME}{TAB}{PASSWORD}{ENTER}").unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s[1], Step::Key { key: Key::Tab, mods: Mods::default(), times: 1 });
        let s = parse("^a{DEL}{TAB 2}{DELAY 300}x{+}{S:PIN}{DELAY=50}").unwrap();
        assert_eq!(s[0], Step::Combo { ch: 'a', mods: Mods { ctrl: true, ..Default::default() } });
        assert_eq!(s[2], Step::Key { key: Key::Tab, mods: Mods::default(), times: 2 });
        assert_eq!(s[3], Step::Delay(300));
        assert_eq!(s[4], Step::Text("x+".into()));
        assert_eq!(s[5], Step::Field("S:PIN".into()));
        assert_eq!(s[6], Step::KeyDelay(50));
        assert!(parse("{NOPE}").is_err());
        assert!(parse("{TAB").is_err());
    }

    #[test]
    fn matches_windows_and_types() {
        assert!(window_matches("*GitHub*", "Sign in to GitHub · GitHub — Mozilla Firefox"));
        assert!(!window_matches("GitHub", "Sign in to GitHub"));
        assert!(window_matches("Zoom*", "Zoom Login"));
        let dir = tempfile::tempdir().unwrap();
        let mut v = crate::store::Vault::open(&dir.path().join("v.kdbx"), &[4u8; 32]).unwrap();
        let gh = v.add_entry(&EntryInput { title: "Work code".into(), username: "sk".into(), password: Some("p\nw".into()), url: "https://github.com/login".into(), ..Default::default() }).unwrap();
        v.add_entry(&EntryInput {
            title: "Game".into(),
            password: Some("x".into()),
            autotype: Some(AutoTypeSettings { enabled: true, sequence: "{PASSWORD}{ENTER}".into(), windows: vec!["Zoom*".into()] }),
            ..Default::default()
        })
        .unwrap();
        let m = matches(&v, "Sign in to GitHub · GitHub - Google Chrome");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].because, "website");
        let m = matches(&v, "Zoom Login");
        assert_eq!((m[0].entry.title.as_str(), m[0].sequence.as_str()), ("Game", "{PASSWORD}{ENTER}"));
        let t = resolve(&mut v, &gh, DEFAULT_SEQUENCE).unwrap();
        assert_eq!(t.0[0], Step::Text("sk".into()));
        assert_eq!(t.0[2], Step::Text("p".into()));
        assert!(matches!(t.0[3], Step::Key { key: Key::Enter, .. }), "line break in a password becomes Enter");
    }
}
