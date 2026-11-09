//! The database is encrypted at rest with SQLite3 Multiple Ciphers.

use rusqlite::Connection;

#[test]
fn encrypted_file_needs_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let key = "x'2DD29CA851E7B56E4697B0E1F08507293D761A05CE4D1B628663F411A8086D99'";
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch(&format!("PRAGMA key = \"{key}\";")).unwrap();
        c.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES ('my phone is 0612345678');").unwrap();
    }
    let raw = std::fs::read(&path).unwrap();
    assert!(!raw.windows(10).any(|w| w == b"0612345678"), "plain text must not be on disk");
    assert!(!raw.starts_with(b"SQLite format 3"));
    // Without the key it can't be read
    let c = Connection::open(&path).unwrap();
    assert!(c.query_row("SELECT count(*) FROM t", [], |r| r.get::<_, i64>(0)).is_err());
    // With it, it can
    let c = Connection::open(&path).unwrap();
    c.execute_batch(&format!("PRAGMA key = \"{key}\";")).unwrap();
    let n: i64 = c.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn store_encrypts_old_plain_database_and_opens_with_key() {
    use sompio_clean::Store;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clean.db");
    {
        let mut s = Store::open(&path).unwrap();
        sompio_clean::load_demo(&mut s).unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(before.starts_with(b"SQLite format 3"));
    let key = [7u8; 32];
    assert!(Store::encrypt_existing(&path, &key).unwrap());
    assert!(!Store::encrypt_existing(&path, &key).unwrap(), "second time does nothing");
    let raw = std::fs::read(&path).unwrap();
    assert!(!raw.starts_with(b"SQLite format 3"));
    assert!(!raw.windows(8).any(|w| w == b"Weekend "), "sample text isn't readable on disk");
    let s = Store::open_encrypted(&path, &key).unwrap();
    assert!(s.has_data().unwrap());
    assert!(Store::open_encrypted(&path, &[8u8; 32]).err().unwrap().to_string().contains("didn't open"));
    let t = std::time::Instant::now();
    for _ in 0..20 {
        Store::open_encrypted(&path, &key).unwrap();
    }
    assert!(t.elapsed().as_millis() < 2000, "opening stays quick: {:?}", t.elapsed());
}
