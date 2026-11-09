//! Samples for testing Vault: a folder of made-up entries that touches every feature, files for
//! every import (KeePass with a password, KeePass with a key file, browser and password-manager
//! CSVs), and a local login page to try auto-type on.

use crate::store::{AutoTypeSettings, EntryInput, FieldInput, Vault};
use anyhow::Result;
use keepass::db::fields;
use keepass::Database;
use std::path::Path;

pub const FOLDER: &str = "Samples";
pub const KDBX_PASSWORD: &str = "sample-password";
pub const TEST_PAGE_TITLE: &str = "Sompio test login";

fn e(title: &str, user: &str, pw: &str, url: &str) -> EntryInput {
    EntryInput { title: title.into(), username: user.into(), password: Some(pw.into()), url: url.into(), ..Default::default() }
}

/// Adds the "Samples" folder. Delete the folder to remove them all.
pub fn add_sample_entries(v: &mut Vault) -> Result<String> {
    let top = v.add_group(None, FOLDER)?;
    let games = v.add_group(Some(&top), "Games")?;
    let inside = |mut x: EntryInput, g: &str| {
        x.group_id = Some(g.to_string());
        x
    };
    let mut test = e(TEST_PAGE_TITLE, "tester@example.org", "Glacier-Lantern-Fjord-77", "");
    test.notes = "Open \"Test login page.html\" from the sample files, click into the username box and press the auto-type shortcut (Ctrl+Alt+A).".into();
    test.tags = vec!["try auto-type".into()];
    v.add_entry(&inside(test, &top))?;

    let mut notepad = e("Notepad (auto-type test)", "alex.rivers", "typed-by-sompio", "");
    notepad.notes = "Open Notepad, then press Ctrl+Alt+A: the window rule *Notepad* picks this entry.".into();
    notepad.autotype = Some(AutoTypeSettings { enabled: true, sequence: "Hello from Sompio{ENTER}User: {USERNAME}{ENTER}Password: {PASSWORD}{ENTER}".into(), windows: vec!["*Notepad*".into(), "*Kladblok*".into()] });
    v.add_entry(&inside(notepad, &top))?;

    let mut twofa = e("Proton Mail (2FA sample)", "alex.rivers@example.org", "quarry-lantern-fjord-pelican-amber", "https://account.proton.me");
    twofa.otp = Some("JBSWY3DPEHPK3PXP".into());
    twofa.notes = "The 2FA code changes every 30 seconds. Use it as {TOTP} in auto-type.".into();
    v.add_entry(&inside(twofa, &top))?;

    let mut bank = e("Bank (extra fields)", "alexrivers", "Plover!Mosaic-Ridge-9", "https://bank.example");
    bank.custom = vec![
        FieldInput { key: "PIN".into(), value: Some("4321".into()), protected: true },
        FieldInput { key: "Customer number".into(), value: Some("0012 3456".into()), protected: false },
    ];
    bank.tags = vec!["money".into()];
    v.add_entry(&inside(bank, &top))?;

    // Health check material: weak, reused, expired
    v.add_entry(&inside(e("Old forum (weak)", "alex94", "password1", "https://forum.example"), &top))?;
    v.add_entry(&inside(e("Shop (reused)", "alex.rivers@example.org", "BrightMarble77", "https://shop.example"), &top))?;
    v.add_entry(&inside(e("Streaming (reused)", "alex.rivers@example.org", "BrightMarble77", "https://stream.example"), &top))?;
    let mut expired = e("Work VPN (expired)", "arivers", "Spring-2024-Pass", "https://vpn.example");
    expired.expires = Some("2025-01-01".into());
    v.add_entry(&inside(expired, &top))?;

    // History: changed twice, so there are earlier versions to look at and bring back
    let id = v.add_entry(&inside(e("Battle.net (history)", "alex#2231", "first-password-2022", "https://battle.net"), &games))?;
    let mut later = e("Battle.net (history)", "alex#2231", "second-password-2024", "https://battle.net");
    later.group_id = Some(games.clone());
    v.update_entry(&id, &later)?;
    let mut now = later.clone();
    now.password = Some("Tundra-Velvet-Crane-42".into());
    now.autotype = Some(AutoTypeSettings { enabled: true, sequence: "{PASSWORD}{ENTER}".into(), windows: vec!["Battle.net*".into()] });
    v.update_entry(&id, &now)?;

    let mut doc = e("Passport (attached file)", "", "", "");
    doc.notes = "A file kept inside the entry, encrypted with the vault. Use Save to get it back out.".into();
    let doc_id = v.add_entry(&inside(doc, &top))?;
    v.add_attachment(&doc_id, "passport-details.txt", b"SAMPLE ONLY\r\nName: Alex Rivers\r\nDocument: XX1234567 (made up)\r\nValid until: 2031-05-01\r\n".to_vec())?;

    let mut wifi = e("Home Wi-Fi (a note)", "", "orbit-candle-meadow-tusk", "");
    wifi.notes = "Guest network: Sompio-Guest\nRouter admin is on the sticker.".into();
    v.add_entry(&inside(wifi, &top))?;
    Ok(top)
}

const TEST_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Sompio test login</title>
<style>body{font:16px system-ui;background:#0f1b2e;color:#dce8f2;display:grid;place-items:center;height:100vh;margin:0}
form{background:#132239;padding:2rem;border-radius:14px;display:grid;gap:.8rem;width:320px}
input{font:inherit;padding:.6rem;border-radius:8px;border:1px solid #2c4060;background:#0b1424;color:inherit}
button{font:inherit;padding:.6rem;border-radius:8px;border:0;background:#f2b35b;color:#0b1424;font-weight:700}
#out{min-height:1.5em;color:#7fd1a0}</style></head>
<body><form onsubmit="event.preventDefault();out.textContent='Signed in as '+user.value+' with a '+pass.value.length+'-character password. Auto-type works.'">
<h2 style="margin:0">Sompio test login</h2>
<p style="margin:0;opacity:.7">Click into Username, then press your auto-type shortcut (Ctrl+Alt+A). Nothing is sent anywhere: this page is on your PC.</p>
<input id="user" placeholder="Username" autocomplete="off"><input id="pass" type="password" placeholder="Password">
<button>Sign in</button><div id="out"></div></form></body></html>
"#;

/// Writes the import samples and the test page into `dir`.
pub fn write_sample_files(dir: &Path) -> Result<Vec<(String, &'static str)>> {
    std::fs::create_dir_all(dir)?;
    let mut files = Vec::new();

    // A KeePass file made with a password, and one that also needs a key file
    let mut db = Database::new();
    db.meta.database_name = Some("Sample KeePass".into());
    {
        let mut root = db.root_mut();
        let mut g = root.add_group();
        g.name = "Email".into();
        let mut x = g.add_entry();
        x.set_unprotected(fields::TITLE, "Sample webmail");
        x.set_unprotected(fields::USERNAME, "alex@example.org");
        x.set_protected(fields::PASSWORD, "from-keepass-123");
        x.set_unprotected(fields::URL, "https://mail.example");
        drop(x);
        let mut y = root.add_entry();
        y.set_unprotected(fields::TITLE, "Sample router");
        y.set_unprotected(fields::USERNAME, "admin");
        y.set_protected(fields::PASSWORD, "router-pass-456");
    }
    let mut buf = Vec::new();
    db.save(&mut buf, crate::store::user_key(KDBX_PASSWORD, None)?).map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::write(dir.join("Sample KeePass.kdbx"), &buf)?;
    files.push(("Sample KeePass.kdbx".to_string(), "A KeePass file. Its password is: sample-password"));

    let keyfile: [u8; 32] = rand::random();
    std::fs::write(dir.join("Sample key file.key"), keyfile)?;
    let mut buf = Vec::new();
    db.save(&mut buf, crate::store::user_key(KDBX_PASSWORD, Some(&keyfile))?).map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::write(dir.join("Sample KeePass with key file.kdbx"), &buf)?;
    files.push(("Sample KeePass with key file.kdbx".to_string(), "Needs the password sample-password and the key file \"Sample key file.key\"."));

    let csvs: [(&str, &str, &'static str); 5] = [
        (
            "Sample Bitwarden export.csv",
            "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\nSocial,,login,Sample Discord,,,0,https://discord.com,alex.rivers,bw-sample-1,JBSWY3DPEHPK3PXP\n,,login,Sample Steam,,,0,https://store.steampowered.com,alex_nl,bw-sample-2,\n,,card,Sample card (skipped),,,0,,,,\n",
            "A Bitwarden export with a folder and a 2FA secret. Cards are skipped.",
        ),
        ("Sample Chrome passwords.csv", "name,url,username,password,note\nexample.org,https://example.org/,alex,chrome-sample-1,\nshop.example,https://shop.example/login,alex@example.org,chrome-sample-2,saved in Chrome\n", "Passwords exported from Chrome, Edge or Brave."),
        (
            "Sample Firefox logins.csv",
            "\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\"https://news.example\",\"alexr\",\"firefox-sample-1\",,\"https://news.example\",\"{1}\",\"1700000000000\",\"1700000000000\",\"1700000000000\"\n",
            "Logins exported from Firefox.",
        ),
        ("Sample 1Password export.csv", "Title,Url,Username,Password,OTPAuth,Favorite,Archived,Tags,Notes\nSample GitHub,https://github.com,alexr,op-sample-1,,false,false,work,from 1Password\n", "A 1Password CSV export with a tag."),
        ("Sample KeePassXC export.csv", "\"Group\",\"Title\",\"Username\",\"Password\",\"URL\",\"Notes\",\"TOTP\",\"Icon\",\"Last Modified\",\"Created\"\n\"Root/Work\",\"Sample Jira\",\"arivers\",\"kpxc-sample-1\",\"https://jira.example\",\"\",\"\",\"0\",\"\",\"\"\n", "A KeePassXC CSV export with folders."),
    ];
    for (name, body, what) in csvs {
        std::fs::write(dir.join(name), body)?;
        files.push((name.to_string(), what));
    }

    std::fs::write(dir.join("Test login page.html"), TEST_PAGE)?;
    files.push(("Test login page.html".to_string(), "Open it in your browser to try auto-type. Add the sample entries first: \"Sompio test login\" is the one it types."));

    let mut readme = String::from("Sompio Vault samples\r\n====================\r\n\r\nMade-up passwords to try every part of Vault. Nothing in them is real.\r\nImport them under Vault, \"Auto-type, import and export\".\r\n\r\n");
    for (f, what) in &files {
        readme.push_str(&format!("{f}\r\n  {what}\r\n\r\n"));
    }
    readme.push_str("Delete the CSV files when you're done: real CSV exports hold passwords in plain text.\r\n");
    std::fs::write(dir.join("READ ME.txt"), readme)?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_cover_every_feature_and_import() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("v.kdbx"), &[9u8; 32]).unwrap();
        let top = add_sample_entries(&mut v).unwrap();
        let all = v.entries(Some(&top), "").unwrap();
        assert_eq!(all.len(), 11);
        assert!(all.iter().any(|x| !v.entry(&x.id).unwrap().attachments.is_empty()));
        assert!(all.iter().any(|e| e.has_totp) && all.iter().any(|e| e.expired));
        let h = crate::health::report(&v);
        assert!(h.weak >= 1 && h.reused >= 2 && h.expired >= 1);
        let bnet = all.iter().find(|e| e.title.starts_with("Battle.net")).unwrap();
        assert_eq!(v.entry(&bnet.id).unwrap().history.len(), 2);
        assert_eq!(crate::autotype::matches(&v, "Untitled - Notepad")[0].entry.title, "Notepad (auto-type test)");
        assert_eq!(crate::autotype::matches(&v, "Sompio test login - Google Chrome")[0].entry.title, TEST_PAGE_TITLE);

        let files = dir.path().join("files");
        write_sample_files(&files).unwrap();
        let kdbx = std::fs::read(files.join("Sample KeePass.kdbx")).unwrap();
        assert_eq!(v.import_kdbx(&kdbx, KDBX_PASSWORD, None, "a").unwrap().entries, 2);
        let kdbx2 = std::fs::read(files.join("Sample KeePass with key file.kdbx")).unwrap();
        let key = std::fs::read(files.join("Sample key file.key")).unwrap();
        assert!(v.import_kdbx(&kdbx2, KDBX_PASSWORD, None, "b").is_err(), "needs the key file");
        assert_eq!(v.import_kdbx(&kdbx2, KDBX_PASSWORD, Some(&key), "b").unwrap().entries, 2);
        for (name, n) in [("Sample Bitwarden export.csv", 2), ("Sample Chrome passwords.csv", 2), ("Sample Firefox logins.csv", 1), ("Sample 1Password export.csv", 1), ("Sample KeePassXC export.csv", 1)] {
            let text = std::fs::read_to_string(files.join(name)).unwrap();
            assert_eq!(crate::csvfile::import(&mut v, &text, name).unwrap().entries, n, "{name}");
        }
    }
}
