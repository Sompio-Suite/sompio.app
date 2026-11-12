<?php
// sompio.org: the public front page. The download for testers lives in tester.php.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Sompio · Private tools for your PC', 'Sompio is one program with a calendar, Trace (finds what you\'ve shared online and helps you remove it) and a password vault. No account, no cloud: everything stays on your PC, encrypted.');
site_header('home');
?>
  <main>
    <section class="home-hero">
      <div class="wrap home-hero-row">
        <div class="home-hero-text">
          <p class="status-pill"><span class="dot" aria-hidden="true"></span> In beta · public release coming soon</p>
          <h1>Private tools for your PC. <span class="hl">Nothing leaves it.</span></h1>
          <p class="lead">Sompio is one program with three tools: a calendar, Trace, which finds what you've shared online over the years and helps you remove it, and a password vault. There's no account to make, no cloud, and no server of ours that holds your data.</p>
          <div class="cta-row">
            <a class="button primary" href="docs.php">See how it works</a>
            <a class="button" href="delete-discord-messages.php">Delete your Discord messages</a>
          </div>
        </div>
        <figure class="shot shot-hero">
          <img src="img/trace.webp" width="1280" height="780" alt="Sompio's Trace tool listing passwords, bank numbers and addresses found in sample Discord messages, sorted by risk.">
        </figure>
      </div>
    </section>

    <section id="tools" class="wrap home-tools">
      <h2>Three tools, one lock</h2>
      <p class="muted section-lead">One password and your key picture open everything. When you step away, it's all encrypted again.</p>

      <article class="feature">
        <div class="feature-text">
          <p class="eyebrow">Trace</p>
          <h3>See what you've shared online, then clean it up</h3>
          <p>An old bank number sent to a friend, a password pasted in a chat, your home address in a server. Trace reads the data exports that Discord, Reddit, WhatsApp, Telegram and others must give you, and finds what's risky.</p>
          <ul class="ticks">
            <li>Passwords, bank and card numbers, addresses, phone numbers and more, sorted by risk</li>
            <li>"Clean up" takes you to each message in turn, so you can delete it where you posted it</li>
            <li>Writes the letters that make companies and data brokers delete what they hold about you</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/trace.webp" width="1280" height="780" alt="Trace findings: a table of risky items with advice on what to do for each." loading="lazy"></figure>
      </article>

      <article class="feature flip">
        <div class="feature-text">
          <p class="eyebrow">Vault</p>
          <h3>Your passwords, in a format you already trust</h3>
          <p>Vault keeps your logins in the KeePass format, so you can open them in KeePass or KeePassXC too, and you're never locked in.</p>
          <ul class="ticks">
            <li>Fills logins in Chrome, Edge, Brave and Firefox with the Sompio extension, which talks only to Sompio on your PC</li>
            <li>Health check for weak, reused and old passwords</li>
            <li>Two-factor codes, auto-type and the KeePassXC shortcuts you know</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/vault.webp" width="1280" height="780" alt="Sompio Vault showing a list of sample logins in folders." loading="lazy"></figure>
      </article>

      <article class="feature">
        <div class="feature-text">
          <p class="eyebrow">Calendar</p>
          <h3>Reminders that pop up on time, even when locked</h3>
          <p>For you, or for a whole group: share a schedule file with your team or guild, and everyone gets pop-ups for their own groups, at the right local time.</p>
          <ul class="ticks">
            <li>Pop-ups that don't steal what you're typing</li>
            <li>Keeps running from the tray when the window is closed</li>
            <li>Routines, quiet hours and a choice of sounds</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/calendar.webp" width="1280" height="780" alt="Sompio Calendar showing a personal schedule and a shared raid schedule with groups." loading="lazy"></figure>
      </article>
    </section>

    <section class="band">
      <div class="wrap">
        <h2>Private by design, not by promise</h2>
        <div class="promises">
          <div><h3>No account</h3><p>Nothing to sign up for. Nothing about you sits on a server of ours, so there's nothing to leak.</p></div>
          <div><h3>Works offline</h3><p>Sompio does all its work on your PC. The only thing it fetches on its own is the newest version number, and you can switch that off. <a href="docs.php#private">See every time it goes online</a>.</p></div>
          <div><h3>Encrypted with your lock</h3><p>Your password plus a key picture only you have. Nobody can open it without both, not even us.</p></div>
          <div><h3>Code you can check</h3><p>The full source is <a href="<?= e(SOURCE_URL) ?>" rel="noopener noreferrer">on GitHub</a>, so anyone can see what Sompio does with your data.</p></div>
          <div><h3>Buy once</h3><p>A licence key, like a game key, checked on your PC without going online. No subscription.</p></div>
          <div><h3>Made for Windows</h3><p>Windows 10 and 11, 64-bit. One small program, no extra installs.</p></div>
        </div>
      </div>
    </section>

    <section class="wrap guide-promo">
      <div class="guide-card">
        <div>
          <p class="eyebrow">Free guide</p>
          <h2>Want your old Discord messages gone?</h2>
          <p>Discord has no button to delete all your messages, and deleting your account leaves them behind. Here's what actually works, step by step.</p>
        </div>
        <a class="button primary" href="delete-discord-messages.php">Read the guide</a>
      </div>
    </section>

    <section class="wrap beta-strip">
      <h2>Sompio is in beta</h2>
      <p class="muted">A small group is testing it now. The public release, with licence keys for sale, follows once testing is done. Questions? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>.</p>
      <p class="small muted">Testing Sompio? Go to the <a href="tester.php">tester portal</a> for the download and the feedback form.</p>
    </section>
  </main>
<?php site_footer(); ?>
