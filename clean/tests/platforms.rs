//! Reddit, Telegram, X and the PC scan: detection, own-messages-only, links, and living side by side.

mod common;
use common::*;
use sompio_clean::{import_package, run_scan, scan_folders, ScanOptions, StatusFilter, Store};

fn texts(store: &Store, platform: &str) -> Vec<String> {
    let mut out = vec![];
    for c in store.channels(None).unwrap().into_iter().filter(|c| c.platform == platform) {
        out.extend(store.messages(&c.id, 0, 1000).unwrap().items.into_iter().map(|m| store.reveal_message(&m.id).unwrap().unwrap().content));
    }
    out
}

#[test]
fn reddit_posts_comments_and_only_your_messages() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("reddit.zip");
    reddit_export(&p);
    let mut store = Store::open_in_memory().unwrap();
    let s = import_package(&mut store, &p, &mut |_| {}).unwrap();
    assert_eq!(s.platform, "reddit");
    assert_eq!(s.messages, 4, "1 post, 2 comments, 1 sent message (the received one is skipped)");
    let all = texts(&store, "reddit");
    assert!(all.iter().any(|t| t.contains("My setup")));
    assert!(!all.iter().any(|t| t.contains("06-98765432")), "someone else's message must not be imported");

    let names: Vec<String> = store.channels(None).unwrap().into_iter().map(|c| c.name).collect();
    assert!(names.contains(&"r/homelab".to_string()) && names.contains(&"r/Amsterdam".to_string()));

    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    assert_eq!((count(&scan, "ip_address"), count(&scan, "address"), count(&scan, "iban")), (1, 1, 1));
    let addr = store.findings(Some("address"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    assert_eq!(addr.platform, "reddit");
    assert_eq!(addr.link.as_deref(), Some("https://www.reddit.com/r/Amsterdam/comments/x/_/c1/"));
    let ip = store.findings(Some("ip_address"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    assert_eq!(ip.link.as_deref(), Some("https://www.reddit.com/r/homelab/comments/p1/my_setup/"));
    assert!(store.stats().unwrap().first_ts.unwrap() < store.stats().unwrap().last_ts.unwrap());
}

#[test]
fn telegram_only_your_messages() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tg.zip");
    telegram_export(&p);
    let mut store = Store::open_in_memory().unwrap();
    let s = import_package(&mut store, &p, &mut |_| {}).unwrap();
    assert_eq!((s.platform.as_str(), s.messages), ("telegram", 2));
    let all = texts(&store, "telegram");
    assert!(all.iter().any(|t| t == "call me on +31 6 12345678 tonight"), "formatted text is flattened: {all:?}");
    assert!(!all.iter().any(|t| t.contains("mara.private")));

    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    assert_eq!((count(&scan, "phone"), count(&scan, "credential"), count(&scan, "email")), (1, 1, 0));
    let cred = store.findings(Some("credential"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    assert_eq!(cred.link.as_deref(), Some("https://t.me/c/1500100200/55"));
    let phone = store.findings(Some("phone"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    assert_eq!(phone.link, None, "private chats have no web link");
}

#[test]
fn telegram_single_chat_export_explains_what_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("chat.zip");
    write_zip(&p, &[("ChatExport/result.json", r#"{"name": "Mara", "type": "personal_chat", "id": 1, "messages": []}"#.into())]);
    let mut store = Store::open_in_memory().unwrap();
    let err = import_package(&mut store, &p, &mut |_| {}).unwrap_err().to_string();
    assert!(err.contains("single chat") && err.contains("Export Telegram data"), "{err}");
}

#[test]
fn x_posts_and_sent_dms_without_reposts() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.zip");
    x_archive(&p);
    let mut store = Store::open_in_memory().unwrap();
    let s = import_package(&mut store, &p, &mut |_| {}).unwrap();
    assert_eq!((s.platform.as_str(), s.messages), ("x", 2));
    let all = texts(&store, "x");
    assert!(!all.iter().any(|t| t.starts_with("RT @") || t.contains("Damrak")));

    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    assert_eq!((count(&scan, "email"), count(&scan, "address"), count(&scan, "phone")), (1, 1, 0));
    let email = store.findings(Some("email"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    assert_eq!(email.link.as_deref(), Some("https://x.com/samtest/status/1500000000000000001"));
    assert_eq!(email.ts, 1_614_834_367_000);
}

#[test]
fn pc_scan_reads_files_and_flags_names() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("Documents");
    std::fs::create_dir_all(docs.join("taxes/2023")).unwrap();
    std::fs::create_dir_all(docs.join(".git")).unwrap();
    std::fs::create_dir_all(docs.join("node_modules/x")).unwrap();
    std::fs::write(docs.join("notes.txt"), "wifi password: SuperSecret99\nbank NL91 ABNA 0417 1643 00").unwrap();
    std::fs::write(docs.join("taxes/2023/Passport_scan.jpg"), [0xFFu8, 0xD8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    std::fs::write(docs.join("id_rsa"), "-----BEGIN OPENSSH PRIVATE KEY-----").unwrap();
    std::fs::write(docs.join("holiday.txt"), "nothing to see, call 06 is not a number").unwrap();
    std::fs::write(docs.join(".git/config"), "password: gitsecret").unwrap();
    std::fs::write(docs.join("node_modules/x/a.txt"), "password: skipme").unwrap();
    std::fs::write(docs.join("passport-renewal-letter.txt"), "Dear customer").unwrap();
    // A Word document
    let docx = docs.join("cv.docx");
    write_zip(&docx, &[("word/document.xml", "<w:document><w:body><w:p><w:r><w:t>Sam Rivers</w:t></w:r></w:p><w:p><w:r><w:t>sam.rivers@example.org &amp; 06-12345678</w:t></w:r></w:p></w:body></w:document>".into())]);

    let mut store = Store::open_in_memory().unwrap();
    let s = scan_folders(&mut store, &[docs.clone()], &mut |_| {}).unwrap();
    assert_eq!(s.messages, 6, "hidden folders and node_modules are skipped");
    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    assert_eq!(count(&scan, "iban"), 1);
    assert_eq!(count(&scan, "email"), 1, "text inside the Word document is read");
    assert_eq!(count(&scan, "credential"), 2, "the password line, and the id_rsa key file");
    assert_eq!(count(&scan, "sensitive_file"), 3, "Passport_scan.jpg, passport-renewal-letter.txt and cv.docx by name");

    let passport = store
        .findings(Some("sensitive_file"), StatusFilter::All, 0, 10, true)
        .unwrap()
        .items
        .into_iter()
        .find(|f| f.matched == "Passport_scan.jpg" && f.before.contains("taxes"))
        .unwrap();
    assert_eq!(passport.platform, "pc");
    assert!(passport.file_path.as_deref().unwrap().ends_with("Passport_scan.jpg"));
    assert_eq!(passport.link, None);
    assert_eq!(store.file_path_for(passport.id).unwrap(), passport.file_path);

    // "Never flag this again" on one file doesn't hide other files with the same word in their name
    let letter = store
        .findings(Some("sensitive_file"), StatusFilter::All, 0, 10, true)
        .unwrap()
        .items
        .into_iter()
        .find(|f| f.matched.starts_with("passport-renewal"))
        .unwrap();
    assert_eq!(store.ignore_everywhere(letter.id).unwrap(), 1);

    // Moving it to the Recycle Bin marks it deleted. (The trash may not exist in CI, so
    // only check the error is reported plainly if it fails.)
    match store.recycle_file(passport.id) {
        Ok(path) => {
            assert!(!std::path::Path::new(&path).exists());
            assert!(store.scan_summary().unwrap().done >= 1);
        }
        Err(e) => assert!(e.to_string().contains("Recycle Bin"), "{e}"),
    }

    // Stats keep files apart from messages
    let st = store.stats().unwrap();
    assert_eq!((st.messages, st.files), (0, 6));
}

#[test]
fn platforms_live_side_by_side_and_replace_only_themselves() {
    let dir = tempfile::tempdir().unwrap();
    let (d, r, t) = (dir.path().join("d.zip"), dir.path().join("r.zip"), dir.path().join("t.zip"));
    discord_package(&d);
    reddit_export(&r);
    telegram_export(&t);
    let mut store = Store::open_in_memory().unwrap();
    for p in [&d, &r, &t] {
        import_package(&mut store, p, &mut |_| {}).unwrap();
    }
    assert_eq!(store.sources().unwrap().len(), 3);
    assert_eq!(store.stats().unwrap().messages, 3 + 4 + 2);

    // Re-importing Reddit leaves Discord and Telegram alone
    import_package(&mut store, &r, &mut |_| {}).unwrap();
    assert_eq!(store.stats().unwrap().messages, 9);

    // One review list across all sources
    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    let platforms: std::collections::HashSet<String> = store
        .findings(None, StatusFilter::Open, 0, 100, false)
        .unwrap()
        .items
        .into_iter()
        .map(|f| f.platform)
        .collect();
    assert_eq!(platforms.len(), 3, "{platforms:?}");
    assert!(scan.total_findings >= 7);

    // Removing a source removes only it
    store.remove_source("telegram").unwrap();
    assert_eq!(store.sources().unwrap().len(), 2);
    assert_eq!(store.stats().unwrap().messages, 7);
    assert_eq!(store.search("Hunter2forreal", None, 0, 10).unwrap().total, 0, "search index updated");
}
