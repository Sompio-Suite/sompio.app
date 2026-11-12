//! Trace's request centre: which data exports you've asked for, and the privacy requests you've
//! sent to companies (delete my data, send me a copy, stop using it), with their deadlines.
//! Stored as one small record in the encrypted database, next to everything else.

use crate::db::Store;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const KEY: &str = "trace_requests";
/// A sanity limit: the record is a few kilobytes even with hundreds of requests.
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceData {
    /// What goes into letters, as you choose: your name, the email you use there, your country
    pub profile: Profile,
    /// Data exports you asked platforms for, by platform key ("discord", "reddit", …)
    pub exports: BTreeMap<String, ExportState>,
    pub requests: Vec<PrivacyRequest>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub email: String,
    pub country: String,
    /// Your usernames on platforms ("discord", "reddit", "x"), for the ready-made searches
    pub handles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportState {
    /// YYYY-MM-DD
    pub requested_on: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyRequest {
    pub id: String,
    /// The company's key in the bundled directory, or "" for one typed in by hand
    pub company: String,
    pub company_name: String,
    /// "erasure" | "access" | "objection"
    pub kind: String,
    /// The account it's about (email or username), as written in the letter
    pub account: String,
    /// YYYY-MM-DD; the one-month deadline counts from here
    pub sent_on: Option<String>,
    /// "waiting" | "answered" | "done"
    pub status: String,
    /// Reminders sent (YYYY-MM-DD each)
    pub reminders: Vec<String>,
    /// When a complaint went to a data protection authority, and which one
    pub complained_on: Option<String>,
    pub authority: String,
    pub note: String,
}

/// "2026-11-12": four digits, two, two, with a real month and day
fn is_date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return false;
    }
    let n: Vec<u32> = p.iter().filter_map(|x| x.parse().ok()).collect();
    n.len() == 3 && (1..=12).contains(&n[1]) && (1..=31).contains(&n[2])
}

impl Store {
    pub fn trace_data(&self) -> Result<TraceData> {
        Ok(match self.get_meta(KEY)? {
            Some(s) => serde_json::from_str(&s).unwrap_or_default(),
            None => TraceData::default(),
        })
    }

    pub fn set_trace_data(&self, data: &TraceData) -> Result<()> {
        for r in &data.requests {
            if !matches!(r.kind.as_str(), "erasure" | "access" | "objection") {
                bail!("Unknown kind of request: {}", r.kind);
            }
            if !matches!(r.status.as_str(), "waiting" | "answered" | "done") {
                bail!("Unknown request status: {}", r.status);
            }
            for d in r.sent_on.iter().chain(r.complained_on.iter()).chain(r.reminders.iter()) {
                if !is_date(d) {
                    bail!("Not a date: {d}");
                }
            }
        }
        let s = serde_json::to_string(data)?;
        if s.len() > MAX_BYTES {
            bail!("Too much to store.");
        }
        self.set_meta(KEY, &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_and_are_checked() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.trace_data().unwrap(), TraceData::default());
        let mut d = TraceData::default();
        d.profile.name = "Sam Rivers".into();
        d.profile.handles.insert("reddit".into(), "samr".into());
        d.exports.insert("discord".into(), ExportState { requested_on: Some("2026-11-12".into()) });
        d.requests.push(PrivacyRequest {
            id: "r1".into(),
            company: "discord".into(),
            company_name: "Discord Netherlands BV".into(),
            kind: "erasure".into(),
            account: "sam@example.org".into(),
            sent_on: Some("2026-11-12".into()),
            status: "waiting".into(),
            ..Default::default()
        });
        s.set_trace_data(&d).unwrap();
        assert_eq!(s.trace_data().unwrap(), d);

        let mut bad = d.clone();
        bad.requests[0].kind = "nonsense".into();
        assert!(s.set_trace_data(&bad).is_err());
        let mut bad = d.clone();
        bad.requests[0].sent_on = Some("12/11/2026".into());
        assert!(s.set_trace_data(&bad).is_err());
        assert_eq!(s.trace_data().unwrap(), d, "a refused save changes nothing");
    }
}
