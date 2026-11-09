//! Sompio core: everything that touches your data lives here.
//!
//! Design rules (keep these when you change things):
//! - No networking. This crate has no HTTP client dependency, and must never get one.
//! - Your data stays in one local SQLite file that you can wipe at any time.
//! - Only what you wrote is imported; other people's messages are skipped.
//! - Sensitive matches are stored as positions, never copied into a second place.
//! - Anything shown on screen is masked unless the user explicitly reveals it.

mod db;
mod demo;
mod import;
mod pcscan;
mod query;
mod scan;
pub mod samples;
mod time;

pub use db::Store;
pub use demo::load_demo;
pub use import::{gmail_link, import_live, parse_sent_mail, SentMail, import_package, platform_label, ImportProgress, ImportSummary, LiveChannel, LiveMessage};
pub use pcscan::scan_folders;
pub use query::{ChannelView, MessageView, MessagesPage, SearchPage, Segment, SourceView, Stats, YearCount};
pub use scan::{FindingsFilter, GroupInfo, GroupPlatform, 
    run_scan, Detection, FindingView, FindingsPage, Kind, KindCount, ScanOptions, ScanSummary, Scanner, StatusFilter,
};

/// Discord IDs ("snowflakes") contain their own creation time.
/// Returns Unix milliseconds.
pub fn snowflake_to_ms(id: i64) -> i64 {
    (id >> 22) + 1_420_070_400_000
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
