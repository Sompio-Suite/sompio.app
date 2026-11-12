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
      <a href="#trace">Trace</a>
      <a href="#vault">Vault</a>
      <a href="#extension">Browser extension</a>
      <a href="#limits">Good to know</a>
      <a href="./" class="toc-back">← Back to the download</a>
    </aside>

    <article class="docs-body">
      <h1>How Sompio works</h1>

      <section id="what">
        <h2>What Sompio is</h2>
        <p>Sompio is a private toolkit for your Windows PC: a calendar, a tool that finds and removes what you've shared online, and a password vault, in one program. Everything works on your own computer. There is no account to make, no cloud, and no server of ours that holds your data.</p>
        <p>All three tools sit behind one lock, the <strong>Sompio Lock</strong>, so one password (plus your key picture) opens everything and keeps it encrypted when you're away. You buy it once with a licence key, like a game key, and the key is checked on your PC without going online.</p>
        <div class="tools">
          <a class="tool" href="#calendar"><strong>Calendar</strong><span>Reminders that pop up on time, for you or a whole group, even when Sompio is locked.</span></a>
          <a class="tool" href="#trace"><strong>Trace</strong><span>Finds personal information you've shared online (bank numbers, passwords, addresses…), helps you remove it, and gets companies to delete what they hold.</span></a>
          <a class="tool" href="#vault"><strong>Vault</strong><span>Keeps your passwords safe, fills them in for you (also in your browser, with the Sompio extension), and works with KeePass and KeePassXC files.</span></a>
        </div>
      </section>

      <section id="private">
        <h2>Why it's private</h2>
        <p>Sompio is built so your information never has to leave your PC. There is no Sompio server holding your passwords, messages or calendar, so there is nothing of yours on our side that could be hacked, sold or leaked.</p>
        <ul class="points">
          <li><strong>Offline by design.</strong> Every tool works without internet. You can unplug your PC and use all of it.</li>
          <li><strong>Encrypted on your disk.</strong> Your calendar, Trace's findings and requests, and your passwords are stored encrypted. Without your password and key picture, the files are unreadable, even to someone who copies them.</li>
          <li><strong>No account, no tracking.</strong> No sign-up, no email address, no analytics, no ads, no "phone home". Sompio doesn't know who you are.</li>
          <li><strong>Pop-ups out of screenshots.</strong> Reminder pop-ups and the auto-type picker are left out of screenshots and screen sharing, so a stream or meeting doesn't show what pops up. The main window can be captured, for help and bug reports. Copied passwords stay out of Windows' clipboard history and cloud clipboard, and clear themselves after 20 seconds.</li>
          <li><strong>The browser extension stays local too.</strong> It talks only to Sompio on your PC, never to a server, and only after you allow your browser in Sompio. See <a href="#extension">Browser extension</a>.</li>
          <li><strong>Problem reports stay with you.</strong> If something goes wrong, Sompio writes a short note to a file on your PC. It's never sent anywhere; you decide whether to share it.</li>
        </ul>
        <h3>The only times Sompio goes online</h3>
        <p>Each one goes only to the place named, and only what's listed is sent.</p>
        <div class="table-wrap">
          <table>
            <thead><tr><th>When</th><th>Where it goes</th><th>What is sent</th></tr></thead>
            <tbody>
              <tr><td>Vault saves an entry with a website</td><td>That website only, to fetch its little icon</td><td>A normal page request, like opening the site. Nothing about you or your passwords. Can be switched off.</td></tr>
              <tr><td>Sompio checks for updates (every few hours, or when you click <em>Check now</em>; can be switched off in Security)</td><td>sompio.org</td><td>A request for one small file with the newest version number. Nothing about you or your data. Updates install only when you click <em>Update now</em>, and only if they're signed by us.</td></tr>
              <tr><td>You click <em>Check for breaches</em> in the Vault</td><td>Have I Been Pwned, the public breach database</td><td>The first 5 characters of a scrambled fingerprint of a password. Never the password itself.</td></tr>
              <tr><td>You connect Trace to your Telegram or email</td><td>Telegram or your email provider</td><td>Your login, to read the messages you sent and delete the ones you choose. Nothing is uploaded anywhere else.</td></tr>
              <tr><td>You click a search, a download page or a company's form in Trace</td><td>That website, in your own browser</td><td>Only what you click. The site sees what you search for, as if you typed it there yourself.</td></tr>
            </tbody>
          </table>
        </div>
        <p>If you never use these, Sompio never touches the internet. Letters to companies open in <em>your own</em> email app, ready for you to send; Sompio never sends mail itself.</p>
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

      <section id="trace">
        <h2>Trace</h2>
        <p>Trace shows you what personal information you've left behind online over the years, and helps you get rid of it. Most people are surprised: an old bank number sent to a friend, a password pasted in a chat, a home address in a server. Trace used to be called Clean.</p>
        <p><strong>How it works.</strong> Every big platform must let you download your own data. You ask for that export, and Trace reads it on your PC. It never logs into your accounts to read them. Telegram and email can also be connected, for results in minutes.</p>
        <ul class="points">
          <li><strong>Sources:</strong> Discord, Reddit, Telegram, X, Facebook, Instagram, TikTok, Snapchat, LinkedIn, WhatsApp, Gmail and other mailboxes, plus any folder on your own PC.</li>
          <li><strong>What it finds:</strong> bank accounts (IBAN), payment cards, passwords and access tokens, street addresses, postcodes, phone numbers, email addresses, IP addresses, crypto wallets, and sensitive files on your PC. Add your own <strong>watch terms</strong> too, like your real name, school or street, which no pattern could guess.</li>
          <li><strong>One clear list.</strong> Everything found sits in a table, sorted by risk, with the message it was in and advice on what to do (for a leaked password: change it now; for a card: call your bank).</li>
          <li><strong>Clean it up, one by one.</strong> "Clean up" takes you through everything in turn: it opens each message where it was posted, you delete it there, and move on to the next. Connected Telegram and email messages are deleted straight from Sompio, and files on your PC go to the Recycle Bin.</li>
        </ul>
        <h3>Requests: get your data, and get it deleted</h3>
        <p>Under the GDPR, companies must give you your data, delete it, or stop using it when you ask, and answer within one month. Trace's <strong>Requests</strong> tab does the paperwork for you:</p>
        <ul class="points">
          <li><strong>Get your data.</strong> Where and how to ask each platform for your export, how long it usually takes, and a Calendar reminder for when it should be ready.</li>
          <li><strong>Find it now.</strong> Don't want to wait? Ready-made searches for your watch terms on Discord, Reddit and X, limited to your own posts.</li>
          <li><strong>Ask a company.</strong> Search about 3,000 companies, data brokers included, and pick what you want: delete my data, send me a copy, or stop using it. Trace writes the letter with the right legal articles, and opens it in your own email app or the company's form.</li>
          <li><strong>Track the deadline.</strong> Every request gets its one-month deadline and a Calendar reminder. No answer? Trace writes the reminder, and then a complaint to your country's data protection authority, which handles it for free.</li>
        </ul>
        <p class="privacy-line"><strong>Privacy:</strong> your exports, everything Trace finds and your requests stay encrypted on your PC. Unlike online "privacy scanners" and paid removal services, nobody else ever sees your messages or the list of companies you deal with. The company directory is built in, so searching it never goes online.</p>
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

      <section id="extension">
        <h2>Browser extension</h2>
        <p>The Sompio extension brings your Vault into Chrome, Edge, Brave and Firefox, the way password managers like Proton Pass or Bitwarden do: a small Sompio button in login fields, your logins one click away, and an offer to save new ones. The difference: there is no Sompio account or server. The extension talks only to Sompio on your own PC.</p>
        <ul class="points">
          <li><strong>Fill with one click.</strong> Click the Sompio button in a login field and pick a login. Only logins saved for that website are shown.</li>
          <li><strong>Save and update.</strong> After you sign in somewhere new, or change a password, Sompio offers to save it. Choose "Never for this site" to stop asking there.</li>
          <li><strong>Strong passwords when you sign up.</strong> In a "new password" field, "Use a strong password" makes one with Sompio's generator and fills both fields.</li>
          <li><strong>2FA codes</strong> for the website, filled into the code field.</li>
          <li><strong>The toolbar button</strong> shows the logins for the page you're on, searches your vault, copies usernames and passwords, and makes passwords.</li>
        </ul>
        <h3>How it stays private</h3>
        <ul class="points">
          <li><strong>No internet, no account.</strong> The browser starts a small part of Sompio on your PC, which passes messages to the Sompio app. Nothing goes over the network.</li>
          <li><strong>You allow each browser once.</strong> When you click "Connect to Sompio", Sompio asks you to allow it and shows a code that must match the one in your browser. You can remove a browser any time in Vault, Settings.</li>
          <li><strong>Encrypted between the browser and Sompio.</strong> Each allowed browser gets its own secret key, kept inside your encrypted vault. Every request is encrypted with it, and an old request can't be sent again.</li>
          <li><strong>Only for the right website.</strong> Sompio checks the website the browser itself reports, not what the page claims, and refuses to fill a login on a different or look-alike website (like <em>example.com.evil.net</em>).</li>
          <li><strong>Nothing while locked.</strong> When Sompio is locked or closed, the extension can't get anything, and offers to open Sompio so you can unlock it.</li>
          <li><strong>Off until you turn it on.</strong> The extension does nothing until you turn it on in Sompio (Vault, Settings).</li>
        </ul>
        <h3>Setting it up (test version)</h3>
        <p>While Sompio is in testing, the extension isn't in the browser stores yet, so you add it by hand. Sompio installs its files for you.</p>
        <ol class="points">
          <li>In Sompio, open <strong>Vault</strong>, then <strong>Settings</strong> (the "Auto-type, import and export" tab), and turn on <strong>Let the Sompio extension use my vault</strong>. Copy the folder it shows.</li>
          <li><strong>Chrome, Edge or Brave:</strong> open <code>chrome://extensions</code> (Edge: <code>edge://extensions</code>), turn on <em>Developer mode</em>, click <em>Load unpacked</em>, and choose that folder.<br><strong>Firefox:</strong> open <code>about:debugging</code>, choose <em>This Firefox</em>, then <em>Load Temporary Add-on</em>, and pick <code>manifest.json</code> in that folder. Firefox removes it when it closes, until the extension is published.</li>
          <li>Restart the browser. Click the Sompio button in the toolbar (pin it from the puzzle-piece menu), then <strong>Connect to Sompio</strong>, and click <strong>Allow</strong> in Sompio when the codes match.</li>
        </ol>
        <p class="privacy-line"><strong>Privacy:</strong> the extension can see the pages you visit, as every password extension must, to find login fields. It keeps nothing about them and sends nothing anywhere: it only asks Sompio, on your PC, for logins for the page you're on.</p>
      </section>

      <section id="limits">
        <h2>Good to know</h2>
        <p>Privacy tools should be honest about their limits, so here are Sompio's:</p>
        <ul class="points">
          <li><strong>Your PC is the vault.</strong> While Sompio is unlocked, it's only as safe as the PC it runs on. No app can fully protect you from malware or someone using your unlocked computer, so keep Windows updated and lock Sompio when you step away.</li>
          <li><strong>Trace finds, you decide.</strong> It spots patterns and your watch terms, but it can miss things and occasionally flag something harmless. Deleting a message doesn't undo that others may have seen it, which is why it tells you to change leaked passwords too. Companies sometimes answer late or partly; Trace helps you follow up, but it can't force them.</li>
          <li><strong>Backups are your job.</strong> With no cloud copy, keep your key picture and recovery code safe, and export your vault now and then.</li>
        </ul>
        <h3>Questions</h3>
        <dl class="faq">
          <dt>Does Sompio work without internet?</dt><dd>Yes, everything. Only the optional actions listed under <a href="#private">Why it's private</a> go online.</dd>
          <dt>Can you see my passwords or messages?</dt><dd>No. They never leave your PC, and we don't have your keys.</dd>
          <dt>What if I forget my password?</dt><dd>Use your recovery code. Without it, nobody can open your data, including us.</dd>
          <dt>Does the browser extension upload my passwords?</dt><dd>No. It has no server to send them to. It asks Sompio on your PC for one login when you click it, encrypted, and only for the website you're on.</dd>
          <dt>Which systems?</dt><dd>Windows 10 and 11 today. Linux, Mac and phones are planned.</dd>
          <dt>Does Trace send emails for me?</dt><dd>No. It writes the letter and opens it in your own email app, so you send it from your own address and the answer comes back to you.</dd>
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
