<?php
// sompio.org: the public front page. The beta download is on download.php.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Sompio · Private tools for your PC', 'Sompio is one program with a password vault, Trace (finds what you\'ve shared online and helps you remove it) and a calendar. No account, no cloud: everything stays on your PC, encrypted.');
site_header('home');
?>
  <main>
    <section class="home-hero">
      <div class="wrap home-hero-row">
        <div class="home-hero-text">
          <p class="status-pill"><span class="dot" aria-hidden="true"></span> Open beta · free until launch</p>
          <h1>Private tools for your PC. <span class="hl">Nothing leaves it.</span></h1>
          <p class="lead">Sompio is one program with three tools: a password vault, Trace, which finds what you've shared online over the years and helps you remove it, and a calendar. There's no account to make, no cloud, and no server of ours that holds your data.</p>
          <div class="cta-row">
            <a class="button primary" href="download.php">Download the beta</a>
            <a class="button" href="docs.php">See how it works</a>
          </div>
          <p class="small muted">Free for Windows 10 and 11 during the beta. Everything unlocked, no account.</p>
        </div>
        <figure class="shot shot-hero">
          <img src="img/trace.webp" width="1280" height="780" alt="Sompio's Trace tool listing ID numbers, passport numbers, backup codes and bank numbers found in sample chats, sorted by risk.">
        </figure>
      </div>
    </section>

    <section id="tools" class="wrap home-tools">
      <h2>Three tools, one lock</h2>
      <p class="muted section-lead">One password and your key picture open everything. When you step away, it's all encrypted again.</p>

      <article class="feature">
        <div class="feature-text">
          <p class="eyebrow">Vault</p>
          <h3>Your passwords, filled in for you, and warned about</h3>
          <p>Bring in the passwords your browser saved in a minute, and let Sompio fill them in on every site. Vault tells you which ones to change first: weak, reused, found in a data breach, or ones you've pasted into a chat yourself, because Trace found them there.</p>
          <ul class="ticks">
            <li>Fills logins in Chrome, Edge, Brave and Firefox with the Sompio extension, which talks only to Sompio on your PC</li>
            <li>Two-factor codes, auto-type into any app, and keyboard shortcuts for everything</li>
            <li>Never locked in: Vault saves in the open KeePass format, so your passwords open in other apps too</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/vault.webp" width="1280" height="780" alt="Sompio Vault showing a list of sample logins in folders." loading="lazy"></figure>
      </article>

      <article class="feature flip">
        <div class="feature-text">
          <p class="eyebrow">Trace</p>
          <h3>See what you've shared online, then clean it up</h3>
          <p>An old bank number sent to a friend, a password pasted in a chat, a scan of your passport in Downloads. Trace starts with what's quick: folders on your PC, your sent mail, a WhatsApp chat. Meanwhile it helps you ask Discord, Instagram, Reddit and the rest for your data, and reminds you when it's ready.</p>
          <ul class="ticks">
            <li>Finds passwords, bank and card numbers, ID and passport numbers, backup codes, addresses, dates of birth and more, sorted by risk</li>
            <li>Only when you ask: reads text in screenshots and scans, and spots photos that show where you live</li>
            <li>"Clean up" takes you to each message in turn, and writes the letters that make companies delete what they hold about you</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/trace.webp" width="1280" height="780" alt="Trace findings: a table of risky items with advice on what to do for each." loading="lazy"></figure>
      </article>

      <article class="feature">
        <div class="feature-text">
          <p class="eyebrow">Calendar</p>
          <h3>Reminders that pop up on time, even when locked</h3>
          <p>Type "Dentist Tue 15:00" and press Enter. Bring in your phone's calendar from a file. Share a schedule with your team, club or family, and everyone gets pop-ups at the right local time.</p>
          <ul class="ticks">
            <li>Pop-ups that don't steal what you're typing</li>
            <li>Keeps running from the tray when the window is closed</li>
            <li>Routines, quiet hours and a choice of sounds</li>
          </ul>
        </div>
        <figure class="shot"><img src="img/calendar.webp" width="1280" height="780" alt="Sompio Calendar: a reminder typed in one line, a day list, the month and what's next." loading="lazy"></figure>
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
          <div><h3>A key, not an account</h3><p>Sompio is unlocked with a licence key, like a game key, checked on your PC without going online. No sign-up, nothing to log in to.</p></div>
          <div><h3>Made for Windows</h3><p>Windows 10 and 11, 64-bit. One small program, no extra installs.</p></div>
        </div>
      </div>
    </section>

    <section class="wrap guide-promo">
      <div class="guide-card">
        <div>
          <p class="eyebrow">Free guides</p>
          <h2>Take back what you've shared online</h2>
          <p>Delete your old Discord messages and Reddit comments, see what's in your data package, and make companies erase what they hold about you. Step by step, free.</p>
        </div>
        <a class="button primary" href="guides.php">Read the guides</a>
      </div>
    </section>

    <section id="join" class="wrap join">
      <div class="join-intro">
        <p class="eyebrow">Beta</p>
        <h2>Try it now, free</h2>
        <p>The beta is open to everyone, with everything unlocked until launch. When Sompio launches, you get the 30-day free trial on top.</p>
        <ul class="ticks">
          <li><a href="download.php">Download the beta</a> for Windows 10 and 11</li>
          <li>Found a problem or have an idea? <a href="feedback.php">Tell us</a>: it decides what gets fixed first</li>
          <li>New versions come to you inside Sompio</li>
        </ul>
      </div>
      <div class="card join-card">
        <?php if (isset($_GET['joined'])): ?>
          <div class="join-done" role="status">
            <h3>You're on the list</h3>
            <p>Thanks! You'll hear from <strong>hello@sompio.org</strong> when a big new version is out, and once at launch. Nothing else.</p>
          </div>
        <?php else: ?>
          <h3>Get updates by email</h3>
          <p class="small muted">Not on Windows right now, or want to wait for launch? Leave your email.</p>
          <?php $errs = ['email' => 'That email address doesn\'t look right.', 'stale' => 'The form was open too long, or sent too fast. Please try again.', 'limit' => 'Too many sign-ups from your network. Try again in an hour.', 'server' => 'Something went wrong on our side. Please try again later.']; ?>
          <?php if (isset($_GET['join_error'], $errs[$_GET['join_error']])): ?><p class="error" role="alert"><?= e($errs[$_GET['join_error']]) ?></p><?php endif; ?>
          <form method="post" action="signup.php" class="join-form">
            <input type="hidden" name="stamp" value="<?= e(signup_stamp()) ?>">
            <div class="hp" aria-hidden="true"><label>Leave this empty <input name="website" tabindex="-1" autocomplete="off"></label></div>
            <label>Email<input name="email" type="email" maxlength="120" required autocomplete="email"></label>
            <fieldset>
              <legend>Most interested in <span class="muted">(optional)</span></legend>
              <div class="chips">
                <?php foreach (CHOICES['tools'] as $v): ?><label class="chip"><input type="checkbox" name="tools[]" value="<?= e($v) ?>"><span><?= e($v) ?></span></label><?php endforeach; ?>
              </div>
            </fieldset>
            <button class="primary" type="submit">Get updates</button>
            <p class="small muted">Your email is only used for these updates: a few emails at most, and one at launch. It's never shared, and there's no tracking. Want it removed? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>. Details in our <a href="privacy.php">privacy statement</a>.</p>
          </form>
        <?php endif; ?>
      </div>
    </section>
  </main>
<?php site_footer(); ?>
