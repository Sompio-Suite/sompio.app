//! Sompio Vault: passwords in the KeePass format (KDBX 4), opened with the Sompio Lock.
//! No network. Auto-type keystrokes are sent by the desktop shell (see `typing`).

pub mod autotype;
pub mod browser;
pub mod csvfile;
pub mod generator;
pub mod health;
pub mod icons;
pub mod leaks;
pub mod samples;
pub mod store;

pub use store::{AutoTypeSettings, EntryDetail, EntryInput, EntrySummary, FieldInput, GroupView, ImportSummary, Totp, Vault};
