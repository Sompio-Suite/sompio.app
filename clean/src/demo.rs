//! Sample data, so people can see what Sompio does while they wait for their real exports.
//! Everything here is made up: example.org addresses, test card numbers, the textbook IBAN.

use crate::import::{after_import, Writer};
use crate::Store;
use anyhow::Result;

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

const WORDS: &[&str] = &[
    "anyone", "up", "for", "a", "raid", "tonight", "the", "boss", "was", "rough", "need", "healer", "brb",
    "dinner", "that", "patch", "broke", "my", "ui", "see", "you", "later", "nice", "drop", "gg", "honestly",
    "no", "idea", "what", "happened", "lol", "server", "is", "lagging", "again", "who", "has", "spare", "mats",
];

/// One made-up person, Alex Rivers, the same as in the Vault and Calendar samples.
const LEAKS: &[&str] = &[
    "my email is alex.rivers@example.org if you need it",
    "text me on 06-12345678 when you're outside",
    "you can send it to Keizersgracht 123, 1015 CJ Amsterdam",
    "IBAN is NL91 ABNA 0417 1643 00, name on the account is mine",
    "card ends weird, it's 4111 1111 1111 1111 exp 09/28",
    "login for the shop is alex.rivers@example.org, password: BrightMarble77",
    "here's my token ghp_q8KQx1W2Lr0bN5yTzVvA3sD7fG9hJ1kL2mN4 lol don't steal it",
    "my home server is at 81.204.37.12 port 25565",
    "I'm Alex Rivers btw, nice to meet everyone",
    "call me at +31 6 98765432 if discord is down",
    "for the rental form: my BSN is 111222333",
    "photo of my passport for the booking: passport number NXC4R9P12",
    "saving these here so I don't lose them, discord backup codes: 8f3k-2m9x 7h2p-4q8z",
    "party is on my birthday, born on 14-03-1994 so I'm turning the big one",
    "parked outside, the grey one, GZ-123-K",
    "come over, we're here https://www.google.com/maps/@52.3731234,4.8903456,17z",
    "seed phrase: abandon ability able about above absent absorb abstract absurd abuse access accident",
];

struct Rng(u64, usize);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
    /// The next leak in turn, so every kind shows up in the sample.
    fn leak(&mut self) -> &'static str {
        self.1 += 1;
        LEAKS[self.1 % LEAKS.len()]
    }
    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[(self.next() * list.len() as f64) as usize % list.len()]
    }
    fn chatter(&mut self) -> String {
        let len = 3 + (self.next() * 18.0) as usize;
        (0..len).map(|_| self.pick(WORDS)).collect::<Vec<_>>().join(" ")
    }
}

/// Replaces whatever is imported with sample data from two platforms.
pub fn load_demo(store: &mut Store) -> Result<()> {
    store.wipe()?;
    let mut rng = Rng(42, 0);
    let start = 1_525_132_800_000i64; // May 2018
    let span = 1_700_000_000_000i64 - start;

    // Discord
    let discord: &[(&str, &str, Option<(&str, &str)>)] = &[
        ("general", "server", Some(("700000000000000001", "Night Owls Gaming"))),
        ("raid-planning", "server", Some(("700000000000000001", "Night Owls Gaming"))),
        ("trading", "server", Some(("700000000000000001", "Night Owls Gaming"))),
        ("help", "server", Some(("700000000000000002", "Home Lab Builders"))),
        ("Direct message with sam", "dm", None),
        ("Direct message with noor", "dm", None),
        ("Weekend crew", "group_dm", None),
    ];
    let mut w = Writer::new(store, "discord")?;
    let mut counter = 0i64;
    for (ci, (name, kind, guild)) in discord.iter().enumerate() {
        let cid_ext = (800_000_000_000_000_000i64 + ci as i64).to_string();
        let display = match guild {
            Some((_, g)) => format!("{name} in {g}"),
            None => name.to_string(),
        };
        let cid = w.channel(&cid_ext, &display, kind, guild.map(|g| g.0), guild.map(|g| g.1))?;
        let n = 300 + (rng.next() * 1400.0) as usize;
        let mut ts = start + (rng.next() * span as f64 * 0.3) as i64;
        for _ in 0..n {
            ts += (rng.next() * (span as f64 * 0.7 / n as f64) * 2.0) as i64;
            counter += 1;
            let id = (((ts - DISCORD_EPOCH_MS) << 22) + (counter % 4096)).to_string();
            let text = if rng.next() < 0.012 { rng.leak().to_string() } else { rng.chatter() };
            let att = if rng.next() < 0.04 { format!("https://cdn.discordapp.com/attachments/{cid_ext}/{id}/screenshot.png") } else { String::new() };
            let link = format!("https://discord.com/channels/{}/{cid_ext}/{id}", guild.map(|g| g.0).unwrap_or("@me"));
            w.message(&id, cid, ts, &text, &att, Some(&link))?;
        }
    }
    w.finish("Sample data")?;

    // Reddit
    let mut w = Writer::new(store, "reddit")?;
    for sub in ["homelab", "Amsterdam", "buildapc"] {
        let cid = w.channel(&format!("r/{sub}"), &format!("r/{sub}"), "community", None, None)?;
        for i in 0..(80 + (rng.next() * 120.0) as usize) {
            let ts = start + (rng.next() * span as f64) as i64;
            let text = if rng.next() < 0.03 { rng.leak().to_string() } else { rng.chatter() };
            let id = format!("t1_{sub}{i}");
            let link = format!("https://www.reddit.com/r/{sub}/comments/demo/_/{i}/");
            w.message(&id, cid, ts, &text, "", Some(&link))?;
        }
    }
    w.finish("Sample data")?;

    // A few more platforms, smaller
    for (platform, chats) in [
        ("telegram", &["Mara", "Builders"][..]),
        ("instagram", &["sam.k", "noor"][..]),
        ("whatsapp", &["Family", "Sam"][..]),
        ("email", &["Sent mail"][..]),
    ] {
        let mut w = Writer::new(store, platform)?;
        for (i, chat) in chats.iter().enumerate() {
            let cid = w.channel(&format!("{platform}-{i}"), chat, if chats.len() > 1 { "dm" } else { "posts" }, None, None)?;
            for j in 0..(40 + (rng.next() * 80.0) as usize) {
                let ts = start + (rng.next() * span as f64) as i64;
                let text = if rng.next() < 0.05 { rng.leak().to_string() } else { rng.chatter() };
                w.message(&format!("{platform}-{i}-{j}"), cid, ts, &text, "", None)?;
            }
        }
        w.finish("Sample data")?;
    }

    after_import(store)?;
    store.set_meta("demo", "1")?;
    Ok(())
}
