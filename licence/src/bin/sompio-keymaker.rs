//! Sompio Key Maker: makes licence keys on your own PC.
//!
//! Double-click it for a simple menu. The first run creates the signing key, the master key for
//! every licence, in your user folder. Back that file up and never share it: anyone who has it
//! can make keys.
//!
//! For scripts and, later, the website:
//!   sompio-keymaker init --public-out <file>     create the signing key if needed, write its public half
//!   sompio-keymaker issue --apps suite --days month|year|forever|<n> [--count 5] [--test]
//!   sompio-keymaker check <key>

use anyhow::{bail, Context, Result};
use sompio_licence::{issue, new_signing_key, parse, parse_days, length_label, signing_key_from_hex, to_hex, App, Apps};
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

fn secret_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("SOMPIO_KEYMAKER_DIR") {
        return PathBuf::from(dir).join("signing-key.secret");
    }
    match std::env::var_os("APPDATA") {
        Some(appdata) => PathBuf::from(appdata).join("Sompio Key Maker").join("signing-key.secret"),
        None => home().join(".config").join("sompio-keymaker").join("signing-key.secret"),
    }
}

fn keys_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SOMPIO_KEYMAKER_DIR") {
        return PathBuf::from(dir).join("keys");
    }
    let docs = home().join("Documents");
    if docs.is_dir() { docs.join("Sompio Keys") } else { home().join("Sompio Keys") }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Loads the signing key, creating it the first time.
fn signing_key(create: bool) -> Result<(ed25519_dalek::SigningKey, bool)> {
    let path = secret_path();
    if let Ok(hex) = std::fs::read_to_string(&path) {
        return Ok((signing_key_from_hex(&hex).with_context(|| format!("{} is damaged", path.display()))?, false));
    }
    if !create {
        bail!("No signing key yet. Run Sompio Key Maker once to create it.");
    }
    let key = new_signing_key()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, to_hex(&key.to_bytes()))?;
    Ok((key, true))
}

fn date(ms: i64) -> String {
    // Civil date from days since 1970 (Howard Hinnant's algorithm)
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}")
}

fn make(apps: Apps, days: u16, count: usize, test: bool) -> Result<(Vec<String>, PathBuf)> {
    let (signing, _) = signing_key(true)?;
    let now = now_ms();
    let keys: Vec<String> = (0..count).map(|_| issue(&signing, apps, days, now, test)).collect::<Result<_>>()?;
    let dir = keys_dir();
    std::fs::create_dir_all(&dir)?;
    let stamp = now / 1000;
    let file = dir.join(format!("{} {} {}{}.csv", apps.label(), length_label(days), date(now), format!(" {stamp}")));
    let mut csv = String::from("key,apps,days,made,test,serial\n");
    let public = signing.verifying_key();
    for k in &keys {
        let parsed = parse(k, &public)?;
        csv.push_str(&format!("{},{},{},{},{},{}\n", k, apps.label(), days, date(now), test, parsed.serial_hex()));
    }
    std::fs::write(&file, csv)?;
    Ok((keys, file))
}

fn ask(prompt: &str) -> String {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().lock().read_line(&mut s);
    s.trim().to_string()
}

fn interactive() -> Result<()> {
    println!("Sompio Key Maker");
    println!("================\n");
    let (signing, created) = signing_key(true)?;
    if created {
        println!("A new signing key was created at:\n  {}\n", secret_path().display());
        println!("This file is the master key for every Sompio licence.");
        println!("Back it up somewhere safe (a USB stick in a drawer) and never share it.");
        println!("Build Sompio again so the app trusts keys made with it.\n");
    }
    loop {
        println!("What do you want to do?");
        println!("  1  Make keys");
        println!("  2  Check a key");
        println!("  3  Show where things are");
        println!("  q  Quit");
        match ask("> ").to_ascii_lowercase().as_str() {
            "1" => {
                println!("\nWhich apps?");
                println!("  1  Sompio Suite (all apps)");
                println!("  2  Trace");
                println!("  3  Calendar");
                println!("  4  Vault");
                println!("  5  Voice");
                println!("  or type names, like: clean,vault");
                let a = ask("> ");
                let apps = match a.as_str() {
                    "1" | "" => Some(Apps::SUITE),
                    "2" => Some(Apps::of(&[App::Clean])),
                    "3" => Some(Apps::of(&[App::Calendar])),
                    "4" => Some(Apps::of(&[App::Vault])),
                    "5" => Some(Apps::of(&[App::Voice])),
                    other => Apps::parse(other),
                };
                let Some(apps) = apps else {
                    println!("I didn't understand that.\n");
                    continue;
                };
                let days = match ask("How long? (month, year, forever, or a number of days; Enter for a month) > ").as_str() {
                    "" => 31,
                    d => match parse_days(d) {
                        Some(n) => n,
                        None => {
                            println!("Type month, year, forever or a number of days.\n");
                            continue;
                        }
                    },
                };
                let count = ask("How many keys? (Enter for 1) > ").parse::<usize>().unwrap_or(1).clamp(1, 10_000);
                let test = !ask("Mark them as test keys? (Y/n) > ").eq_ignore_ascii_case("n");
                let (keys, file) = make(apps, days, count, test)?;
                println!();
                for k in keys.iter().take(20) {
                    println!("{k}\n");
                }
                if keys.len() > 20 {
                    println!("…and {} more.", keys.len() - 20);
                }
                println!("{} {} for {}, {}{}.", keys.len(), if keys.len() == 1 { "key" } else { "keys" }, apps.label(), length_label(days), if test { ", marked as test" } else { "" });
                println!("Saved to:\n  {}\n", file.display());
            }
            "2" => {
                let k = ask("Paste the key > ");
                match parse(&k, &signing.verifying_key()) {
                    Ok(key) => println!(
                        "\nValid. {} for {}, made {}{}. Must be added before {}. Serial {}.\n",
                        key.apps.label(),
                        length_label(key.days),
                        date(key.issued_ms),
                        if key.test { ", test key" } else { "" },
                        date(key.activate_by_ms()),
                        key.serial_hex()
                    ),
                    Err(e) => println!("\n{e}\n"),
                }
            }
            "3" => {
                println!("\nSigning key (keep it safe):\n  {}", secret_path().display());
                println!("Keys you've made:\n  {}", keys_dir().display());
                println!("Public key (goes into the app):\n  {}\n", to_hex(signing.verifying_key().as_bytes()));
            }
            "q" | "quit" | "exit" => return Ok(()),
            _ => println!(),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = run(&args);
    if let Err(e) = result {
        eprintln!("{e:#}");
        if args.is_empty() {
            ask("\nPress Enter to close.");
        }
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None => interactive(),
        Some("init") => {
            let (signing, created) = signing_key(true)?;
            let public = to_hex(signing.verifying_key().as_bytes());
            if let Some(out) = flag(args, "--public-out") {
                std::fs::write(&out, &public)?;
            }
            if created {
                println!("Created a new signing key at {}. Back it up and never share it.", secret_path().display());
            }
            println!("{public}");
            Ok(())
        }
        Some("issue") => {
            let apps = Apps::parse(&flag(args, "--apps").unwrap_or_else(|| "suite".into())).context("--apps: suite, or names like clean,vault")?;
            let days: u16 = parse_days(&flag(args, "--days").unwrap_or_else(|| "31".into())).context("--days: month, year, forever or a number")?;
            let count: usize = flag(args, "--count").unwrap_or_else(|| "1".into()).parse().context("--count must be a number")?;
            let (keys, file) = make(apps, days, count, args.iter().any(|a| a == "--test"))?;
            for k in keys {
                println!("{k}");
            }
            eprintln!("Saved to {}", file.display());
            Ok(())
        }
        Some("check") => {
            let (signing, _) = signing_key(false)?;
            let k = parse(args.get(1).context("check <key>")?, &signing.verifying_key())?;
            println!("Valid: {} for {}, made {}, serial {}", k.apps.label(), length_label(k.days), date(k.issued_ms), k.serial_hex());
            Ok(())
        }
        Some(other) => bail!("Unknown command {other}. Run without arguments for the menu."),
    }
}
