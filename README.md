# Veil

Find personal information in your Discord history, offline.

Veil reads the data package Discord gives you (User Settings › Data & Privacy › Request all of my data)
and shows you every message where you shared something like a phone number, home address, bank details,
a password or an access token. Nothing leaves your computer.

> "Veil" is a working name. Rename it before you publish (`productName` and `identifier` in
> `app/src-tauri/tauri.conf.json`, and the `name` fields in the `package.json`/`Cargo.toml` files).

## Run it on Windows

One-time setup:

1. Install **Rust**: https://rustup.rs (accept the defaults; it will offer the Visual Studio C++ build tools, say yes).
2. Install **Node.js 20 or newer**: https://nodejs.org
3. WebView2 is already part of Windows 10 and 11.

Then, in a terminal inside the `app` folder:

```
npm install
npm run tauri dev      # opens the app with hot reload
npm run tauri build    # makes an installer in app/src-tauri/target/release/bundle/
```

The first Rust build takes a few minutes; later ones are quick.

**UI only, no Rust:** `npm run dev` and open http://localhost:1420 in a browser. It runs on fake data,
so you can work on the design without building the app.

**Engine only, no UI:** from the `core` folder,

```
cargo test                                         # run the tests
cargo run --release --bin veil-cli -- import "C:\path\to\package.zip"
cargo run --release --bin veil-cli -- scan "Your Name, Your Street"
cargo run --release --bin veil-cli -- report        # masked
cargo run --release --bin veil-cli -- report email --reveal
```

## How it's put together

```
core/              Rust library, all the real work. No UI, no network.
  src/import.rs      reads the package (.zip or folder, old CSV and new JSON layouts)
  src/db.rs          the local SQLite database
  src/query.rs       stats, conversations, paged messages, full-text search
  src/scan.rs        the exposure scanner and the masked results
  src/bin/veil-cli.rs  command-line access to the same engine
  tests/             tests with fake packages in both layouts
app/
  src-tauri/         the desktop shell: passes UI requests to core, nothing else
  src/               the React interface
    api.ts           typed bridge to Rust (falls back to mock.ts in a browser)
    usePaged.ts      loads only the rows on screen, so huge histories stay smooth
    views/           Welcome, Overview, Exposure report, Messages, Settings
```

## Why it stays fast

Tested with a 1 million message package: import takes about 6 seconds, a full scan about 1.5 seconds.

- Parsing, searching and scanning happen in Rust, on a background thread. The window never waits on them.
- Lists are virtualized: only the ~30 rows you can see exist on screen, and data loads 200 rows at a time.
- Search uses SQLite's full-text index instead of scanning every message.
- The zip is read file by file and the huge Activity/analytics folders are skipped without unpacking.
- No UI framework: plain React and one CSS file.

## Privacy design (keep these when you change things)

- **No network.** `core` has no HTTP library. The window's content security policy blocks every remote
  connection, and attachments are listed by file name instead of being loaded from Discord's servers.
- **Smallest possible permissions.** The window can only call Veil's own commands and open file pickers
  (`app/src-tauri/capabilities/default.json`). No shell, no file system access, no HTTP.
- **No login.** Veil never asks for a Discord token or password.
- **One file, deletable.** Data lives in `veil.db` in the local (non-roaming) app data folder. "Delete everything"
  removes it with SQLite's secure delete and compacts the file so old content isn't left on disk.
- **Masked by default.** Findings are shown as redaction bars until you choose to show them. Every sensitive part
  of a preview is masked, not just the match being listed.
- **No duplicates of your secrets.** Findings store positions in the message, not copies of the text.
- **Exports hold IDs only.** The flagged-messages CSV has channel IDs, message IDs and dates, no message text.

## What the scanner looks for

| Type | How false alarms are kept down |
| --- | --- |
| Passwords, tokens, API keys | Known formats (Discord tokens, JWTs, GitHub, AWS, OpenAI, Slack) and "password: …" phrases |
| Payment cards | Luhn checksum, real card prefixes, Discord mentions and IDs ignored |
| IBANs | Full IBAN checksum |
| Street addresses | Dutch/German, English and Romanian patterns |
| Phone numbers | International, Dutch and US formats, digit-count checks |
| Email, public IP addresses | Private and local IPs and version numbers ignored |
| Dutch postcodes, crypto wallets | Low severity |
| Watch terms | Whatever you add: real name, street, school, employer |

To add a pattern: add a line in `Scanner::new` in `core/src/scan.rs`, a check in `validate` if it needs one,
and a test in `core/tests/scanner.rs` that it catches the real thing and ignores lookalikes.

## Roadmap

1. **Now:** offline import, exposure scan, search, browsing.
2. **Next:** exports (HTML, JSON, CSV) of conversations; encrypt the database with a passphrase (SQLCipher).
3. **Later:** optional clean-up mode that deletes flagged messages through Discord. This needs your token and
   automates your user account, which Discord's terms prohibit. Make it opt-in, slow and very clearly explained.
4. **Then:** other platforms (Telegram and Reddit exports use the same idea).
