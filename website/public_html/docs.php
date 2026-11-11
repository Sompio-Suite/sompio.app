<?php
// Documentation: what Sompio is, each tool, and how it keeps data private. Same password as the
// rest of the site; after signing in, testers come straight back here.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

if (!signed_in()) {
    $_SESSION['after_login'] = 'docs.php';
    header('Location: ./', true, 303);
    exit;
}
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="robots" content="noindex, nofollow">
  <title>Documentation · Sompio beta</title>
  <link rel="icon" type="image/svg+xml" href="logo.svg">
  <link rel="stylesheet" href="style.css?v=<?= filemtime(__DIR__ . '/style.css') ?>">
</head>
<body>
  <header class="top">
    <div class="wrap top-row">
      <a class="brand" href="./"><img src="logo.svg" alt="" width="28" height="28"> sompio <span class="tag">beta</span></a>
      <nav>
        <a href="./#install">Install</a>
        <a href="./#test">What to test</a>
        <a href="docs.php" aria-current="page">Documentation</a>
        <a href="./#feedback">Feedback</a>
      </nav>
    </div>
  </header>

  <main class="wrap docs">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#what">What Sompio is</a>
      <a href="#private">Why it's private</a>
      <a href="#lock">The Sompio Lock</a>
      <a href="#calendar">Calendar</a>
      <a href="#clean">Clean</a>
      <a href="#vault">Vault</a>
      <a href="#limits">Good to know</a>
      <a href="./" class="toc-back">← Back to the download</a>
    </aside>

    <article class="docs-body">
      <h1>How Sompio works</h1>

      <section id="what">
        <h2>What Sompio is</h2>
        <p>Sompio is a private toolkit for your Windows PC: a calendar, a cleaner for what you've shared online, and a password vault, in one program. Everything works on your own computer. There is no account to make, no cloud, and no server of ours that holds your data.</p>
        <p>All three tools sit behind one lock, the <strong>Sompio Lock</strong>, so one password (plus your key picture) opens everything and keeps it encrypted when you're away. You buy it once with a licence key, like a game key, and the key is checked on your PC without going online.</p>
        <div class="tools">
          <a class="tool" href="#calendar"><strong>Calendar</strong><span>Reminders that pop up on time, for you or a whole group, even when Sompio is locked.</span></a>
          <a class="tool" href="#clean"><strong>Clean</strong><span>Finds personal information you've shared online (bank numbers, passwords, addresses…) and helps you remove it.</span></a>
          <a class="tool" href="#vault"><strong>Vault</strong><span>Keeps your passwords safe, fills them in for you, and works with KeePass and KeePassXC files.</span></a>
        </div>
      </section>

      <section id="private">
        <h2>Why it's private</h2>
        <p>Sompio is built so your information never has to leave your PC. There is no Sompio server holding your passwords, messages or calendar, so there is nothing of yours on our side that could be hacked, sold or leaked.</p>
        <ul class="points">
          <li><strong>Offline by design.</strong> Every tool works without internet. You can unplug your PC and use all of it.</li>
          <li><strong>Encrypted on your disk.</strong> Your calendar, Clean's findings and your passwords are stored encrypted. Without your password and key picture, the files are unreadable, even to someone who copies them.</li>
          <li><strong>No account, no tracking.</strong> No sign-up, no email address, no analytics, no ads, no "phone home". Sompio doesn't know who you are.</li>
          <li><strong>Pop-ups out of screenshots.</strong> Reminder pop-ups and the auto-type picker are left out of screenshots and screen sharing, so a stream or meeting doesn't show what pops up. The main window can be captured, for help and bug reports. Copied passwords stay out of Windows' clipboard history and cloud clipboard, and clear themselves after 20 seconds.</li>
          <li><strong>Problem reports stay with you.</strong> If something goes wrong, Sompio writes a short note to a file on your PC. It's never sent anywhere; you decide whether to share it.</li>
        </ul>
        <h3>The only times Sompio goes online</h3>
        <p>Each one goes only to the place named, and only what's listed is sent.</p>
        <div class="table-wrap">
          <table>
            <thead><tr><th>When</th><th>Where it goes</th><th>What is sent</th></tr></thead>
            <tbody>
              <tr><td>Vault saves an entry with a website</td><td>That website only, to fetch its little icon</td><td>A normal page request, like opening the site. Nothing about you or your passwords. Can be switched off.</td></tr>
              <tr><td>You click <em>Check for breaches</em> in the Vault</td><td>Have I Been Pwned, the public breach database</td><td>The first 5 characters of a scrambled fingerprint of a password. Never the password itself.</td></tr>
              <tr><td>You connect Clean to your Telegram or email to delete messages there</td><td>Telegram or your email provider</td><td>Only the delete requests you approve.</td></tr>
            </tbody>
          </table>
        </div>
        <p>If you never use those three, Sompio never touches the internet.</p>
      </section>

      <section id="lock">
        <h2>The Sompio Lock</h2>
        <p>One lock protects all three tools, and it needs <strong>two things</strong> to open: your <strong>password</strong> (in your head) and your <strong>key picture</strong> (a file). Someone who finds only one of them gets nothing.</p>
        <ul class="points">
          <li><strong>Key picture.</strong> On first start, Sompio makes a night-sky .png with a random secret hidden in its pixels. You save it wherever you like, and keep a copy off the PC (a USB stick, or your phone as a file).</li>
          <li><strong>Recovery code.</strong> You also get a code to print or write down. It's your way back in if you lose the password or the picture.</li>
          <li><strong>YubiKey (optional).</strong> A YubiKey can stand in for the key picture.</li>
          <li><strong>Auto-lock.</strong> Sompio locks itself after a while without use. Calendar reminders keep coming while it's locked.</li>
        </ul>
        <p class="callout"><strong>Nobody can reset it, not even us.</strong> That's the point: we never have your keys. Lose the password or picture <em>and</em> the recovery code, and the data can't be opened by anyone. Don't edit, crop or send the key picture through chat apps that shrink photos, because that erases the secret.</p>
      </section>

      <section id="calendar">
        <h2>Calendar</h2>
        <p>Calendar makes sure you don't miss things: a small pop-up appears at the right moment, on top of whatever you're doing, without stealing your keyboard while you type or play.</p>
        <ul class="points">
          <li><strong>Schedules and reminders</strong> that repeat daily, weekly or on chosen days, with an optional heads-up a few minutes before.</li>
          <li><strong>Shared schedules for groups.</strong> Share a schedule as a file with your team, guild or club; everyone imports it, and reminders can target only certain groups. Time zones are handled, so a 20:00 event shows at the right local time for everyone.</li>
          <li><strong>Always on.</strong> Sompio sits in the tray and can start with Windows. Reminders keep coming even while Sompio is locked.</li>
          <li><strong>Brings in Desk Reminders</strong> schedules, if you used that app before.</li>
        </ul>
        <p class="privacy-line"><strong>Privacy:</strong> your schedule is stored encrypted on your PC. No calendar account or cloud sync means no company sees your plans, who you meet or where you'll be.</p>
      </section>

      <section id="clean">
        <h2>Clean</h2>
        <p>Clean shows you what personal information you've left behind online over the years, and helps you get rid of it. Most people are surprised: an old bank number sent to a friend, a password pasted in a chat, a home address in a server.</p>
        <p><strong>How it works.</strong> Every big platform lets you download your own data. You request that export, and Clean reads it on your PC. It never logs into your accounts to read them.</p>
        <ul class="points">
          <li><strong>Sources:</strong> Discord, Reddit, Telegram, X, Facebook, Instagram, TikTok, Snapchat, LinkedIn, WhatsApp, Gmail and other mailboxes, plus any folder on your own PC.</li>
          <li><strong>What it finds:</strong> bank accounts (IBAN), payment cards, passwords and access tokens, street addresses, postcodes, phone numbers, email addresses, IP addresses, crypto wallets, and sensitive files on your PC. Add your own <strong>watch terms</strong> too, like your real name, school or street, which no pattern could guess.</li>
          <li><strong>One clear list.</strong> Everything found sits in a table, sorted by risk, with the message it was in and advice on what to do (for a leaked password: change it now; for a card: call your bank).</li>
          <li><strong>Clean it up.</strong> Open each message where it was posted to delete it, or connect Telegram or email to delete straight from Sompio. Files on your PC go to the Recycle Bin. Mark what's fine as "not a problem" and track what you've dealt with.</li>
        </ul>
        <p class="privacy-line"><strong>Privacy:</strong> your exports and everything Clean finds stay encrypted on your PC. Unlike online "privacy scanners", nobody else ever sees your messages.</p>
      </section>

      <section id="vault">
        <h2>Vault</h2>
        <p>Vault is a password manager that keeps every login in one encrypted place on your PC, so you can use a different, strong password everywhere without remembering them. It works like KeePassXC and uses the same open KeePass file format, so you're never locked in.</p>
        <ul class="points">
          <li><strong>Folders, search and a clear table</strong> of all your logins, with the selected entry shown underneath.</li>
          <li><strong>Fill in passwords for you.</strong> Auto-type types your username and password into the login window with one shortcut (Ctrl+Alt+A). Copying works too, with KeePassXC's shortcuts (Ctrl+B username, Ctrl+C password).</li>
          <li><strong>2FA codes</strong> for sites that use authenticator apps, right next to the password.</li>
          <li><strong>Password generator</strong> for strong passwords or easy-to-type passphrases.</li>
          <li><strong>Health check</strong> that spots weak, reused, old and expired passwords, and an optional breach check (see <a href="#private">Why it's private</a>).</li>
          <li><strong>Extras:</strong> earlier versions of each entry, attachments, a recycle bin, website icons, and import from KeePass, KeePassXC, Bitwarden, 1Password, Chrome and Firefox. Export to KeePass any time.</li>
        </ul>
        <p class="privacy-line"><strong>Privacy:</strong> unlike cloud password managers, your vault is never uploaded or synced to a company's servers. It's one encrypted file on your PC, opened only by your lock.</p>
      </section>

      <section id="limits">
        <h2>Good to know</h2>
        <p>Privacy tools should be honest about their limits, so here are Sompio's:</p>
        <ul class="points">
          <li><strong>Your PC is the vault.</strong> While Sompio is unlocked, it's only as safe as the PC it runs on. No app can fully protect you from malware or someone using your unlocked computer, so keep Windows updated and lock Sompio when you step away.</li>
          <li><strong>Clean finds, you decide.</strong> It spots patterns and your watch terms, but it can miss things and occasionally flag something harmless. Deleting a message doesn't undo that others may have seen it, which is why it tells you to change leaked passwords too.</li>
          <li><strong>Backups are your job.</strong> With no cloud copy, keep your key picture and recovery code safe, and export your vault now and then.</li>
        </ul>
        <h3>Questions</h3>
        <dl class="faq">
          <dt>Does Sompio work without internet?</dt><dd>Yes, everything. Only the three optional actions listed under <a href="#private">Why it's private</a> go online.</dd>
          <dt>Can you see my passwords or messages?</dt><dd>No. They never leave your PC, and we don't have your keys.</dd>
          <dt>What if I forget my password?</dt><dd>Use your recovery code. Without it, nobody can open your data, including us.</dd>
          <dt>Which systems?</dt><dd>Windows 10 and 11 today. Linux, Mac and phones are planned.</dd>
        </dl>
      </section>

      <div class="docs-end">
        <a class="button primary" href="./">Back to the download</a>
        <a class="button" href="./#feedback">Send feedback</a>
      </div>
    </article>
  </main>

  <footer class="wrap small muted">
    This page has no trackers or ads. Its only cookie keeps you signed in. Feedback goes straight to the Sompio team and isn't shared.
  </footer>
</body>
</html>
