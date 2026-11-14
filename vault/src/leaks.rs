//! "Has this password been in a data breach?" using Have I Been Pwned's range search
//! (k-anonymity): only the first 5 characters of each password's SHA-1 hash are sent, and the
//! service answers with every leaked hash starting with them. The match happens here, so the
//! password, and even its full hash, never leave the PC. The network part is in the shell.

use crate::store::{field, EntrySummary, Vault};
use keepass::db::fields;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

pub const RANGE_URL: &str = "https://api.pwnedpasswords.com/range/";

/// What to ask for: hash prefix -> (rest of the hash, entries using that password)
pub struct Plan {
    pub prefixes: BTreeMap<String, Vec<(String, Vec<EntrySummary>)>>,
}

pub fn plan(v: &Vault) -> Plan {
    let mut prefixes: BTreeMap<String, Vec<(String, Vec<EntrySummary>)>> = BTreeMap::new();
    for e in v.live_entries() {
        let pw = field(&e, fields::PASSWORD);
        if pw.is_empty() {
            continue;
        }
        let hex: String = Sha1::digest(pw.as_bytes()).iter().map(|b| format!("{b:02X}")).collect();
        let (pre, rest) = hex.split_at(5);
        let list = prefixes.entry(pre.to_string()).or_default();
        match list.iter_mut().find(|(r, _)| r == rest) {
            Some((_, entries)) => entries.push(v.summary_of(&e)),
            None => list.push((rest.to_string(), vec![v.summary_of(&e)])),
        }
    }
    Plan { prefixes }
}

/// Every saved password long enough to look for, with its entry, for the Trace check: Vault
/// asks Trace whether any of them appear in your own chats and files. Runs on this PC only.
pub fn passwords_to_find(v: &Vault) -> Vec<(String, EntrySummary)> {
    v.live_entries()
        .into_iter()
        .filter_map(|e| {
            let pw = field(&e, fields::PASSWORD);
            (pw.chars().count() >= 6).then(|| (pw.to_string(), v.summary_of(&e)))
        })
        .collect()
}

/// How often a hash suffix appears in a range answer ("SUFFIX:COUNT" lines; padding has count 0).
pub fn count_in(body: &str, suffix: &str) -> u64 {
    body.lines()
        .filter_map(|l| l.trim().split_once(':'))
        .find(|(h, _)| h.eq_ignore_ascii_case(suffix))
        .and_then(|(_, n)| n.trim().parse().ok())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Leak {
    pub entry: EntrySummary,
    /// How many times this password shows up in known breaches
    pub seen: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::EntryInput;

    #[test]
    fn hashes_locally_and_reads_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("v.kdbx"), &[1u8; 32]).unwrap();
        v.add_entry(&EntryInput { title: "A".into(), password: Some("password".into()), ..Default::default() }).unwrap();
        v.add_entry(&EntryInput { title: "B".into(), password: Some("password".into()), ..Default::default() }).unwrap();
        let p = plan(&v);
        // SHA-1("password") = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
        let list = &p.prefixes["5BAA6"];
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
        assert_eq!(list[0].1.len(), 2);
        let body = "0018A45C4D1DEF81644B54AB7F969B88D65:1\r\n1E4C9B93F3F0682250B6CF8331B7EE68FD8:10434004\r\n00D4F6E8FA6EECAD2A3AA415EEC418D38EC:0\r\n";
        assert_eq!(count_in(body, &list[0].0), 10434004);
        assert_eq!(count_in(body, "FFFF"), 0);
    }
}
