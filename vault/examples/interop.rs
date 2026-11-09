// Checks files against real KeePassXC: cargo run --example interop -- <dir> export|import (see README).
use sompio_vault::*;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&args[1]);
    let mut v = Vault::open(&dir.join("inner.kdbx"), &[5u8; 32]).unwrap();
    if args[2] == "export" {
        let g = v.add_group(None, "Games").unwrap();
        v.add_entry(&EntryInput { group_id: Some(g), title: "Battle.net".into(), username: "sam".into(), password: Some("Tundra-Velvet-42".into()), url: "https://battle.net".into(), otp: Some("JBSWY3DPEHPK3PXP".into()), notes: "line1\nline2".into(), tags: vec!["gaming".into()], custom: vec![FieldInput { key: "PIN".into(), value: Some("4321".into()), protected: true }], ..Default::default() }).unwrap();
        v.export_kdbx(&dir.join("out.kdbx"), "export-pass-123", None).unwrap();
        println!("exported");
    } else {
        let bytes = std::fs::read(dir.join("fromxc.kdbx")).unwrap();
        let s = v.import_kdbx(&bytes, "xc-pass-123", None, "From KeePassXC").unwrap();
        println!("{:?}", s);
        for e in v.entries(None, "").unwrap() { println!("{} | {} | {} | totp={}", e.group_path, e.title, e.username, e.has_totp); }
        let id = v.entries(None, "mail").unwrap()[0].id.clone();
        println!("pw={}", &*v.reveal(&id, "Password").unwrap());
    }
}
