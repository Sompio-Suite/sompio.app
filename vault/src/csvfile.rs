//! CSV in and out. Reads exports from Bitwarden, Chrome/Edge/Brave, Firefox, 1Password, LastPass,
//! Proton Pass, Dashlane and KeePassXC by their column names. Writes KeePassXC's CSV layout.

use crate::store::{field, EntryInput, FieldInput, ImportSummary, Vault, OTP};
use anyhow::{bail, Result};
use keepass::db::fields;

/// The first column found from each list wins.
const TITLE: &[&str] = &["title", "name", "account", "item name"];
const USER: &[&str] = &["username", "login_username", "user name", "login", "email", "login name", "user"];
const PASS: &[&str] = &["password", "login_password", "pass"];
const URL: &[&str] = &["url", "login_uri", "website", "web site", "uri", "origin_url", "urls"];
const NOTES: &[&str] = &["notes", "note", "extra", "comments", "comment"];
const TOTP: &[&str] = &["totp", "login_totp", "otpauth", "otp", "one-time password", "2fa"];
const GROUP: &[&str] = &["group", "folder", "grouping", "vault", "category"];
const TAGS: &[&str] = &["tags", "tag"];

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| header.iter().position(|h| h == n))
}

pub fn import(v: &mut Vault, text: &str, label: &str) -> Result<ImportSummary> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let delim = if first.matches(';').count() > first.matches(',').count() { b';' } else { b',' };
    let mut r = csv::ReaderBuilder::new().delimiter(delim).flexible(true).from_reader(text.as_bytes());
    let header: Vec<String> = r.headers()?.iter().map(|h| h.trim().to_lowercase()).collect();
    let (title, user, pass, url) = (find(&header, TITLE), find(&header, USER), find(&header, PASS), find(&header, URL));
    if pass.is_none() || (title.is_none() && url.is_none()) {
        bail!("This CSV doesn't have the columns of a password export (it needs at least a password and a name or website column).");
    }
    let (notes, totp, group, tags) = (find(&header, NOTES), find(&header, TOTP), find(&header, GROUP), find(&header, TAGS));
    let used: Vec<usize> = [title, user, pass, url, notes, totp, group, tags].iter().flatten().copied().collect();
    // Bitwarden's "type" column: skip cards and identities, keep logins and notes
    let kind = header.iter().position(|h| h == "type");
    let mut rows = Vec::new();
    for rec in r.records() {
        let rec = rec?;
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string();
        if let Some(k) = kind {
            let t = rec.get(k).unwrap_or("").to_lowercase();
            if !t.is_empty() && t != "login" && t != "note" {
                continue;
            }
        }
        let password = get(pass);
        let (t, u) = (get(title), get(user));
        if t.is_empty() && u.is_empty() && password.is_empty() && get(url).is_empty() {
            continue;
        }
        // KeePassXC writes "Root/Social/Games"; drop its root folder name
        let mut path: Vec<String> = get(group).split(['/', '\\']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if header.first().is_some_and(|h| h == "group") && path.first().is_some_and(|p| p == "Root" || p == "Vault") {
            path.remove(0);
        }
        // Columns we don't know become extra fields, so nothing is lost
        let custom: Vec<FieldInput> = header
            .iter()
            .enumerate()
            .filter(|(i, h)| !used.contains(i) && !["type", "favorite", "fav", "reprompt", "icon", "last modified", "created", "guid", "timecreated", "timelastused", "timepasswordchanged", "httprealm", "formactionorigin", "archived", "fields"].contains(&h.as_str()))
            .filter_map(|(i, h)| rec.get(i).map(|v| (h, v.trim())))
            .filter(|(_, v)| !v.is_empty())
            .map(|(h, v)| FieldInput { key: h.to_string(), value: Some(v.to_string()), protected: false })
            .collect();
        let otp = get(totp);
        rows.push((
            path,
            EntryInput {
                title: t,
                username: u,
                password: Some(password),
                url: get(url),
                notes: get(notes),
                tags: get(tags).split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                otp: (!otp.is_empty()).then_some(otp),
                custom,
                ..Default::default()
            },
        ));
    }
    if rows.is_empty() {
        bail!("No passwords found in this file.");
    }
    v.add_imported(label, rows)
}

/// KeePassXC's CSV layout. Passwords are in plain text in this file.
pub fn export(v: &Vault) -> Result<String> {
    let mut w = csv::WriterBuilder::new().quote_style(csv::QuoteStyle::Always).from_writer(Vec::new());
    w.write_record(["Group", "Title", "Username", "Password", "URL", "Notes", "TOTP", "Icon", "Last Modified", "Created"])?;
    for e in v.live_entries() {
        let s = v.summary_of(&e);
        let group = if s.group_path.is_empty() { "Root".to_string() } else { format!("Root/{}", s.group_path.replace(" / ", "/")) };
        let fmt = |t: Option<chrono::NaiveDateTime>| t.map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string()).unwrap_or_default();
        w.write_record([
            group.as_str(),
            field(&e, fields::TITLE),
            field(&e, fields::USERNAME),
            field(&e, fields::PASSWORD),
            field(&e, fields::URL),
            field(&e, fields::NOTES),
            field(&e, OTP),
            "0",
            &fmt(e.times.last_modification),
            &fmt(e.times.creation),
        ])?;
    }
    Ok(String::from_utf8(w.into_inner()?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bitwarden_chrome_and_keepassxc() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("v.kdbx"), &[3u8; 32]).unwrap();
        let bw = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
                  Social,,login,Discord,,,0,https://discord.com,sam,pw-one,JBSWY3DPEHPK3PXP\n\
                  ,,card,Visa,,,0,,,,\n";
        let s = import(&mut v, bw, "Bitwarden").unwrap();
        assert_eq!((s.entries, s.groups), (1, 1));
        let e = &v.entries(None, "discord").unwrap()[0];
        assert!(e.has_totp);
        assert_eq!(e.group_path, "Bitwarden / Social");
        let chrome = "name,url,username,password,note\nexample.org,https://example.org/,me,pw-two,hello\n";
        assert_eq!(import(&mut v, chrome, "Chrome").unwrap().entries, 1);
        let out = export(&v).unwrap();
        assert!(out.contains("\"Root/Bitwarden/Social\",\"Discord\",\"sam\",\"pw-one\""));
        let mut v2 = Vault::open(&dir.path().join("v2.kdbx"), &[3u8; 32]).unwrap();
        assert_eq!(import(&mut v2, &out, "Back").unwrap().entries, 2);
        assert_eq!(v2.entries(None, "discord").unwrap()[0].group_path, "Back / Bitwarden / Social");
        assert!(import(&mut v2, "a,b\n1,2\n", "x").is_err());
    }
}
