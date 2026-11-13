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
            <a class="button primary" href="#join">Join the beta</a>
            <a class="button" href="docs.php">See how it works</a>
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
          <div><h3>No subscription</h3><p>A key for a month, a year or forever, checked on your PC without going online. Nothing renews by itself. <a href="pricing.php">See pricing</a>.</p></div>
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

    <section id="join" class="wrap join">
      <div class="join-intro">
        <p class="eyebrow">Beta</p>
        <h2>Join the beta</h2>
        <p>A small group is testing Sompio before the public release. Want to help? Tell us a little about your setup, and we'll invite testers in small groups, picked so every kind of PC and app gets covered.</p>
        <ul class="ticks">
          <li>You get the full app, free, for the whole beta</li>
          <li>Your feedback decides what gets fixed first</li>
          <li>Already invited? Go to the <a href="tester.php">tester portal</a></li>
        </ul>
      </div>
      <div class="card join-card">
        <?php if (isset($_GET['joined'])): ?>
          <div class="join-done" role="status">
            <h3>You're on the list</h3>
            <p>Thanks! If you're picked for the next group, an invite with the download comes from <strong>hello@sompio.org</strong>. Nothing else will be sent.</p>
          </div>
        <?php else: ?>
          <?php $errs = ['email' => 'That email address doesn\'t look right.', 'stale' => 'The form was open too long, or sent too fast. Please try again.', 'limit' => 'Too many sign-ups from your network. Try again in an hour.', 'server' => 'Something went wrong on our side. Please try again later.']; ?>
          <?php if (isset($_GET['join_error'], $errs[$_GET['join_error']])): ?><p class="error" role="alert"><?= e($errs[$_GET['join_error']]) ?></p><?php endif; ?>
          <form method="post" action="signup.php" class="join-form">
            <input type="hidden" name="stamp" value="<?= e(signup_stamp()) ?>">
            <div class="hp" aria-hidden="true"><label>Leave this empty <input name="website" tabindex="-1" autocomplete="off"></label></div>
            <label>Email<input name="email" type="email" maxlength="120" required autocomplete="email"></label>
            <fieldset>
              <legend>Your Windows</legend>
              <div class="chips">
                <?php foreach (CHOICES['windows'] as $i => $v): ?><label class="chip"><input type="radio" name="windows" value="<?= e($v) ?>"<?= $i === 0 ? ' checked' : '' ?>><span><?= e($v) ?></span></label><?php endforeach; ?>
              </div>
            </fieldset>
            <fieldset>
              <legend>Browsers you use</legend>
              <div class="chips">
                <?php foreach (CHOICES['browsers'] as $v): ?><label class="chip"><input type="checkbox" name="browsers[]" value="<?= e($v) ?>"><span><?= e($v) ?></span></label><?php endforeach; ?>
              </div>
            </fieldset>
            <fieldset>
              <legend>Apps you'd check with Trace</legend>
              <div class="chips">
                <?php foreach (CHOICES['apps'] as $v): ?><label class="chip"><input type="checkbox" name="apps[]" value="<?= e($v) ?>"><span><?= e($v) ?></span></label><?php endforeach; ?>
              </div>
            </fieldset>
            <fieldset>
              <legend>Most interested in</legend>
              <div class="chips">
                <?php foreach (CHOICES['tools'] as $v): ?><label class="chip"><input type="checkbox" name="tools[]" value="<?= e($v) ?>"><span><?= e($v) ?></span></label><?php endforeach; ?>
              </div>
            </fieldset>
            <label>Anything else? <span class="muted">(optional)</span><textarea name="note" rows="2" maxlength="500" placeholder="For example: I have a huge Discord export, or I use a YubiKey"></textarea></label>
            <label class="check"><input type="checkbox" name="launch" value="1"><span>Also email me once when Sompio launches</span></label>
            <button class="primary" type="submit">Join the beta</button>
            <p class="small muted">Your email is only used to send you a beta invite, and is deleted when the beta ends. If you tick the box, you get one more email at launch, and that's it. It's kept on sompio.org, never shared, and there's no tracking. Want it removed sooner? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>. Details in our <a href="privacy.php">privacy statement</a>.</p>
          </form>
        <?php endif; ?>
      </div>
    </section>
  </main>
<?php site_footer(); ?>
