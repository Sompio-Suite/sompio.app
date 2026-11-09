//! Command-line access to the same engine the app uses. Handy for testing on your own
//! package without the UI:
//!
//!   sompio-clean-cli import  <export.zip|folder>   (Discord, Reddit, Telegram or X)
//!   sompio-clean-cli folders <folder> [folder…]    (scan files on this PC)
//!   sompio-clean-cli sources
//!   sompio-clean-cli scan    [watch term, another term]
//!   sompio-clean-cli report  [kind] [--reveal]
//!   sompio-clean-cli search  <text>
//!   sompio-clean-cli demo       (load sample data)
//!   sompio-clean-cli wipe
//!
//! Uses ./sompio.db unless SOMPIO_DB is set.

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::time::Instant;
use sompio_clean::{import_package, load_demo, run_scan, scan_folders, ScanOptions, StatusFilter, Store};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let db = std::env::var("SOMPIO_DB").map(PathBuf::from).unwrap_or_else(|_| "sompio.db".into());
    let mut store = Store::open(&db)?;
    let t = Instant::now();

    match args.first().map(String::as_str) {
        Some("import") => {
            let Some(path) = args.get(1) else { bail!("usage: sompio-clean-cli import <package.zip|folder>") };
            let s = import_package(&mut store, path.as_ref(), &mut |p| {
                if p.total > 0 && (p.done % 200 == 0 || p.done == p.total) {
                    eprintln!("{}: {}/{}", p.phase, p.done, p.total);
                }
            })?;
            println!("Imported {} {} messages from {} channels in {:.1?}", s.messages, sompio_clean::platform_label(&s.platform), s.channels, t.elapsed());
            for w in s.warnings {
                println!("  warning: {w}");
            }
        }
        Some("folders") => {
            let folders: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
            let s = scan_folders(&mut store, &folders, &mut |_| {})?;
            println!("Read {} files in {} folders in {:.1?}", s.messages, s.channels, t.elapsed());
            for w in s.warnings {
                println!("  warning: {w}");
            }
        }
        Some("sources") => {
            for s in store.sources()? {
                println!("{:<14} {:>8} items  from {}", s.label, s.messages, s.origin.replace('\n', ", "));
            }
        }
        Some("scan") => {
            let terms: Vec<String> = args[1..].join(" ").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let terms = if terms.is_empty() { store.watch_terms()? } else { terms };
            let s = run_scan(&mut store, &ScanOptions { watch_terms: terms }, &mut |_, _| {})?;
            println!("Scanned {} messages in {:.1?}. {} need attention.\n", s.scanned, t.elapsed(), s.flagged_messages);
            for k in s.by_kind.iter().filter(|k| k.count > 0) {
                println!("  {:<28} {:>6}  ({} to review)", k.label, k.count, k.open);
            }
        }
        Some("report") => {
            let reveal = args.iter().any(|a| a == "--reveal");
            let kind = args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str);
            let page = store.findings(kind, StatusFilter::All, 0, 50, reveal)?;
            println!("{} findings (showing up to 50)\n", page.total);
            for f in page.items {
                println!("[{}] {} — {}\n    {}[{}]{}\n", f.label, sompio_clean::platform_label(&f.platform), f.channel_name, f.before, f.matched, f.after);
            }
        }
        Some("search") => {
            let page = store.search(&args[1..].join(" "), None, 0, 20)?;
            println!("{} matches\n", page.total);
            for m in page.items {
                println!("{} | {}\n    {}\n", m.channel_name, m.id, m.content.replace('\n', " "));
            }
        }
        Some("demo") => {
            load_demo(&mut store)?;
            println!("Loaded sample data into {}", db.display());
        }
        Some("wipe") => {
            store.wipe()?;
            println!("All imported data removed from {}", db.display());
        }
        _ => println!("usage: sompio-clean-cli <import|folders|sources|demo|scan|report|search|wipe> ..."),
    }
    Ok(())
}
