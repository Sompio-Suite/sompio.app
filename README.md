# Sompio Suite

Private tools that keep your data on your PC, unlocked with a key instead of an account.

| App | Status |
| --- | --- |
| **Clean** | Built: finds and cleans up personal information in your exports and files (below) |
| **Calendar** | Next: Desk Reminders rebuilt, with privacy routines |
| **Vault** | Later: KeePass-compatible password manager |
| **Voice** | Later: end-to-end encrypted voice rooms |

The plan lives in the "Sompio Suite plan" doc.

## The Sompio Lock

Everything Sompio stores is encrypted with one random 256-bit vault key (`lock/`). The vault key is only kept
wrapped (XChaCha20-Poly1305) in `lock.json`, and opens two ways:

- **Master password + key picture.** The password goes through Argon2id (64 MiB, 3 passes) and is mixed with a
  256-bit secret hidden in a night-sky PNG the person keeps on their phone. Both are needed. The secret sits in a
  private PNG chunk and, as backup, in the lowest bit of the blue channel along three rows, so ordinary copying
  keeps it; chat apps that shrink photos destroy it. On a PC the person trusts, the picture's secret can be
  remembered in Windows Credential Manager so only the password is typed there.
- **Recovery code** (120 bits, printed once). It sets a new password, picture and code; the old ones stop working.

Each app gets its own key from the vault key (HKDF). Clean's database is encrypted with SQLite3 Multiple Ciphers
(ChaCha20-Poly1305), vendored as a drop-in for rusqlite's bundled SQLite in `vendor/libsqlite3-sys`, so no
OpenSSL is needed to build. Data from before the lock is encrypted in place on setup. Sompio locks itself after
a chosen idle time. Nobody, including us, can reset a lost password together with a lost recovery code.

Not yet: YubiKey as the second key, Windows Hello for quick unlock, and the Telegram and email credentials still
live in Windows Credential Manager rather than under the lock.

## Releasing to customers

Customers never build anything. They download `Sompio_<version>_x64-setup.exe` from a GitHub Release (linked from
the website) and run it. GitHub builds that file on its own Windows machines:

1. Once: run `Build Sompio.bat` on your PC, so `licence/public.key` holds the public half of your signing key.
   Commit and push that file. It's public by design; the signing key itself stays in `%APPDATA%\Sompio Key Maker`.
2. Each version: raise `"version"` in `app/src-tauri/tauri.conf.json`, push, then on GitHub open
   **Actions › Release › Run workflow**. About 15 minutes later a draft release appears under **Releases** with the
   installer attached. Download it, test it, then press **Publish**.

The release workflow refuses to build while `public.key` is still the development key, because that build would
reject every key you make.

## Licence keys

A key holds which apps it unlocks, for how many days, the day it was made and a random serial, signed with
Ed25519 (`licence/`). The app has only the public half (`licence/public.key`), so it can check keys offline but
never make them. Keys stack: new days start where the current ones end. A key must be added within a year of
being made. Every new install gets a 14-day suite trial. The app remembers the latest date it has seen, so setting
the clock back gains nothing. When access ends, Clean turns read-only: everything stays visible and exportable.

**Making keys.** `Build Sompio.bat` builds `Sompio Key Maker.exe` next to the app. Double-click it for a menu
(make keys, check a key). The first run creates the signing key in `%APPDATA%\Sompio Key Maker\` and the build
writes its public half into the app. Back that folder up and never share it: whoever has it can make keys.
Keys you make are saved as CSV files in `Documents\Sompio Keys\`. For scripts and the future website:
`sompio-keymaker issue --apps suite --days 30 --count 10 [--test]`.

The `public.key` in the repository belongs to a throwaway development key. Builds made by the script replace it
with yours; CI builds still carry the throwaway one until a release key is set up.

# Sompio Clean

Find personal information in your chats, posts, sent mail and files, offline.

Sompio reads the data exports from Discord, WhatsApp, Instagram, Facebook, TikTok, Snapchat, Reddit, Telegram,
X, LinkedIn and Gmail, plus folders you choose on your PC,
and shows you everywhere you shared something like a phone number, home address, bank details,
a password or an access token, then walks you through cleaning it up. Nothing leaves your computer.

| Source | What Sompio reads | Only your own? |
| --- | --- | --- |
| Discord | "Request all of my data" package | Yes, the package only has your messages |
| Reddit | Data request export (CSV files) | Posts and comments are yours; private messages are filtered to the ones you sent |
| Telegram | Telegram Desktop › Export Telegram data, JSON format | Filtered to messages from your account |
| X | "Download an archive of your data" | Your posts (not reposts) and DMs you sent |
| WhatsApp | Export chat (without media), the .txt or .zip, one chat at a time | Your lines only; you're recognised from the chat name or "Your names" |
| Instagram | Download your information, JSON | Messages you sent, your post captions and comments |
| Facebook | Download your information, JSON (includes Messenger) | Messages you sent, your posts and comments |
| TikTok | Download your data, JSON | Your comments, bio and DMs you sent |
| Snapchat | My Data with JSON files | Saved chat messages you sent |
| LinkedIn | Get a copy of your data | Messages you sent, your posts and comments |
| Gmail / email | Google Takeout (.mbox), or connect any IMAP account | Mail you sent only, without quoted replies |
| This PC | Folders you choose | Text files, Word documents, and every file's name (passport scans, key files…) |

## What using it looks like

1. Open an export or scan folders on your PC, or try the built-in sample data while your exports are prepared.
   Add more sources any time on the **Sources** screen; each platform's newest export replaces its last one.
2. Sompio imports and scans straight away, then lands on **Review** with everything it found.
3. Each item says how urgent it is (Fix now, Worth a look, Low risk) and what to do about it.
4. **Open in Discord / on Reddit / on X** jumps to the exact message in your browser so you can delete it by hand.
   For files on your PC: **Show in folder** and **Move to Recycle Bin**.
5. Mark each one **I deleted it**, **Not a problem** or **Later**. Progress is saved and survives rescans.
   **Never flag this again** stops a harmless value (a public work email, say) from coming back.
6. Add watch terms (your real name, street, school) and scan again to catch what patterns can't.
7. **Group repeats** (on by default) shows each value once with how many places it appears, so one decision
   covers all of them. **Show each place** splits it out again. Select several items (checkbox, X, Shift-click)
   to decide on them together, or delete them in one go on a connected account. Everything has Undo.

Sensitive text is hidden everywhere, including in conversations, until you click it.

Keyboard on the Review screen: J/K or arrows to move, Space to show or hide, D deleted, N not a problem,
L later, U move back to review, O open in Discord, C copy link, X select, Esc clear selection.

> "Sompio" is a working name. Rename it before you publish (`productName` and `identifier` in
> `app/src-tauri/tauri.conf.json`, and the `name` fields in the `package.json`/`Cargo.toml` files).

## Connected accounts (Telegram and email)

**Email** works with Gmail, Yahoo, iCloud, GMX, Fastmail or any IMAP server, using an app password
(never the real password). Sompio reads the Sent folder read-only and can move a flagged email to Trash.
Records use the Message-ID, the same ID a Takeout import uses, so decisions carry over between the two.
Outlook.com needs Microsoft OAuth sign-in, which isn't built yet. The account details live in Windows
Credential Manager (`email.account`).

**Telegram:**

On the Sources screen you can connect a Telegram account instead of importing an export. Sompio then
downloads only the messages you sent (Telegram filters them server-side) and can delete a flagged
message for everyone with one click.

- Only `connect/` talks to the network, and only when the person logs in, syncs or deletes.
  The status check is offline. `clean/` stays network-free.
- The login session is kept in Windows Credential Manager (service "Sompio"), never in a file or the database.
  Phone numbers and cloud passwords are sent to Telegram only and never stored.
- Telegram requires an app key (api_id + api_hash, free at my.telegram.org > API development tools).
  Without one built in, each person is asked for their own once.
  To build Sompio's own key into the app, set the environment variables `SOMPIO_TG_API_ID` and
  `SOMPIO_TG_API_HASH` before building (on GitHub: add them as repository secrets; the workflows pass them on).
  Keep the hash out of the repository.

## Getting the Windows program (no setup needed)

GitHub builds Sompio for you. You don't need Node or Rust on your computer for this.

1. Put this folder on GitHub as a repository (GitHub Desktop: File › Add local repository, then Publish).
2. Every push starts the **Build** workflow. Open the repository's **Actions** tab, click the latest run,
   and download **Sompio-windows** under Artifacts. It contains:
   - `Sompio_<version>_x64-setup.exe`: the installer. No admin rights needed.
   - `Sompio_<version>_x64_en-US.msi`: the same, for IT departments.
   - `Sompio-portable.exe`: runs without installing.
3. To publish a version for others: push a tag like `v0.1.0`, or run the **Release** workflow from the
   Actions tab. It creates a draft on the Releases page with the installer attached. Check it, then publish.

Until the app is code-signed, Windows shows "Windows protected your PC" on first run.
Users click **More info**, then **Run anyway**.

## Developing on Windows

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

**Engine only, no UI:** from the `clean` folder,

```
cargo test                                         # run the tests
cargo run --release --bin sompio-clean-cli -- import "C:\path\to\export.zip"   # any supported platform
cargo run --release --bin sompio-clean-cli -- folders "C:\Users\you\Documents"
cargo run --release --bin sompio-clean-cli -- sources
cargo run --release --bin sompio-clean-cli -- scan "Your Name, Your Street"
cargo run --release --bin sompio-clean-cli -- report        # masked
cargo run --release --bin sompio-clean-cli -- report email --reveal
cargo run --release --bin sompio-clean-cli -- demo          # load the sample data
```

## How it's put together

```
clean/             Rust library (Sompio Clean), all the real work. No UI, no network.
connect/           Live account connections (Telegram, email over IMAP). The only code that goes online.
  src/import/        one reader per platform; mod.rs detects which export it is
  src/pcscan.rs      the PC folder scan and Recycle Bin
  src/db.rs          the local SQLite database
  src/query.rs       stats, conversations, paged messages, full-text search
  src/scan.rs        the exposure scanner, masked results, review decisions, never-flag rules
  src/demo.rs        made-up sample data for trying the app
  src/bin/sompio-clean-cli.rs  command-line access to the same engine
  tests/             tests with fake packages in both layouts
licence/           Licence keys: signing, offline checking, the trial, and Sompio Key Maker.
lock/              The Sompio Lock: Argon2id, key wrapping, the key picture, recovery codes.
suite/             Glue for the app: setup, unlock, recovery, each app's database key.
vendor/            libsqlite3-sys with SQLite3 Multiple Ciphers in place of plain SQLite.
app/
  src-tauri/         the desktop shell: passes UI requests to the libraries, nothing else
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

- **No network.** `clean` has no HTTP library. The window's content security policy blocks every remote
  connection, and attachments are listed by file name instead of being loaded from Discord's servers.
- **Smallest possible permissions.** The window can only call Sompio's own commands, open file pickers, and hand
  links to messages on the platforms Sompio reads (plus the app-password pages of email providers) to your browser (`app/src-tauri/capabilities/default.json`).
  No shell, no file system access, no HTTP, and no other links.
- **No login.** Sompio never asks for a Discord token or password.
- **One file, deletable.** Data lives in `sompio.db` in the local (non-roaming) app data folder. "Delete everything"
  removes it with SQLite's secure delete and compacts the file so old content isn't left on disk.
- **Masked by default.** Findings are shown as redaction bars until you choose to show them. Every sensitive part
  of a preview is masked, not just the match being listed.
- **No duplicates of your secrets.** Findings and review decisions store positions in the message, not copies
  of the text. "Never flag this again" stores a SHA-256 fingerprint, not the value.
- **Exports hold IDs only.** The flagged-messages CSV has IDs, dates, status and Discord links, no message text.

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

To add a pattern: add a line in `Scanner::new` in `clean/src/scan.rs`, a check in `validate` if it needs one,
and a test in `clean/tests/scanner.rs` that it catches the real thing and ignores lookalikes.

## Roadmap

1. **Now:** offline import, exposure scan, guided review with links into Discord, search, browsing.
2. **Next:** exports (HTML, JSON, CSV) of conversations; encrypt the database with a passphrase (SQLCipher).
3. **Later:** optional clean-up mode that deletes flagged messages through Discord. This needs your token and
   automates your user account, which Discord's terms prohibit. Make it opt-in, slow and very clearly explained.
4. **Then:** Outlook.com (Microsoft sign-in), Reddit live (needs Reddit's API approval), Routines.
