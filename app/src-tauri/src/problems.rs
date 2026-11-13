//! A local problems log for testers: crashes in the app and errors in the window are written
//! to problems.log next to Sompio's data. Nothing is sent anywhere; people attach the file
//! to a bug report themselves. Only error messages go in, never passwords or messages.

use crate::CmdResult;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static LOG: OnceLock<PathBuf> = OnceLock::new();
const MAX_BYTES: u64 = 512 * 1024;

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn write(line: &str) {
    let Some(path) = LOG.get() else { return };
    let line = redact(line);
    // Keep it small: start over when it grows too big
    if std::fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {}", stamp(), line.replace('\n', "\n    "));
    }
}

/// Starts the log and records crashes (panics) in it.
pub fn start(dir: &Path) {
    let _ = LOG.set(dir.join("problems.log"));
    write(&format!("Sompio {} started on {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH));
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown".into());
        write(&format!("CRASH{place}: {msg}"));
        default(info);
    }));
}

/// Errors from the window (a screen that broke, a failed command).
#[tauri::command]
pub fn log_problem(text: String) {
    let t: String = text.chars().take(2000).collect();
    write(&format!("WINDOW: {t}"));
}

#[derive(Serialize)]
pub struct Details {
    version: String,
    os: String,
    log: String,
    recent: Vec<String>,
}

/// What to paste into a bug report: version, system and the last lines of the log.
#[tauri::command]
pub fn app_details(app: AppHandle) -> CmdResult<Details> {
    let log = LOG.get().cloned().unwrap_or_default();
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let recent: Vec<String> = text.lines().rev().take(30).map(str::to_string).collect::<Vec<_>>().into_iter().rev().collect();
    Ok(Details {
        version: app.package_info().version.to_string(),
        os: format!("{} {} ({})", std::env::consts::OS, os_version(), std::env::consts::ARCH),
        log: redact(&log.display().to_string()),
        recent: recent.iter().map(|l| redact(l)).collect(),
    })
}

/// Takes the Windows user name out of folder paths, so a bug report doesn't say who you are:
/// "C:\\Users\\Anna\\AppData" becomes "C:\\Users\\you\\AppData" (also /home/anna and /Users/anna).
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    // This PC's own home folder first: exact, even when the name has spaces in it
    for (var, generic) in [("USERPROFILE", r"C:\Users\you"), ("HOME", "/home/you")] {
        if let Ok(home) = std::env::var(var) {
            if home.len() > 3 {
                out = replace_ignoring_case(&out, &home, generic);
            }
        }
    }
    // Then any other user folder
    for marker in ["\\users\\", "/users/", "/home/"] {
        out = mask_after(&out, marker);
    }
    // The name on its own too (for example in an error message), when it's long enough not to be a common word
    if let Some(name) = std::env::var("USERNAME").ok().or_else(|| std::env::var("USER").ok()) {
        if name.chars().count() >= 4 && !name.eq_ignore_ascii_case("you") {
            out = replace_ignoring_case(&out, &name, "you");
        }
    }
    out
}

/// Replaces the folder name right after `marker` (matched ignoring case) with "you".
fn mask_after(text: &str, marker: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(found) = lower[i..].find(marker) {
        let start = i + found + marker.len();
        out.push_str(&text[i..start]);
        let end = text[start..].find(|c: char| c == '\\' || c == '/' || c.is_whitespace() || c == '"' || c == '\'' || c == ':' || c == ')').map_or(text.len(), |n| start + n);
        let name = &text[start..end];
        let keep = name.is_empty() || ["you", "public", "default", "all users"].iter().any(|k| name.eq_ignore_ascii_case(k));
        out.push_str(if keep { name } else { "you" });
        i = end;
    }
    out.push_str(&text[i..]);
    out
}

fn replace_ignoring_case(text: &str, find: &str, with: &str) -> String {
    let (lower, needle) = (text.to_ascii_lowercase(), find.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(found) = lower[i..].find(&needle) {
        out.push_str(&text[i..i + found]);
        out.push_str(with);
        i += found + needle.len();
    }
    out.push_str(&text[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names_leave_paths() {
        assert_eq!(replace_ignoring_case(r"at c:\users\anna maria\AppData", r"C:\Users\Anna Maria", r"C:\Users\you"), r"at C:\Users\you\AppData");
        assert_eq!(mask_after(r"C:\users\anna\x and D:\Users\Bob\y", "\\users\\"), r"C:\users\you\x and D:\Users\you\y");
        assert_eq!(mask_after(r"C:\Users\Public\x", "\\users\\"), r"C:\Users\Public\x");
        assert_eq!(mask_after("/home/anna/.cargo/registry", "/home/"), "/home/you/.cargo/registry");
        assert_eq!(mask_after("no paths here", "/home/"), "no paths here");
        assert_eq!(replace_ignoring_case("Failed for ANNA and anna", "Anna", "you"), "Failed for you and you");
    }
}

#[tauri::command]
pub fn open_problems_folder(app: AppHandle) -> CmdResult<()> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let file = dir.join("problems.log");
    if !file.exists() {
        write("Log opened from Security");
    }
    tauri_plugin_opener::reveal_item_in_dir(&file).map_err(|e| format!("Couldn't open the folder: {e}"))
}

#[cfg(windows)]
fn os_version() -> String {
    // "Microsoft Windows [Version 10.0.22631.4460]"
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .creation_flags_hidden()
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_start_matches("Microsoft Windows").trim().trim_matches(['[', ']']).to_string())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn os_version() -> String {
    String::new()
}

#[cfg(windows)]
trait Hidden {
    fn creation_flags_hidden(&mut self) -> &mut Self;
}

#[cfg(windows)]
impl Hidden for std::process::Command {
    fn creation_flags_hidden(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up
        self.creation_flags(0x0800_0000)
    }
}
