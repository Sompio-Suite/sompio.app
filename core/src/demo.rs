//! Sample data, so people can see what Veil does while Discord prepares their real package.
//! Everything here is made up: example.org addresses, test card numbers, the textbook IBAN.

use crate::Store;
use anyhow::Result;

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

const CHANNELS: &[(&str, &str, Option<(i64, &str)>)] = &[
    ("general", "server", Some((700_000_000_000_000_001, "Night Owls Gaming"))),
    ("raid-planning", "server", Some((700_000_000_000_000_001, "Night Owls Gaming"))),
    ("trading", "server", Some((700_000_000_000_000_001, "Night Owls Gaming"))),
    ("help", "server", Some((700_000_000_000_000_002, "Home Lab Builders"))),
    ("show-your-setup", "server", Some((700_000_000_000_000_002, "Home Lab Builders"))),
    ("Direct message with sam", "dm", None),
    ("Direct message with noor", "dm", None),
    ("Weekend crew", "group_dm", None),
];

const WORDS: &[&str] = &[
    "anyone", "up", "for", "a", "raid", "tonight", "the", "boss", "was", "rough", "need", "healer", "brb",
    "dinner", "that", "patch", "broke", "my", "ui", "see", "you", "later", "nice", "drop", "gg", "honestly",
    "no", "idea", "what", "happened", "lol", "server", "is", "lagging", "again", "who", "has", "spare", "mats",
];

const LEAKS: &[&str] = &[
    "my email is sam.rivers.private@example.org if you need it",
    "text me on 06-12345678 when you're outside",
    "you can send it to Keizersgracht 123, 1015 CJ Amsterdam",
    "IBAN is NL91 ABNA 0417 1643 00, name on the account is mine",
    "card ends weird, it's 4111 1111 1111 1111 exp 09/28",
    "login is sammy and password: Winter2024!",
    "here's my token ghp_q8KQx1W2Lr0bN5yTzVvA3sD7fG9hJ1kL2mN4 lol don't steal it",
    "my home server is at 81.204.37.12 port 25565",
    "I'm Sam Rivers btw, nice to meet everyone",
    "call me at +31 6 98765432 if discord is down",
];

/// Replaces whatever is imported with sample data.
pub fn load_demo(store: &mut Store) -> Result<()> {
    store.wipe()?;
    let mut seed: u64 = 42;
    let mut rand = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };

    let tx = store.conn.transaction()?;
    {
        let mut ins_c = tx.prepare(
            "INSERT INTO channels(id, name, kind, guild_id, guild_name) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut ins_m = tx.prepare(
            "INSERT OR IGNORE INTO messages(id, channel_id, ts, content, attachments) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let start = 1_525_132_800_000i64; // May 2018
        let span = 1_700_000_000_000i64 - start;
        let mut counter = 0i64;

        for (ci, (name, kind, guild)) in CHANNELS.iter().enumerate() {
            let cid = 800_000_000_000_000_000 + ci as i64;
            let display = match guild {
                Some((_, g)) => format!("{name} in {g}"),
                None => name.to_string(),
            };
            ins_c.execute((cid, &display, kind, guild.map(|g| g.0), guild.map(|g| g.1)))?;

            let n = 300 + (rand() * 1400.0) as usize;
            let mut ts = start + (rand() * span as f64 * 0.3) as i64;
            for _ in 0..n {
                ts += (rand() * (span as f64 * 0.7 / n as f64) * 2.0) as i64;
                counter += 1;
                let id = ((ts - DISCORD_EPOCH_MS) << 22) + (counter % 4096);
                let len = 3 + (rand() * 18.0) as usize;
                let mut text = (0..len).map(|_| WORDS[(rand() * WORDS.len() as f64) as usize]).collect::<Vec<_>>().join(" ");
                if rand() < 0.012 {
                    text = LEAKS[(rand() * LEAKS.len() as f64) as usize].to_string();
                }
                let att = if rand() < 0.04 {
                    format!("https://cdn.discordapp.com/attachments/{cid}/{id}/screenshot.png")
                } else {
                    String::new()
                };
                ins_m.execute((id, cid, ts, &text, &att))?;
            }
        }
    }
    tx.commit()?;
    store.conn.execute_batch(
        "UPDATE channels SET
            message_count = (SELECT COUNT(*) FROM messages m WHERE m.channel_id = channels.id),
            first_ts      = (SELECT MIN(ts) FROM messages m WHERE m.channel_id = channels.id),
            last_ts       = (SELECT MAX(ts) FROM messages m WHERE m.channel_id = channels.id);
         INSERT INTO messages_fts(messages_fts) VALUES('rebuild');",
    )?;
    store.set_meta("demo", "1")?;
    store.set_meta("imported_at_ms", &crate::now_ms().to_string())?;
    Ok(())
}

impl Store {
    /// True while the sample data is loaded instead of a real package.
    pub fn is_demo(&self) -> Result<bool> {
        Ok(self.get_meta("demo")?.as_deref() == Some("1"))
    }
}
