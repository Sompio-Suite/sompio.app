//! Every sample file imports, and each one has something for the scan to find.
use sompio_clean::{import_package, run_scan, samples, scan_folders, ScanOptions, Store};

#[test]
fn every_sample_imports_and_has_findings() {
    let dir = tempfile::tempdir().unwrap();
    let files = samples::write_samples(dir.path()).unwrap();
    assert!(files.len() >= 12);
    for f in &files {
        let mut store = Store::open_in_memory().unwrap();
        let path = dir.path().join(&f.file);
        let s = if f.platform == "This PC" {
            scan_folders(&mut store, &[path], &mut |_| {}).unwrap()
        } else {
            import_package(&mut store, &path, &mut |_| {}).unwrap_or_else(|e| panic!("{}: {e}", f.file))
        };
        assert!(s.messages > 0, "{} imported nothing", f.file);
        let scan = run_scan(&mut store, &ScanOptions::default(), &mut |_, _| {}).unwrap();
        assert!(scan.total_findings > 0, "{} has nothing to find", f.file);
    }
    assert!(dir.path().join("READ ME.txt").exists());
}
