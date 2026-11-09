//! The password health check: weak, reused, expired and old passwords. Runs entirely offline.

use crate::generator::strength;
use crate::store::{field, EntrySummary, Vault};
use chrono::Utc;
use keepass::db::fields;
use serde::Serialize;
use std::collections::HashMap;

/// A password unchanged for this long is called old.
pub const OLD_DAYS: i64 = 365;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    Weak { score: u8, label: String, hint: Option<String> },
    Reused { count: usize, others: Vec<String> },
    Expired,
    Old { days: i64 },
    NoPassword,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthItem {
    pub entry: EntrySummary,
    pub issues: Vec<Issue>,
    /// Higher is worse, for sorting
    pub severity: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub checked: usize,
    pub weak: usize,
    pub reused: usize,
    pub expired: usize,
    pub old: usize,
    /// 0 to 100: the share of passwords with nothing wrong
    pub score: u8,
    pub items: Vec<HealthItem>,
}

pub fn report(v: &Vault) -> HealthReport {
    let now = Utc::now().naive_utc();
    let entries: Vec<_> = v.live_entries().collect();
    let mut by_password: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let pw = field(e, fields::PASSWORD);
        if !pw.is_empty() {
            by_password.entry(pw).or_default().push(i);
        }
    }
    // Strength depends only on the password, so reused ones are scored once.
    let mut scored: HashMap<&str, crate::generator::Strength> = HashMap::new();
    let (mut weak, mut reused, mut expired, mut old, mut checked) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut items = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let pw = field(e, fields::PASSWORD);
        let summary = v.summary_of(e);
        let mut issues = Vec::new();
        let mut severity = 0u8;
        if pw.is_empty() {
            if summary.username.is_empty() && summary.url.is_empty() {
                continue; // a note, not a login
            }
            issues.push(Issue::NoPassword);
            severity = severity.max(1);
        } else {
            checked += 1;
            let s = scored.entry(pw).or_insert_with(|| strength(pw));
            if s.score <= 2 {
                weak += 1;
                severity = severity.max(4 - s.score.min(2));
                issues.push(Issue::Weak { score: s.score, label: s.label.to_string(), hint: s.warning.clone() });
            }
            let same = &by_password[pw];
            if same.len() > 1 {
                reused += 1;
                severity = severity.max(3);
                let others = same.iter().filter(|j| **j != i).map(|j| field(&entries[*j], fields::TITLE).to_string()).collect();
                issues.push(Issue::Reused { count: same.len(), others });
            }
            if let Some(changed) = v.password_modified(e) {
                let days = (now - changed).num_days();
                if days >= OLD_DAYS {
                    old += 1;
                    severity = severity.max(1);
                    issues.push(Issue::Old { days });
                }
            }
        }
        if summary.expired {
            expired += 1;
            severity = severity.max(2);
            issues.push(Issue::Expired);
        }
        if !issues.is_empty() {
            items.push(HealthItem { entry: summary, issues, severity });
        }
    }
    items.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.entry.title.to_lowercase().cmp(&b.entry.title.to_lowercase())));
    let bad = items.iter().filter(|i| i.severity >= 2).count();
    let score = if checked == 0 { 100 } else { (100 * checked.saturating_sub(bad) / checked) as u8 };
    HealthReport { checked, weak, reused, expired, old, score, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::EntryInput;

    #[test]
    fn finds_weak_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("v.kdbx"), &[2u8; 32]).unwrap();
        let add = |v: &mut Vault, t: &str, pw: &str| {
            v.add_entry(&EntryInput { title: t.into(), username: "me".into(), password: Some(pw.into()), ..Default::default() }).unwrap()
        };
        add(&mut v, "Forum", "password1");
        add(&mut v, "Shop", "tundra-velvet-crane-oxygen-mallet");
        add(&mut v, "Bank", "tundra-velvet-crane-oxygen-mallet");
        add(&mut v, "Mail", "quarry-lantern-fjord-pelican-amber");
        let r = report(&v);
        assert_eq!((r.checked, r.weak, r.reused), (4, 1, 2));
        assert_eq!(r.items[0].entry.title, "Forum");
        assert_eq!(r.score, 25);
    }
}
