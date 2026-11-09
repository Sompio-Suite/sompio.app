//! Grouping repeats of the same value, and deciding on many findings at once.

use sompio_clean::{import_live, run_scan, FindingsFilter, LiveChannel, LiveMessage, ScanOptions, StatusFilter, Store};

fn chan(id: &str, msgs: &[&str]) -> LiveChannel {
    LiveChannel {
        ext_id: id.into(),
        name: id.into(),
        kind: "dm".into(),
        messages: msgs
            .iter()
            .enumerate()
            .map(|(i, t)| LiveMessage { ext_id: format!("{id}/{i}"), ts: 1_700_000_000_000 + i as i64, content: t.to_string(), link: None })
            .collect(),
    }
}

fn setup() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    // The same number written three ways, on two platforms, plus a different number.
    import_live(&mut store, "telegram", "t", vec![chan("a", &["call me on +31 6 12345678", "my number: +31612345678"])]).unwrap();
    import_live(&mut store, "email", "e", vec![chan("sent", &["Phone +31 6 1234 5678", "other: +31 6 87654321"])]).unwrap();
    run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    store
}

#[test]
fn groups_the_same_value_across_platforms() {
    let store = setup();
    let all = store.findings_filtered(&FindingsFilter { kind: Some("phone"), ..Default::default() }, 0, 50, false).unwrap();
    assert_eq!(all.total, 4);
    let grouped = store.findings_filtered(&FindingsFilter { kind: Some("phone"), group: true, ..Default::default() }, 0, 50, false).unwrap();
    assert_eq!(grouped.total, 2);
    assert_eq!(grouped.items[0].repeats, 3, "the repeated number comes first");
    assert_eq!(grouped.items[1].repeats, 1);

    let info = store.group_info(grouped.items[0].id, StatusFilter::Open).unwrap();
    assert_eq!(info.ids.len(), 3);
    assert_eq!(info.platforms.iter().map(|p| (p.platform.as_str(), p.count)).collect::<Vec<_>>(), vec![("telegram", 2), ("email", 1)]);

    let same = store.findings_filtered(&FindingsFilter { same_as: Some(grouped.items[0].id), ..Default::default() }, 0, 50, false).unwrap();
    assert_eq!(same.total, 3);
}

#[test]
fn bulk_decisions_can_be_undone() {
    let mut store = setup();
    let grouped = store.findings_filtered(&FindingsFilter { kind: Some("phone"), group: true, ..Default::default() }, 0, 50, false).unwrap();
    let one = grouped.items[1].id;
    store.set_status(one, Some("later")).unwrap();

    let info = store.group_info(grouped.items[0].id, StatusFilter::Open).unwrap();
    let mut ids = info.ids.clone();
    ids.push(one);
    let before = store.set_status_many(&ids, Some("ok")).unwrap();
    assert_eq!(store.scan_summary().unwrap().open, 0);

    store.restore_statuses(&before).unwrap();
    let s = store.scan_summary().unwrap();
    assert_eq!((s.open, s.later), (3, 1), "each went back to what it was");
}

#[test]
fn old_databases_get_fingerprints() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sompio.db");
    {
        let store = setup_at(&path);
        drop(store);
    }
    // Simulate a database from before fingerprints existed.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("DROP INDEX findings_fp; ALTER TABLE findings DROP COLUMN fp;").unwrap();
    drop(conn);
    let store = Store::open(&path).unwrap();
    let grouped = store.findings_filtered(&FindingsFilter { kind: Some("phone"), group: true, ..Default::default() }, 0, 50, false).unwrap();
    assert_eq!(grouped.total, 2);
}

fn setup_at(path: &std::path::Path) -> Store {
    let mut store = Store::open(path).unwrap();
    import_live(&mut store, "telegram", "t", vec![chan("a", &["call me on +31 6 12345678", "my number: +31612345678"])]).unwrap();
    import_live(&mut store, "email", "e", vec![chan("sent", &["Phone +31 6 1234 5678", "other: +31 6 87654321"])]).unwrap();
    run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
    store
}
