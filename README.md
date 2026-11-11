# Sompio Suite

Private tools that keep your data on your PC, unlocked with a key instead of an account.

| App | Status |
| --- | --- |
| **Clean** | Built: finds and cleans up personal information in your exports and files (below) |
| **Calendar** | Built: Desk Reminders rebuilt, encrypted, with privacy routines (below) |
| **Vault** | Built (round 1 of 3): KeePass-compatible password manager with auto-type (below) |
| **Voice** | On hold: taken out of the app in 0.7.0. The code is kept in `voice/` (below) |

The plan lives in the "Sompio Suite plan" doc.

## Sompio Calendar

Desk Reminders, rebuilt into the suite. Schedules use the same file format (`Days,Time,Colour,Title,Text,Notice,For`),
so an existing `schedule.csv` and shared schedules open as they are, but inside Sompio they're kept in `calendar.db`,
encrypted with the lock like everything else.

- **Day, month, Next up:** today's reminders with how many are done, a month with dots for the special days (raids,
  check-ups, one-offs), and what's coming. Edit, skip one time, mute for a day or always, or remove.
- **Routines:** Privacy check-ups (monthly password check-up, quarterly "request fresh data exports", sign out of old
  sessions, check your recovery sheet), Backups, and Money and subscriptions. Desk Reminders' old wellness templates
  (water, eye breaks…) sit folded away under "Everyday reminders" and aren't promoted.
- **Schedules:** bring in a whole Desk Reminders folder (your schedule, `shared/`, colours, groups, quiet hours, sound),
  add a shared schedule file, or start one you organise and save it as a file to share. Groups (Tanks, Healers…)
  decide who gets a pop-up.
- **Pop-ups:** a small window in the corner you choose that never takes the keyboard, with your sound, Done and snooze
  5/15 min. They're kept out of screen shares and streams (Windows 10 2020+), and a discreet mode hides the text.
- **Keeps running:** closing the window keeps Sompio in the tray. Reminders for the next 7 days stay in memory, so they
  keep coming while Sompio is locked. After a restart, unlock once to start them. "Start with Windows" is in the
  Calendar settings.

Code: `calendar/` (format, engine, store, pop-up queue; all tested) and `app/src-tauri/src/calendar.rs` (commands,
the reminder loop, the pop-up window and the tray).

## Sompio Vault

A password manager that keeps everything in a real KeePass file (`vault.kdbx`, KDBX 4), so you're never locked in.
Inside Sompio the file opens with a key from the lock; "Save as KeePass file" makes a copy with a password of your
choosing that KeePassXC, KeePass, KeeWeb and Strongbox open. Checked against KeePassXC 2.7.6 in both directions.

Round 1 (built):
- Folders, entries with title, username, password, website, notes, tags, expiry and your own extra fields (hidden or
  not), recycle bin, the last 10 versions of every entry with "use this version", search.
- 2FA codes (TOTP) from a setup key or otpauth:// link, as KeePassXC stores them.
- Generator: passwords (length, character kinds, no look-alikes, excluded characters) and EFF-word passphrases, with
  zxcvbn strength.
- Health check: weak, reused, expired and old passwords, scored, offline.
- Import: KeePass/KeePassXC files (password and/or key file), CSV from Bitwarden, Chrome/Edge/Brave, Firefox,
  1Password, LastPass, Proton Pass, Dashlane, KeePassXC. Export: KeePass file, CSV.
- Copy with auto-clear (default 20 s), kept out of Windows clipboard history and cloud clipboard.
- Auto-type: a global shortcut (default Ctrl+Alt+A) types into the window in front, matched by window rules, the
  entry title or the site name; a chooser when several fit. KeePass sequences ({USERNAME}{TAB}{PASSWORD}{ENTER},
  {TOTP}, {S:Field}, {DELAY n}, {TAB n}, ^a…). "Type it" on an entry types into the window behind Sompio.
- Reading, copying, typing, deleting and exporting keep working when a key runs out; adding and changing need one.

Round 2 (built so far):
- Files inside entries (up to 25 MB each), kept in the KeePass file, so KeePassXC sees them too.
- Merge changes from a copy: export, use it in KeePassXC elsewhere, merge back. Entries match by ID, the newer
  version wins and the older goes into history (checked with real KeePassXC edits).
- Breach check: Have I Been Pwned's range search. Only 5 characters of each password's SHA-1 are sent, only when
  the person presses the button.

Still to come in round 2: browser fill (aiming for the KeePassXC-Browser extension's protocol) and YubiKey.
Round 3: SSH agent, passkeys, Windows Hello quick unlock.

## Sompio Voice (on hold)

Not in the app since 0.7.0, while how it fits is decided. Everything is kept: the server, client and protocol
in `voice/` (still built and tested on their own), and the app's parts in `voice/parked-app/` (the Voice
screen, the audio engine, its worklet and mock, and the shell's `voice.rs`). To bring it back: move those
files back to `app/src/`, `app/public/` and `app/src-tauri/src/`, add `sompio-voice` to the app's Cargo.toml,
and register the module, commands, shortcut handler and exit hook in `lib.rs` again (see git history, 0.6.0).

Self-hosted voice and chat with channels, like TeamSpeak (`voice/`).

- **Server** (`sompio-voice-server`, also hostable from the app): channel tree with sub-channels, channel
  passwords and user limits, temporary channels (close when empty) and lasting ones, server password, roles
  (admin, moderator, member) given with one-time admin keys like TeamSpeak's privilege keys, move, kick from
  channel or server, ban, poke, channel/server/private chat, server name and welcome. Kept in one JSON file.
  Linux: `voice/linux/install.sh` installs it as a systemd service (Debian 12 / Ubuntu 22.04 or newer).
- **Connecting**: by IP or domain (an A record), default UDP port 9987. QUIC with TLS 1.3; the server's
  self-signed certificate is pinned the first time (people compare the fingerprint the server prints), and a
  changed one is refused with a warning. Each person has a separate identity key per server, derived from their
  lock, so servers can't link them.
- **Audio**: in the window with WebCodecs: Opus at 48 kHz in 20 ms frames (~50 kbit/s), with the browser
  engine's echo cancellation, noise suppression and gain control; voice activation with a level meter, or
  push-to-talk on a global key (works while a game has focus); per-person volume; a small jitter buffer.
- **Hosting from the app**: the server runs while Sompio is open. Sompio asks the router to open the port with
  UPnP (`voice/src/upnp.rs`) and says plainly what happened: open to everyone (with the address to send), no
  router answered, the router refused, or the connection is shared (carrier-grade NAT, 100.64.0.0/10, where no
  port can be opened at home). The mapping is closed again when the server stops or Sompio quits. The
  always-on option is the Linux server on a VPS.
- Joining servers is free; hosting from the app needs Voice on the key.

The server stores only channels, roles and bans: no chat history and no recordings.

Not yet: end-to-end encryption (today the server relays audio and chat, encrypted to and from it, so whoever runs
the server could in principle listen in, as with TeamSpeak), file transfer, and per-channel codec settings.

## YubiKey

A YubiKey can stand in for the key picture (Security, YubiKey). It uses HMAC-SHA1 challenge-response in slot 2
(or 1), like KeePassXC, so a key already set up for KeePassXC works as is. Sompio talks to the key's OTP
interface with HID feature reports (`hwkey/`), which Windows allows without admin rights or a driver. The key's
answer to a stored random challenge opens a copy of the picture's secret, which then works with the password as
usual, so password changes keep working and several keys can be added (add a spare). Recovery makes a new
picture, so keys are added again after it. The protocol code is tested against a simulated YubiKey; it hasn't
met a real one in this repository's tests.

## Look and layout

White and compact, in the spirit of KeePassXC: a light navigation pane, flat underlined tabs, a toolbar,
a tree on the left, a sortable table and a preview pane underneath. Clean's Findings and the Vault both use it
(`.xc-*` styles in `styles.css`). The Vault has KeePassXC's shortcuts (Ctrl+B/C/T/U, Ctrl+Shift+V, Ctrl+E/K/N/F,
Enter, Delete), column sorting, clone and drag-to-folder.

First run: welcome, the rules of the lock (do's and don'ts), "Generate my key picture (.png)" (saved and read
back to check it), the password, then the recovery code (`Vault::new_picture` and `setup_with_picture`).

## Testing and problem reports

`How to test Sompio.txt` is the guide for testers; Build Sompio.bat puts it in `Sompio for Windows\For testers`
with the setup file (and without the Key Maker). Problems are written to `problems.log` in the app's data folder
(`%LOCALAPPDATA%\app.sompio.desktop`): crashes in the Rust side (panic hook), errors and crashed screens in the
window (a crashed screen shows "Reload Sompio" instead of an empty window). Error messages only, never data;
rotated at 512 KB; nothing is sent anywhere. Security, "Something wrong?" copies the version, Windows version and
last log lines for a bug report.

## Samples

The Samples page (sidebar) has made-up data for every function: Clean's sample data, sample exports for all
12 import types plus a folder for the PC scan, Calendar reminders that pop up within a minute plus a shared
schedule and a Desk Reminders folder, Vault entries covering every feature plus KeePass files (one needs a key
file; password `sample-password`), five CSV formats and a local login page for auto-type. Files are written to
Documents\Sompio samples. Every sample is covered by a test that imports it.

Code: `vault/` (store, generator, health, CSV, auto-type parsing and matching; all tested) and
`app/src-tauri/src/vault.rs` (commands, clipboard, global shortcut, keystrokes).

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

**Updates.** Installed copies check `latest.json` on the newest GitHub release every few hours (it can be turned
off under Security) and show **Update now** in the sidebar. Updates are signed; the app refuses anything not signed
with your update key. Set it up once with `Set up updates.bat`: it makes the key, writes its public half and the
download address into `tauri.release.conf.json` (used only by the release workflow), and shows you the private half to paste into the GitHub secret
`TAURI_SIGNING_PRIVATE_KEY`. Releases must be public for the app to fetch them. To keep the code private, make a
separate public repository for releases, set the Actions variables `RELEASES_OWNER` and `RELEASES_REPO` and a
`RELEASES_TOKEN` secret (a token that can write to that repository); the workflow publishes there instead.

**Local builds.** `Build Sompio.bat` keeps compiled pieces in `%LOCALAPPDATA%\Sompio Build`, so a new copy of the
project only rebuilds what changed. Local builds don't use GitHub or `tauri.release.conf.json` at all: updates
are simply off in them (Security says so), and you get a new version by running the script again.

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
calendar/          Sompio Calendar: the Desk Reminders format, the reminder engine, the encrypted store.
vault/             Sompio Vault: KeePass files, generator, health check, CSV, auto-type sequences.
voice/             Sompio Voice: the server, the client connection, the protocol; linux/ has the installer.
hwkey/             YubiKey challenge-response over HID.
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
