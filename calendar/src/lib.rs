//! Sompio Calendar: reminders and schedules, in the Desk Reminders file format, encrypted under
//! the Sompio Lock. Pure logic, no UI: the desktop shell shows the windows and plays the sounds.

pub mod engine;
pub mod format;
pub mod runtime;
pub mod store;

pub use engine::{alert_key, due, occurrences, Alert, AlertMode, Kind, Occurrence, Quiet, Source};
pub use format::{parse, Problem, RowInput, Schedule, HEADER};
pub use store::{CalendarStore, Loaded, ScheduleView, Settings, PALETTE};
pub use runtime::{Popup, Reminders, Shown};
