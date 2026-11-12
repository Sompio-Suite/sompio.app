//! Passes messages between the Sompio browser extension and the Sompio app, on this PC only.
//!
//! The browser starts this helper (native messaging) and talks to it over stdin/stdout: each
//! message is a 4-byte length and JSON. The helper sends each one on to Sompio through a local
//! pipe, and sends the answer back. It holds no keys or passwords: requests are encrypted by the
//! extension for Sompio, and only Sompio can read them.

use serde_json::{json, Value};

pub const HOST_NAME: &str = "app.sompio.browser";
pub const FIREFOX_ID: &str = "browser@sompio.org";
/// Messages are small; anything bigger is refused
pub const MAX_FRAME: usize = 1024 * 1024;

#[cfg(windows)]
pub fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!(r"\\.\pipe\sompio-browser-{user}")
}

#[cfg(unix)]
pub fn pipe_name() -> String {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    let user = std::env::var("USER").unwrap_or_default();
    format!("{dir}/sompio-browser-{user}.sock")
}


/// The browser starts the host with the extension's address (Chrome) or id (Firefox) as an argument.
pub fn is_host_launch() -> bool {
    std::env::args().skip(1).any(|a| a.starts_with("chrome-extension://") || a == FIREFOX_ID || a.ends_with(&format!("{HOST_NAME}.firefox.json")) || a.ends_with(&format!("{HOST_NAME}.json")))
}

#[cfg(windows)]
fn connect() -> std::io::Result<std::fs::File> {
    let mut tries = 0;
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe_name()) {
            Ok(f) => return Ok(f),
            // All pipe instances busy for a moment
            Err(e) if e.raw_os_error() == Some(231) && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(unix)]
fn connect() -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(pipe_name())
}

fn read_frame(r: &mut impl std::io::Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(std::io::Error::other("message too big"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(Some(buf))
}

fn write_frame(w: &mut impl std::io::Write, data: &[u8]) -> std::io::Result<()> {
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(data)?;
    w.flush()
}

/// Passes messages between the browser (stdin/stdout) and Sompio (the pipe). Holds no secrets
/// itself, and exits when the browser closes the connection.
pub fn run() {
    let (mut input, mut output) = (std::io::stdin().lock(), std::io::stdout().lock());
    let mut pipe = None;
    while let Ok(Some(msg)) = read_frame(&mut input) {
        let mut answer = None;
        for _ in 0..2 {
            if pipe.is_none() {
                pipe = connect().ok();
            }
            let Some(p) = pipe.as_mut() else { break };
            match write_frame(p, &msg).and_then(|_| read_frame(p)) {
                Ok(Some(reply)) => {
                    answer = Some(reply);
                    break;
                }
                // Sompio restarted since: connect again once
                _ => pipe = None,
            }
        }
        let reply = answer.unwrap_or_else(|| {
            let parsed: Value = serde_json::from_slice(&msg).unwrap_or(Value::Null);
            let mut r = if parsed.get("type").and_then(Value::as_str) == Some("status") {
                json!({ "type": "status", "running": false })
            } else {
                json!({ "type": "error", "code": "not_running", "error": "Sompio isn't running. Start it, then try again." })
            };
            if let Some(rid) = parsed.get("rid") {
                r["rid"] = rid.clone();
            }
            r.to_string().into_bytes()
        });
        if write_frame(&mut output, &reply).is_err() {
            return;
        }
    }
}

