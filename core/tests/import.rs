//! Builds fake data packages in both the newer (JSON, zip) and older (CSV, folder)
//! layouts and checks the whole pipeline: import, stats, search, scan, masking, export.

use std::io::Write;
use std::path::Path;
use veil_core::{import_package, run_scan, ScanOptions, Store};

// Snowflakes from 2021 and 2023.
const M1: &str = "800000000000000000";
const M2: &str = "800000000000000001";
const M3: &str = "1100000000000000000";

fn write_zip(path: &Path, files: &[(&str, String)]) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        z.start_file(*name, opts).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}

fn new_format_package(path: &Path) {
    write_zip(
        path,
        &[
            ("Account/user.json", "{}".into()),
            ("Activity/analytics/events-2023.json", "huge file we skip".into()),
            (
                "Messages/index.json",
                r#"{"111": "general in Test Server", "222": "Direct Message with friend"}"#.into(),
            ),
            (
                "Messages/c111/channel.json",
                r#"{"id": "111", "type": "GUILD_TEXT", "name": "general", "guild": {"id": "999", "name": "Test Server"}}"#.into(),
            ),
            (
                "Messages/c111/messages.json",
                format!(
                    r#"[{{"ID": {M1}, "Timestamp": "2021-03-01 10:00:00", "Contents": "hello world, my email is someone@example.org", "Attachments": ""}},
                        {{"ID": "{M2}", "Timestamp": "2021-03-01 10:00:01", "Contents": "cat pic", "Attachments": "https://cdn.discordapp.com/a.png"}}]"#
                ),
            ),
            (
                "Messages/c222/channel.json",
                r#"{"id": "222", "type": "DM", "recipients": ["1", "2"]}"#.into(),
            ),
            (
                "Messages/c222/messages.json",
                format!(
                    r#"[{{"ID": "{M3}", "Timestamp": "2023-01-01 00:00:00", "Contents": "my card is 4111 1111 1111 1111 and I live at Prinsengracht 263", "Attachments": ""}}]"#
                ),
            ),
        ],
    );
}

#[test]
fn imports_new_zip_format_and_scans() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("package.zip");
    new_format_package(&zip_path);

    let mut store = Store::open(&dir.path().join("veil.db")).unwrap();
    let mut phases = vec![];
    let summary = import_package(&mut store, &zip_path, &mut |p| phases.push(p.phase)).unwrap();
    assert_eq!(summary.channels, 2);
    assert_eq!(summary.messages, 3);
    assert_eq!(summary.servers, 1);
    assert!(phases.contains(&"Done"));

    let stats = store.stats().unwrap();
    assert_eq!(stats.messages, 3);
    assert_eq!(stats.dms, 1);
    assert_eq!(stats.with_attachments, 1);
    assert_eq!(stats.per_year.iter().map(|y| y.year).collect::<Vec<_>>(), vec![2021, 2023]);

    let channels = store.channels(None).unwrap();
    assert_eq!(channels[0].name, "general in Test Server");
    assert_eq!(channels[0].message_count, 2);
    assert_eq!(store.channels(Some("friend")).unwrap()[0].kind, "dm");

    // IDs survive as exact strings (JS can't hold them as numbers)
    let page = store.messages("111", 0, 50).unwrap();
    assert_eq!(page.items[0].id, M1);
    assert_eq!(page.items[1].attachments.len(), 1);
    assert_eq!(store.message_index("111", M2).unwrap(), 1);

    // Search, including prefix match and symbols that would break raw FTS syntax
    assert_eq!(store.search("hell", None, 0, 10).unwrap().total, 1);
    assert_eq!(store.search("\"cat\" AND (", None, 0, 10).unwrap().total, 0);
    assert_eq!(store.search("cat", Some("222"), 0, 10).unwrap().total, 0);

    // Scan
    let s = run_scan(&mut store, &ScanOptions { watch_terms: vec!["prinsengracht".into()] }, &mut |_, _| {}).unwrap();
    assert_eq!(s.scanned, 3);
    assert_eq!(s.flagged_messages, 2);
    let count = |k: &str| s.by_kind.iter().find(|c| c.kind.key() == k).unwrap().count;
    assert_eq!(count("email"), 1);
    assert_eq!(count("card"), 1);
    assert_eq!(count("address") + count("watch_term"), 1, "address and watch term overlap; one is kept");

    // Masked by default: no raw card number or email reaches the UI
    let masked = store.findings(None, 0, 100, false).unwrap();
    for f in &masked.items {
        let shown = format!("{}{}{}", f.before, f.matched, f.after);
        assert!(!shown.contains("4111 1111"), "card leaked: {shown}");
        assert!(!shown.contains("someone@example"), "email leaked: {shown}");
    }
    let revealed = store.findings(Some("card"), 0, 10, true).unwrap();
    assert_eq!(revealed.items[0].matched, "4111 1111 1111 1111");

    // Export holds IDs only
    let out = dir.path().join("flagged.csv");
    assert_eq!(store.export_flagged_csv(&out).unwrap(), 2);
    let csv = std::fs::read_to_string(&out).unwrap();
    assert!(csv.contains(M3) && !csv.contains("4111"));

    // Wipe leaves nothing behind
    store.wipe().unwrap();
    assert!(!store.has_data().unwrap());
    assert_eq!(store.search("hello", None, 0, 10).unwrap().total, 0);
    assert_eq!(store.scan_summary().unwrap().total_findings, 0);
}

#[test]
fn imports_old_csv_folder_format() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("package");
    let ch = root.join("messages").join("333");
    std::fs::create_dir_all(&ch).unwrap();
    std::fs::write(root.join("messages/index.json"), r#"{"333": "random in Old Server"}"#).unwrap();
    std::fs::write(ch.join("channel.json"), r#"{"id": "333", "type": 0, "guild": {"id": "5", "name": "Old Server"}}"#).unwrap();
    std::fs::write(
        ch.join("messages.csv"),
        format!("ID,Timestamp,Contents,Attachments\n{M1},2021-03-01 10:00:00.000000+00:00,\"multi\nline, with comma\",\n{M2},2021-03-01,bel 06-12345678,\n"),
    )
    .unwrap();

    let mut store = Store::open_in_memory().unwrap();
    let s = import_package(&mut store, &root, &mut |_| {}).unwrap();
    assert_eq!((s.channels, s.messages), (1, 2));
    assert_eq!(store.channels(None).unwrap()[0].kind, "server");
    assert_eq!(store.messages("333", 0, 10).unwrap().items[0].content, "multi\nline, with comma");
    let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    assert_eq!(scan.flagged_messages, 1);
}

#[test]
fn rejects_files_that_are_not_packages() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.zip");
    write_zip(&p, &[("readme.txt", "hi".into())]);
    let mut store = Store::open_in_memory().unwrap();
    let err = import_package(&mut store, &p, &mut |_| {}).unwrap_err();
    assert!(err.to_string().contains("No messages found"));
}
