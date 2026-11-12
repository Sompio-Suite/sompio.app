//! The browser extension's side of the vault: which logins belong to the website you're on,
//! and the browsers you've paired. Paired browsers' keys are kept inside the encrypted vault
//! (and left out of exports), so they're unreadable while Sompio is locked.

use crate::store::{field, EntryInput, EntrySummary, Vault};
use anyhow::{anyhow, bail, Result};
use keepass::db::{fields, CustomDataItem, CustomDataValue};
use serde::{Deserialize, Serialize};

pub const CLIENTS_KEY: &str = "Sompio.BrowserClients";

/// A browser that may ask the vault for logins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserClient {
    /// Random id the extension made
    pub id: String,
    /// The shared key, base64 (32 bytes)
    pub key: String,
    /// "Chrome", "Edge", "Firefox"…
    pub browser: String,
    /// Unix ms
    pub added: i64,
    pub last_used: i64,
}

/// The host of a web address, lower case, without "www." and the port. None for non-web addresses.
pub fn host_of(address: &str) -> Option<String> {
    let a = address.trim();
    if a.is_empty() {
        return None;
    }
    let with_scheme = if a.contains("://") { a.to_string() } else { format!("https://{a}") };
    let (scheme, rest) = with_scheme.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "https" | "http") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = if authority.starts_with('[') {
        authority.split(']').next()?.to_string() + "]"
    } else {
        authority.split(':').next()?.to_string()
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || (!host.contains('.') && host != "localhost" && !host.starts_with('[')) {
        return None;
    }
    Some(host.strip_prefix("www.").map(str::to_string).unwrap_or(host))
}

/// The registrable part of a host: "accounts.google.com" → "google.com", "shop.bbc.co.uk" → "bbc.co.uk".
fn site_of(host: &str) -> &str {
    if host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return host;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let keep = if labels.len() >= 3 && labels[labels.len() - 2].len() <= 3 && labels[labels.len() - 1].len() == 2 { 3 } else { 2 };
    if labels.len() <= keep {
        return host;
    }
    let skip: usize = labels[..labels.len() - keep].iter().map(|l| l.len() + 1).sum();
    &host[skip..]
}

/// How well a saved address fits the page: 3 the same host, 2 the entry is for a parent of the
/// page's host (example.com on login.example.com), 1 the same site otherwise. None: a different site.
pub fn match_score(entry_url: &str, page_url: &str) -> Option<u8> {
    let (e, p) = (host_of(entry_url)?, host_of(page_url)?);
    if e == p {
        Some(3)
    } else if p.ends_with(&format!(".{e}")) {
        Some(2)
    } else if site_of(&e) == site_of(&p) && site_of(&p).contains('.') {
        Some(1)
    } else {
        None
    }
}

impl Vault {
    /// Logins saved for the page's website, best match first. The recycle bin is left out.
    pub fn logins_for(&self, page_url: &str) -> Vec<EntrySummary> {
        let mut found: Vec<(u8, EntrySummary)> = Vec::new();
        for e in self.db.iter_all_entries() {
            if self.in_bin(e.parent().id()) {
                continue;
            }
            if let Some(score) = match_score(field(&e, fields::URL), page_url) {
                found.push((score, self.summary_of(&e)));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase())));
        found.into_iter().map(|(_, s)| s).collect()
    }

    /// Whether an entry may be filled into this page: it must be saved for the same website.
    pub fn check_site(&self, id: &str, page_url: &str) -> Result<EntrySummary> {
        let s = self.entry(id)?.summary;
        if s.in_recycle_bin {
            bail!("That login is in the recycle bin.");
        }
        match_score(&s.url, page_url).ok_or_else(|| anyhow!("That login is saved for another website."))?;
        Ok(s)
    }

    /// Saves a login the browser offered: a new entry, or a new password for an existing one.
    pub fn save_from_browser(&mut self, id: Option<&str>, page_url: &str, title: &str, username: &str, password: &str) -> Result<String> {
        if password.is_empty() {
            bail!("There's no password to save.");
        }
        let host = host_of(page_url).ok_or_else(|| anyhow!("Only logins on websites can be saved."))?;
        if let Some(id) = id {
            let d = self.entry(id)?;
            match_score(&d.summary.url, page_url).ok_or_else(|| anyhow!("That login is saved for another website."))?;
            let input = EntryInput {
                group_id: Some(d.summary.group_id.clone()),
                title: d.summary.title.clone(),
                username: if username.is_empty() { d.summary.username.clone() } else { username.to_string() },
                password: Some(password.to_string()),
                url: d.summary.url.clone(),
                notes: d.notes.clone(),
                tags: d.summary.tags.clone(),
                expires: None,
                custom: d.custom.iter().map(|f| crate::store::FieldInput { key: f.key.clone(), value: None, protected: f.protected }).collect(),
                otp: None,
                autotype: None,
            };
            self.update_entry(id, &input)?;
            self.save()?;
            return Ok(id.to_string());
        }
        let origin = page_url.split(['?', '#']).next().unwrap_or(page_url);
        let origin = match origin.split_once("://") {
            Some((scheme, rest)) => format!("{scheme}://{}", rest.split('/').next().unwrap_or("")),
            None => format!("https://{host}"),
        };
        let input = EntryInput {
            title: if title.trim().is_empty() { host.clone() } else { title.trim().chars().take(120).collect() },
            username: username.to_string(),
            password: Some(password.to_string()),
            url: origin,
            ..Default::default()
        };
        let id = self.add_entry(&input)?;
        self.save()?;
        Ok(id)
    }

    pub fn browser_clients(&self) -> Vec<BrowserClient> {
        match self.db.meta.custom_data.get(CLIENTS_KEY).and_then(|i| i.value.as_ref()) {
            Some(CustomDataValue::String(s)) => serde_json::from_str(s).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn set_browser_clients(&mut self, clients: &[BrowserClient]) -> Result<()> {
        if clients.is_empty() {
            self.db.meta.custom_data.remove(CLIENTS_KEY);
        } else {
            self.db.meta.custom_data.insert(
                CLIENTS_KEY.into(),
                CustomDataItem { value: Some(CustomDataValue::String(serde_json::to_string(clients)?)), last_modification_time: None },
            );
        }
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_and_matches() {
        assert_eq!(host_of("https://www.Example.com:8443/login?x=1").as_deref(), Some("example.com"));
        assert_eq!(host_of("example.com").as_deref(), Some("example.com"));
        assert_eq!(host_of("ftp://example.com"), None);
        assert_eq!(host_of("just words"), None);
        assert_eq!(match_score("https://example.com", "https://www.example.com/login"), Some(3));
        assert_eq!(match_score("example.com", "https://login.example.com"), Some(2));
        assert_eq!(match_score("https://accounts.google.com", "https://mail.google.com"), Some(1));
        assert_eq!(match_score("https://bbc.co.uk", "https://shop.bbc.co.uk"), Some(2));
        // Look-alikes never match
        assert_eq!(match_score("https://example.com", "https://example.com.evil.net"), None);
        assert_eq!(match_score("https://example.com", "https://notexample.com"), None);
        assert_eq!(match_score("https://a.co.uk", "https://b.co.uk"), None);
        assert_eq!(match_score("", "https://example.com"), None);
    }

    #[test]
    fn logins_save_and_clients_stay_out_of_exports() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("v.kdbx"), &[7; 32]).unwrap();
        let id = v.add_entry(&EntryInput { title: "Shop".into(), username: "sam".into(), password: Some("old".into()), url: "https://shop.example".into(), ..Default::default() }).unwrap();
        v.add_entry(&EntryInput { title: "Other".into(), password: Some("x".into()), url: "https://other.example".into(), ..Default::default() }).unwrap();
        let found = v.logins_for("https://shop.example/account/login");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id);
        assert!(v.check_site(&id, "https://evil.example").is_err());
        assert!(v.check_site(&id, "https://shop.example/x").is_ok());

        v.save_from_browser(Some(&id), "https://shop.example/login", "", "", "new-pass").unwrap();
        assert_eq!(&*v.reveal(&id, "Password").unwrap(), "new-pass");
        assert!(v.save_from_browser(Some(&id), "https://evil.example", "", "", "x").is_err());
        let new = v.save_from_browser(None, "https://news.example/sign-in?next=/", "News", "me", "pw").unwrap();
        assert_eq!(v.entry(&new).unwrap().summary.url, "https://news.example");

        let c = BrowserClient { id: "abc".into(), key: "k".into(), browser: "Chrome".into(), added: 1, last_used: 1 };
        v.set_browser_clients(&[c.clone()]).unwrap();
        let again = Vault::open(&dir.path().join("v.kdbx"), &[7; 32]).unwrap();
        assert_eq!(again.browser_clients(), vec![c]);
        let out = dir.path().join("export.kdbx");
        again.export_kdbx(&out, "export-password", None).unwrap();
        let exported = keepass::Database::open(&mut std::fs::File::open(&out).unwrap(), keepass::DatabaseKey::new().with_password("export-password")).unwrap();
        assert!(!exported.meta.custom_data.contains_key(CLIENTS_KEY));
    }
}
