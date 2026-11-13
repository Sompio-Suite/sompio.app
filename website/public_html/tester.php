<?php
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
start_session();
// Anything waiting because mail was down goes out now
flush_outbox();

$error = null;
if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['password'])) {
    if (!csrf_ok()) {
        $error = 'The page was open too long. Try again.';
    } elseif (too_many_tries()) {
        $error = 'Too many wrong passwords. Wait 15 minutes and try again.';
    } elseif (hash_equals((string) config()['password'], (string) $_POST['password']) && config()['password'] !== 'CHANGE-THIS-PASSWORD') {
        session_regenerate_id(true);
        $_SESSION['pw'] = password_fingerprint();
        $_SESSION['seen'] = time();
        unset($_SESSION['timed_out']);
        // Feedback that was being sent when the 15 minutes ran out goes out now
        $next = isset($_SESSION['pending']) ? 'feedback.php?resume=1' : ($_SESSION['after_login'] ?? 'tester.php');
        unset($_SESSION['after_login']);
        header('Location: ' . $next, true, 303);
        exit;
    } else {
        note_wrong_try();
        usleep(600000);
        $error = 'That password isn\'t right.';
    }
}

$in = signed_in();
$timedOut = !$in && !empty($_SESSION['timed_out']);
$release = $in ? latest_release() : null;
$flash = $_SESSION['flash'] ?? null;
unset($_SESSION['flash']);
$c = config();
?>
<?php page_head($in ? 'Tester portal · Sompio' : 'Tester sign-in · Sompio', 'The Sompio tester portal.', false); ?>
<?php if (!$in): ?>
  <?php site_header('tester'); ?>
  <main class="gate">
    <img src="logo.svg" alt="" width="56" height="56">
    <h1>Tester portal</h1>
    <p class="muted">The test version, install help and the feedback form are here. The <a href="docs.php">documentation</a> and the <a href="delete-discord-messages.php">Discord guide</a> are open to everyone.</p>
    <form method="post" class="card">
      <input type="hidden" name="csrf" value="<?= e(csrf_token()) ?>">
      <label for="password">Tester password</label>
      <input id="password" name="password" type="password" autocomplete="current-password" required autofocus>
      <?php if ($error): ?><p class="error" role="alert"><?= e($error) ?></p>
      <?php elseif (isset($_SESSION['pending'])): ?><p class="note">You were signed out after 15 minutes. Enter the password again and your feedback will be sent.</p>
      <?php elseif ($timedOut): ?><p class="note">You were signed out after 15 minutes without activity. Enter the password to continue.</p><?php endif; ?>
      <button class="primary" type="submit">Open</button>
    </form>
    <p class="small muted">Don't have the password? Ask whoever invited you to test. <a href="./">Back to sompio.org</a></p>
  </main>
<?php else: ?>
  <?php site_header('tester'); ?>
  <nav class="subnav wrap" aria-label="Tester portal">
    <a href="#install">Install</a>
    <a href="#test">What to test</a>
    <a href="#feedback">Feedback</a>
  </nav>

  <main class="wrap">
    <?php if ($flash): ?><p class="<?= $flash['ok'] ? 'success' : 'error' ?>" role="status"><?= e($flash['text']) ?></p><?php endif; ?>

    <div class="hero">
      <div class="hero-intro">
        <h1>Thanks for testing Sompio</h1>
        <p class="lead">Everything you put in Sompio stays on your PC, encrypted. There's no account, and nothing is sent to us. Please use made-up data while testing: the app's Samples page has some for everything.</p>
        <a class="learn" href="docs.php">
          <span class="learn-icon" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5zM4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/></svg></span>
          <span class="learn-text"><strong>How the suite works</strong><span>What Calendar, Trace and Vault do, and how Sompio keeps your data private.</span></span>
          <span class="button learn-button">Read the documentation</span>
        </a>
        <div class="beta-note">
          <h2>Beta: not the final product</h2>
          <p>This is a test version. Things may break, look unfinished, or change before the real release, and some parts will still be improved because of what you tell us. Please use made-up data instead of your real passwords and messages, and don't rely on it for anything important yet.</p>
        </div>
      </div>
      <div class="card download">
        <?php if ($release): ?>
          <p class="small muted">Latest test version</p>
          <h2>Sompio <?= e($release['version']) ?></h2>
          <p class="small muted">Windows 10 and 11, 64-bit · <?= e(human_size($release['size'])) ?> · <?= e(date('j M Y', $release['date'])) ?></p>
          <a class="button primary big" href="download.php">Download for Windows</a>
          <p class="phone-note small">On a phone? Open this page on your Windows PC to install Sompio.</p>
          <div class="fingerprint">
            <p class="fp-label"><svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6z M9 12l2 2 4-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>File fingerprint (SHA-256)</p>
            <code class="hash"><?= e(strtoupper($release['sha256'])) ?></code>
            <p class="small muted">To check your download is really ours, open PowerShell in your Downloads folder and run <code><span class="nowrap">Get-FileHash</span> <span class="nowrap">.\<?= e($release['name']) ?></span></code>. The Hash it shows must match the one above, and the one on <a href="<?= e(SOURCE_URL) ?>/releases/tag/v<?= e($release['version']) ?>" rel="noopener noreferrer">GitHub</a>, which is published separately. This step is optional.</p>
          </div>
        <?php else: ?>
          <h2>Coming soon</h2>
          <p class="muted">The first test version isn't up yet. Check back shortly.</p>
        <?php endif; ?>
      </div>
    <section id="install" class="hero-install">
      <h2>Installing: Windows will warn you, and that's expected</h2>
      <p>This test version isn't signed with a Microsoft certificate yet (the finished version will be). Until then, Windows and your browser don't know Sompio and show warnings. Here's what you'll see and what to click.</p>
      <ol class="steps">
        <li>
          <h3>1. Your browser may hold the download</h3>
          <p><strong>Edge:</strong> "Sompio_… isn't commonly downloaded". Click the <strong>…</strong> next to it, then <strong>Keep</strong>, then <strong>Show more</strong>, then <strong>Keep anyway</strong>.</p>
          <p><strong>Chrome:</strong> click <strong>Keep</strong> (or open the downloads list and choose <strong>Download unverified file</strong>).</p>
          <div class="mock"><span class="mock-icon">!</span><span class="mock-text"><b>Sompio_<?= e($release['version'] ?? '0.0.0') ?>_x64-setup.exe</b> isn't commonly downloaded. Make sure you trust it before you open it.</span><span class="mock-btn">Keep</span><span class="mock-btn">Delete</span></div>
        </li>
        <li>
          <h3>2. "Windows protected your PC"</h3>
          <p>A blue box appears when you open the setup file. Click <strong>More info</strong>, then <strong>Run anyway</strong>.</p>
          <div class="mock blue"><div><b>Windows protected your PC</b><span>Microsoft Defender SmartScreen prevented an unrecognized app from starting. Running this app might put your PC at risk.</span><u>More info</u></div><span class="mock-btn">Run anyway</span><span class="mock-btn">Don't run</span></div>
        </li>
        <li>
          <h3>3. If Microsoft Defender removes the file</h3>
          <p>Rarely, Defender quarantines a new unsigned program by mistake. If that happens, don't turn off your antivirus. Tell us through the form below, so we can send it to Microsoft as a false alarm.</p>
        </li>
        <li>
          <h3>4. Set up the lock</h3>
          <p>Sompio walks you through it: how the lock works, then your key picture, then your password, then a recovery code. Read the first screen: nobody can reset the lock for you, not even us. You get 30 days of everything, no licence key needed.</p>
        </li>
      </ol>
      <p class="small muted">To update later, download the new version here and install it over the old one. Your data stays.</p>
    </section>
    </div>



    <section id="test">
      <h2>What to test</h2>
      <div class="grid3">
        <div class="card">
          <h3>Calendar</h3>
          <ul>
            <li>Samples page: "Add sample reminders". The first pop-up comes within a minute.</li>
            <li>Do pop-ups come on time, without stealing what you're typing?</li>
            <li>Close the window: Sompio stays in the tray and reminders keep coming, even when locked.</li>
          </ul>
        </div>
        <div class="card">
          <h3>Trace <span class="muted small">(was Clean)</span></h3>
          <ul>
            <li>Samples page: "Open Trace with sample data". Findings shows everything found, and "Clean up" goes through it one at a time.</li>
            <li>The new Requests tab: how to get your data from each platform, and letters asking companies to delete it, with deadlines.</li>
            <li>Import the sample exports under Sources, and scan the sample PC folder.</li>
          </ul>
        </div>
        <div class="card">
          <h3>Vault</h3>
          <ul>
            <li>Samples page: "Add sample entries".</li>
            <li>KeePassXC shortcuts: Ctrl+B username, Ctrl+C password, Ctrl+U website, Ctrl+Shift+V types it.</li>
            <li>Auto-type: open Notepad, press Ctrl+Alt+A.</li>
            <li>New: the browser extension. Turn it on in Vault, Settings, and follow the steps there (or in the <a href="docs.php#extension">documentation</a>).</li>
          </ul>
        </div>
      </div>
    </section>

    <section class="support-strip card">
      <div>
        <h2>Enjoying Sompio?</h2>
        <p class="muted">Sompio is made by one person, without ads or investors. If you like the project, a small tip helps keep it going. Completely optional: testing is already a big help.</p>
      </div>
      <a class="button support-button" href="<?= e(SUPPORT_URL) ?>" rel="noopener noreferrer"><?= HEART_ICON ?>Support on Ko-fi</a>
    </section>

    <section id="feedback">
      <h2>Tell us what you found</h2>
      <p>Bugs, confusing bits, ideas, or what you liked. Everything helps.
        <?php if (!empty($c['discord'])): ?>Already in our Discord? You can post there too: <a href="<?= e($c['discord']) ?>" rel="noopener noreferrer">open Discord</a>.<?php endif; ?></p>
      <form method="post" action="feedback.php" enctype="multipart/form-data" class="card feedback">
        <input type="hidden" name="csrf" value="<?= e(csrf_token()) ?>">
        <div class="row2">
          <label>Name or nickname <span class="muted">(optional)</span><input name="name" maxlength="80"></label>
          <label>Email, if you'd like a reply <span class="muted">(optional)</span><input name="contact" type="email" maxlength="120"></label>
        </div>
        <div class="row2">
          <label>Which part?
            <select name="area">
              <option>Calendar</option><option>Trace</option><option>Vault</option>
              <option>Installing or starting</option><option>The lock</option><option>Something else</option>
            </select>
          </label>
          <label>What kind?
            <select name="kind">
              <option>Something didn't work</option><option>Something was confusing</option>
              <option>An idea</option><option>Something I liked</option>
            </select>
          </label>
        </div>
        <label>What did you do, and what happened? <textarea name="message" rows="6" maxlength="8000" required placeholder="I clicked …, and then …"></textarea></label>
        <label>Bug report details <span class="muted">(optional: in Sompio, Security, "Copy details for a bug report", then paste here)</span>
          <textarea name="details" rows="4" maxlength="8000" class="mono"></textarea></label>
        <label>Screenshot <span class="muted">(optional, PNG or JPG up to 5 MB. Windows key + Shift + S takes one. Please don't include real passwords.)</span>
          <input type="file" name="shot" accept="image/png,image/jpeg,image/webp"></label>
        <button class="primary" type="submit">Send feedback</button>
        <p class="small muted">How we handle feedback: see the <a href="privacy.php#what">privacy statement</a>.</p>
      </form>
    </section>
  </main>

<?php endif; ?>
<?php site_footer(); ?>
