//! sompio-voice-server: run your own Sompio Voice server.
//!
//!   sompio-voice-server                      start (data in ./sompio-voice-data, UDP port 9987)
//!   sompio-voice-server --port 9988 --data /var/lib/sompio-voice --name "Night Owls"
//!   sompio-voice-server set-password <pw>    people need this to join ("" removes it)
//!   sompio-voice-server new-admin-key        if the first admin key got lost

use sompio_voice::server;
use std::path::PathBuf;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let data = PathBuf::from(arg(&args, "--data").unwrap_or_else(|| "sompio-voice-data".into()));
    match args.first().map(String::as_str) {
        Some("set-password") => {
            let pw = args.get(1).cloned().unwrap_or_default();
            server::set_password(&data, Some(&pw))?;
            println!("{}", if pw.is_empty() { "Password removed. Restart the server." } else { "Password set. Restart the server." });
            return Ok(());
        }
        Some("new-admin-key") => {
            println!("New admin key: {}", server::new_admin_key(&data)?);
            println!("Restart the server, then use it in Sompio: Voice, server menu, Use an admin key.");
            return Ok(());
        }
        Some("--help") | Some("-h") | Some("help") => {
            println!("{}", include_str!("server.rs").lines().skip(2).take(5).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
            return Ok(());
        }
        _ => {}
    }
    let port: u16 = arg(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(sompio_voice::proto::DEFAULT_PORT);
    let name = arg(&args, "--name").unwrap_or_else(|| "Sompio Voice".into());
    let r = server::start(server::Options { data_dir: data.clone(), port, name, public: true }).await?;
    println!("Sompio Voice server \"{}\" is running.", r.name);
    println!("  Port:        UDP {} (open it in your firewall / router)", r.addr.port());
    println!("  Fingerprint: {}", r.fingerprint);
    println!("               People see this the first time they connect. Tell them to check it matches.");
    println!("  Data:        {}", std::fs::canonicalize(&data).unwrap_or(data).display());
    if let Some(k) = &r.first_admin_key {
        println!();
        println!("  Admin key:   {k}");
        println!("               Connect with Sompio and use this key (server menu, Use an admin key) to become admin.");
        println!("               It works once. It's also saved in server.json until used.");
    }
    println!();
    println!("Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    r.stop().await;
    Ok(())
}
