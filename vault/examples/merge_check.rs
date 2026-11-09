// KeePassXC merge check: cargo run --example merge_check -- <dir> export|merge
use sompio_vault::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let mut v = Vault::open(&dir.join("inner.kdbx"), &[5u8; 32]).unwrap();
    if a[2] == "export" {
        let g = v.add_group(None, "Games").unwrap();
        let id = v.add_entry(&EntryInput { group_id: Some(g), title: "Steam".into(), username: "alex".into(), password: Some("before".into()), ..Default::default() }).unwrap();
        v.add_attachment(&id, "codes.txt", b"backup codes 1234".to_vec()).unwrap();
        v.export_kdbx(&dir.join("copy.kdbx"), "copy-pass-123", None).unwrap();
    } else {
        let m = v.merge_kdbx(&std::fs::read(dir.join("copy.kdbx")).unwrap(), "copy-pass-123", None).unwrap();
        println!("{m:?}");
        for e in v.entries(None, "").unwrap() {
            println!("{} / {} pw={} history={}", e.group_path, e.title, &*v.reveal(&e.id, "Password").unwrap(), v.entry(&e.id).unwrap().history.len());
        }
    }
}
