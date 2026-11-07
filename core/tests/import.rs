//! Discord, end to end: import (both layouts), stats, search, scan, masking, review, export, wipe.

mod common;
use common::*;
use veil_core::{import_package, load_demo, run_scan, ScanOptions, StatusFilter, Store};

#[test]
fn imports_new_zip_format_and_scans() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("package.zip");
    discord_package(&zip_path);

    let mut store = Store::open(&dir.path().join("veil.db")).unwrap();
    let mut phases = vec![];
    let summary = import_package(&mut store, &zip_path, &mut |p| phases.push(p.phase)).unwrap();
    assert_eq!(summary.platform, "discord");
    assert_eq!((summary.channels, summary.messages, summary.servers), (2, 3, 1));
    assert!(phases.contains(&"Done"));

    let stats = store.stats().unwrap();
    assert_eq!((stats.messages, stats.dms, stats.with_attachments), (3, 1, 1));
    assert_eq!(stats.per_year.iter().map(|y| y.year).collect::<Vec<_>>(), vec![2021, 2023]);
    assert_eq!(stats.sources[0].label, "Discord");

    let channels = store.channels(None).unwrap();
    assert_eq!(channels[0].name, "general in Test Server");
    assert_eq!(channels[0].platform, "discord");
    let dm = &store.channels(Some("friend")).unwrap()[0];
    assert_eq!(dm.kind, "dm");

    let general = &channels[0].id;
    let page = store.messages(general, 0, 50).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].link.as_deref(), Some(format!("https://discord.com/channels/999/111/{M1}").as_str()));
    assert_eq!(page.items[1].attachments.len(), 1);
    assert_eq!(store.message_index(general, &page.items[1].id).unwrap(), 1);

    assert_eq!(store.search("hell", None, 0, 10).unwrap().total, 1);
    assert_eq!(store.search("\"cat\" AND (", None, 0, 10).unwrap().total, 0);
    assert_eq!(store.search("cat", Some(&dm.id), 0, 10).unwrap().total, 0);

    let s = run_scan(&mut store, &ScanOptions { watch_terms: vec!["prinsengracht".into()] }, &mut |_, _| {}).unwrap();
    assert_eq!((s.scanned, s.flagged_messages), (3, 2));
    assert_eq!(count(&s, "email"), 1);
    assert_eq!(count(&s, "card"), 1);
    assert_eq!(count(&s, "address") + count(&s, "watch_term"), 1, "overlapping matches keep one");

    // Masked by default: no raw card number or email reaches the UI
    for f in store.findings(None, StatusFilter::All, 0, 100, false).unwrap().items {
        let shown = format!("{}{}{}", f.before, f.matched, f.after);
        assert!(!shown.contains("4111 1111") && !shown.contains("someone@example"), "leaked: {shown}");
    }
    let card = store.findings(Some("card"), StatusFilter::All, 0, 10, false).unwrap().items.remove(0);
    assert_eq!(card.link.as_deref(), Some(format!("https://discord.com/channels/@me/222/{M3}").as_str()));
    assert_eq!(store.reveal_finding(card.id).unwrap().unwrap().matched, "4111 1111 1111 1111");

    // Export: IDs and links, no text
    let out = dir.path().join("flagged.csv");
    assert_eq!(store.export_flagged_csv(&out).unwrap(), 2);
    let csv = std::fs::read_to_string(&out).unwrap();
    assert!(csv.contains(M3) && !csv.contains("4111"));

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
    let c = &store.channels(None).unwrap()[0];
    assert_eq!(c.kind, "server");
    assert_eq!(store.messages(&c.id, 0, 10).unwrap().items[0].content, "multi\nline, with comma");
    assert_eq!(run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap().flagged_messages, 1);
}

#[test]
fn rejects_files_that_are_not_exports() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.zip");
    write_zip(&p, &[("readme.txt", "hi".into())]);
    let mut store = Store::open_in_memory().unwrap();
    let err = import_package(&mut store, &p, &mut |_| {}).unwrap_err();
    assert!(err.to_string().contains("doesn't recognise"));
}

#[test]
fn review_ignore_and_masking() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("package.zip");
    discord_package(&zip_path);
    let mut store = Store::open_in_memory().unwrap();
    import_package(&mut store, &zip_path, &mut |_| {}).unwrap();
    let opts = ScanOptions::default();
    run_scan(&mut store, &opts, &mut |_, _| {}).unwrap();

    let card = store.findings(Some("card"), StatusFilter::All, 0, 1, false).unwrap().items.remove(0);
    store.set_status(card.id, Some("deleted")).unwrap();
    let s = run_scan(&mut store, &opts, &mut |_, _| {}).unwrap();
    assert_eq!(s.done, 1, "decisions survive a rescan");

    // ...and a re-import of a newer export
    import_package(&mut store, &zip_path, &mut |_| {}).unwrap();
    let s = run_scan(&mut store, &opts, &mut |_, _| {}).unwrap();
    assert_eq!(s.done, 1, "decisions survive a re-import");
    let card_id = store.findings(Some("card"), StatusFilter::Done, 0, 1, false).unwrap().items[0].id;
    store.set_status(card_id, None).unwrap();
    assert_eq!(store.scan_summary().unwrap().done, 0);
    assert!(store.set_status(card_id, Some("bogus")).is_err());

    let email_id = store.findings(Some("email"), StatusFilter::All, 0, 1, false).unwrap().items[0].id;
    assert_eq!(store.ignore_everywhere(email_id).unwrap(), 1);
    let s = run_scan(&mut store, &opts, &mut |_, _| {}).unwrap();
    assert_eq!((count(&s, "email"), s.ignored_rules), (0, 1));
    store.clear_ignored().unwrap();
    assert_eq!(count(&run_scan(&mut store, &opts, &mut |_, _| {}).unwrap(), "email"), 1);

    let dm = store.channels(Some("friend")).unwrap().remove(0);
    let m = store.messages(&dm.id, 0, 10).unwrap().items.remove(0);
    assert!(m.has_sensitive && !m.content.contains("4111 1111"));
    assert!(store.reveal_message(&m.id).unwrap().unwrap().content.contains("4111 1111 1111 1111"));
}

#[test]
fn sample_data_loads_and_has_findings() {
    let mut store = Store::open_in_memory().unwrap();
    load_demo(&mut store).unwrap();
    assert!(store.is_demo().unwrap());
    let stats = store.stats().unwrap();
    assert!(stats.messages > 2000 && stats.servers == 2 && stats.dms == 2);
    assert_eq!(stats.sources.len(), 2);
    let s = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    for k in ["email", "phone", "address", "iban", "card", "credential", "ip_address"] {
        assert!(count(&s, k) > 0, "sample data should show {k}");
    }
    // A real import replaces the sample flag
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("package.zip");
    discord_package(&zip_path);
    import_package(&mut store, &zip_path, &mut |_| {}).unwrap();
    assert!(!store.is_demo().unwrap());
    assert_eq!(store.sources().unwrap().len(), 1, "sample data is gone");
}
