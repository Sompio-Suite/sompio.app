<?php
// The open beta: anyone can download the newest setup file. The file sits in private/, which the
// web server never serves directly; download.php?get=1 hands it out, a few times per network per
// hour, so nobody can run up the site's traffic.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

$r = latest_release();

if (isset($_GET['get'])) {
    if ($r === null) {
        http_response_code(404);
        exit('No beta version is up yet. Please check back soon.');
    }
    if (over_net_limit('download', 10, 3600)) {
        http_response_code(429);
        header('Retry-After: 3600');
        exit('Too many downloads from your network in the last hour. Please try again later.');
    }
    @ini_set('zlib.output_compression', 'Off');
    while (ob_get_level()) {
        ob_end_clean();
    }
    header('Content-Type: application/octet-stream');
    header('Content-Disposition: attachment; filename="' . $r['name'] . '"');
    header('Content-Length: ' . $r['size']);
    header('X-Content-Type-Options: nosniff');
    header('Cache-Control: private, no-store');
    readfile($r['path']);
    exit;
}

page_head('Download the beta · Sompio', 'Download the free Sompio beta for Windows, Mac and Linux: a password vault, Trace and a calendar, all offline and encrypted on your computer.');
site_header('download');
?>
  <main class="wrap">
    <div class="hero">
      <div class="hero-intro">
        <p class="status-pill"><span class="dot" aria-hidden="true"></span> Open beta · free until launch</p>
        <h1>Download the Sompio beta</h1>
        <p class="lead">Everything is unlocked during the beta, free, no key or account needed. When Sompio launches, you get the 30-day free trial on top.</p>
        <ul class="ticks">
          <li>Vault, Trace and Calendar in one program</li>
          <li>No account and no cloud: your data stays on your computer, encrypted</li>
          <li>New versions come to you inside Sompio</li>
        </ul>
        <div class="beta-note">
          <h2>It's a beta</h2>
          <p>Sompio works, but things can still break or change. Two habits keep you safe: in Vault, save a copy of your passwords now and then (<em>Auto-type, import and export</em>, then <em>Save as KeePass file</em>), and keep the recovery sheet Sompio gives you when you set up the lock. Nobody can reset the lock for you, not even us.</p>
          <p>Found a problem, or have an idea? <a href="feedback.php">Tell us</a>. It decides what gets fixed first.</p>
        </div>
      </div>
      <div class="card download">
        <?php if ($r): $pl = $r['platforms']; $hasMac = isset($pl['mac']); $hasLinux = isset($pl['linux_appimage']) || isset($pl['linux_deb']); ?>
          <h2>Sompio <?= e($r['version']) ?></h2>
          <?php if ($hasMac || $hasLinux): ?>
            <div class="os-tabs" role="tablist" aria-label="Your system">
              <a href="#for-windows" class="os-tab" data-os="windows" role="tab">Windows</a>
              <?php if ($hasMac): ?><a href="#for-mac" class="os-tab" data-os="mac" role="tab">Mac</a><?php endif; ?>
              <?php if ($hasLinux): ?><a href="#for-linux" class="os-tab" data-os="linux" role="tab">Linux</a><?php endif; ?>
            </div>
          <?php endif; ?>

          <div class="os-panel" id="for-windows" data-os="windows">
            <p class="small muted">Windows 10 and 11, 64-bit · <?= e(human_size($r['size'])) ?> · <?= e(date('j M Y', $r['date'])) ?></p>
            <a class="button primary big" href="download.php?get=1" rel="nofollow">Download for Windows</a>
            <div class="fingerprint">
              <p class="fp-label"><?= SHIELD_ICON ?>File fingerprint (SHA-256)</p>
              <code class="hash"><?= e(strtoupper($r['sha256'])) ?></code>
              <p class="small muted">To check: run <code class="nowrap">Get-FileHash .\<?= e($r['name']) ?></code> in PowerShell. It must match this and <a href="<?= e(SOURCE_URL) ?>/releases/tag/v<?= e($r['version']) ?>" rel="noopener noreferrer">GitHub</a>.</p>
            </div>
          </div>

          <?php if ($hasMac): $m = $pl['mac']; ?>
          <div class="os-panel" id="for-mac" data-os="mac">
            <p class="small muted">macOS 11 or newer, Apple silicon and Intel · <?= e(human_size($m['size'])) ?></p>
            <a class="button primary big" href="<?= e($m['url']) ?>" rel="nofollow noopener noreferrer">Download for Mac</a>
            <div class="fingerprint">
              <p class="fp-label"><?= SHIELD_ICON ?>File fingerprint (SHA-256)</p>
              <code class="hash"><?= e(strtoupper($m['sha256'])) ?></code>
              <p class="small muted">To check: run <code class="nowrap">shasum -a 256</code> on the file in Terminal. It must match this and <a href="<?= e(SOURCE_URL) ?>/releases/tag/v<?= e($r['version']) ?>" rel="noopener noreferrer">GitHub</a>.</p>
            </div>
          </div>
          <?php endif; ?>

          <?php if ($hasLinux): ?>
          <div class="os-panel" id="for-linux" data-os="linux">
            <p class="small muted">64-bit Linux, like Ubuntu, Debian or Fedora<?= isset($pl['linux_appimage']) ? ' · ' . e(human_size($pl['linux_appimage']['size'])) : '' ?></p>
            <?php if (isset($pl['linux_appimage'])): $f = $pl['linux_appimage']; ?>
              <a class="button primary big" href="<?= e($f['url']) ?>" rel="nofollow noopener noreferrer">Download for Linux (AppImage)</a>
            <?php elseif (isset($pl['linux_deb'])): $f = $pl['linux_deb']; ?>
              <a class="button primary big" href="<?= e($f['url']) ?>" rel="nofollow noopener noreferrer">Download for Ubuntu and Debian (.deb)</a>
            <?php endif; ?>
            <div class="fingerprint">
              <p class="fp-label"><?= SHIELD_ICON ?>File fingerprint (SHA-256)</p>
              <?php if (isset($pl['linux_appimage'])): ?><code class="hash"><?= e(strtoupper($pl['linux_appimage']['sha256'])) ?></code><?php endif; ?>
              <p class="small muted">To check: run <code>sha256sum</code> on the file. It must match this and <a href="<?= e(SOURCE_URL) ?>/releases/tag/v<?= e($r['version']) ?>" rel="noopener noreferrer">GitHub</a>.<?php if (isset($pl['linux_deb'])): ?> Prefer a package? Get the <a href="<?= e($pl['linux_deb']['url']) ?>" rel="nofollow noopener noreferrer">.deb for Ubuntu and Debian</a> (<?= e(human_size($pl['linux_deb']['size'])) ?>); its fingerprint is on GitHub.<?php endif; ?></p>
            </div>
          </div>
          <?php endif; ?>
          <p class="phone-note small">On a phone? Open this page on your computer to install Sompio.</p>
          <p class="card-foot small"><a href="<?= e(SOURCE_URL) ?>/releases/tag/v<?= e($r['version']) ?>" rel="noopener noreferrer">What's new</a> · <a href="tor.php">Download through Tor</a></p>
        <?php else: ?>
          <h2>Coming soon</h2>
          <p class="muted">The beta isn't up yet. Check back shortly.</p>
        <?php endif; ?>
      </div>
    </div>

    <section id="install" class="hero-install">
      <div class="os-panel" data-os="windows">
        <h2>Installing on Windows: it will warn you, and that's expected</h2>
        <p>The beta isn't signed with a Microsoft certificate yet (the finished version will be). Until then, Windows and your browser don't know Sompio and show warnings. Here's what you'll see and what to click.</p>
        <ol class="steps">
          <li>
            <h3>1. Your browser may hold the download</h3>
            <p><strong>Edge:</strong> "Sompio_… isn't commonly downloaded". Click the <strong>…</strong> next to it, then <strong>Keep</strong>, then <strong>Show more</strong>, then <strong>Keep anyway</strong>.</p>
            <p><strong>Chrome:</strong> click <strong>Keep</strong> (or open the downloads list and choose <strong>Download unverified file</strong>).</p>
            <div class="mock"><span class="mock-icon">!</span><span class="mock-text"><b>Sompio_<?= e($r['version'] ?? '1.0.0') ?>_x64-setup.exe</b> isn't commonly downloaded. Make sure you trust it before you open it.</span><span class="mock-btn">Keep</span><span class="mock-btn">Delete</span></div>
          </li>
          <li>
            <h3>2. "Windows protected your PC"</h3>
            <p>A blue box appears when you open the setup file. Click <strong>More info</strong>, then <strong>Run anyway</strong>.</p>
            <div class="mock blue"><div><b>Windows protected your PC</b><span>Microsoft Defender SmartScreen prevented an unrecognized app from starting. Running this app might put your computer at risk.</span><u>More info</u></div><span class="mock-btn">Run anyway</span><span class="mock-btn">Don't run</span></div>
          </li>
          <li>
            <h3>3. If Microsoft Defender removes the file</h3>
            <p>Rarely, Defender quarantines a new unsigned program by mistake. If that happens, don't turn off your antivirus. <a href="feedback.php">Tell us</a>, so we can send it to Microsoft as a false alarm.</p>
          </li>
          <li>
            <h3>4. Set up the lock</h3>
            <p>Sompio walks you through it: how the lock works, then your key picture, then your password, then a recovery code. Read the first screen: nobody can reset the lock for you, not even us.</p>
          </li>
        </ol>
      </div>

      <div class="os-panel" data-os="mac">
        <h2>Installing on a Mac: macOS will stop it once, and that's expected</h2>
        <p>The beta isn't notarized by Apple yet (the finished version will be), so macOS doesn't know Sompio and blocks it the first time. You allow it once.</p>
        <ol class="steps">
          <li>
            <h3>1. Move it to Applications</h3>
            <p>Open the downloaded <strong>.dmg</strong> file and drag <strong>Sompio</strong> onto the <strong>Applications</strong> folder.</p>
          </li>
          <li>
            <h3>2. Open it, and let macOS block it</h3>
            <p>Open Sompio from Applications. macOS says it couldn't verify Sompio. Click <strong>Done</strong> (not Move to Bin).</p>
          </li>
          <li>
            <h3>3. Allow it in System Settings</h3>
            <p>Open <strong>System Settings</strong>, then <strong>Privacy &amp; Security</strong>. Scroll down to "Sompio was blocked" and click <strong>Open Anyway</strong>, then confirm with your password or Touch ID. From then on, Sompio opens normally.</p>
            <p class="small muted">On macOS 14 or older: right-click Sompio in Applications, choose <strong>Open</strong>, then <strong>Open</strong> again.</p>
          </li>
          <li>
            <h3>4. Set up the lock</h3>
            <p>Sompio walks you through it. Read the first screen: nobody can reset the lock for you, not even us.</p>
          </li>
        </ol>
        <p class="small muted">Not on the Mac yet: auto-type into other apps, and reading text in pictures in Trace. Both are coming.</p>
      </div>

      <div class="os-panel" data-os="linux">
        <h2>Installing on Linux</h2>
        <ol class="steps">
          <li>
            <h3>AppImage (most systems)</h3>
            <p>Make the file runnable: right-click it, <strong>Properties</strong>, <strong>Allow executing file as program</strong>, or run <code>chmod +x Sompio_*.AppImage</code>. Then double-click it. On Ubuntu 22.04 and newer you may first need <code>sudo apt install libfuse2</code> (on 24.04: <code>libfuse2t64</code>). New versions come to you inside Sompio.</p>
          </li>
          <li>
            <h3>.deb (Ubuntu, Debian)</h3>
            <p>Run <code>sudo apt install ./Sompio_*_linux_amd64.deb</code> in the folder you downloaded it to. Sompio tells you about new versions; install those the same way.</p>
          </li>
          <li>
            <h3>Tails</h3>
            <p>Being tested. Use the AppImage, kept in your Persistent Storage folder. Sompio's data lives in your home folder, which Tails wipes at shutdown unless the <strong>Dotfiles</strong> feature of Persistent Storage is on. Tried it? <a href="feedback.php">Tell us</a> how it went.</p>
          </li>
        </ol>
        <p class="small muted">Not on Linux yet: auto-type into other apps, and reading text in pictures in Trace.</p>
      </div>
      <p class="small muted">New versions come to you inside Sompio: a pop-up offers them, and your data stays as it is.</p>
    </section>

    <section id="start">
      <h2>Where to start</h2>
      <div class="grid3">
        <div class="card">
          <h3>Vault</h3>
          <ul>
            <li>Bring in the passwords your browser saved: Vault shows you how, step by step.</li>
            <li>Turn on the browser extension in Vault's settings, so logins fill themselves in.</li>
            <li>Run the health check: weak, reused and breached passwords, and ones you've shared in your own chats.</li>
          </ul>
        </div>
        <div class="card">
          <h3>Trace</h3>
          <ul>
            <li>Start with what's quick: folders on your computer, your mailbox, a WhatsApp chat.</li>
            <li>Ask Discord, Instagram and the rest for your data at the same time; Sompio reminds you when it should be ready.</li>
            <li>Want to look first? The Samples page has a made-up person to try everything on.</li>
          </ul>
        </div>
        <div class="card">
          <h3>Calendar</h3>
          <ul>
            <li>Type a reminder in one line: "Dentist Tue 15:00", then Enter.</li>
            <li>Bring in your phone's calendar from a file, under More.</li>
            <li>Close the window: Sompio stays by the clock, and reminders still pop up.</li>
          </ul>
        </div>
      </div>
      <p class="muted">More in the <a href="docs.php">documentation</a>.</p>
    </section>

    <section class="support-strip card">
      <div>
        <h2>Enjoying Sompio?</h2>
        <p class="muted">Sompio is made by one person, without ads or investors. If you like it, a small tip helps keep it going. Completely optional.</p>
      </div>
      <a class="button support-button" href="<?= e(SUPPORT_URL) ?>" rel="noopener noreferrer"><?= HEART_ICON ?>Support on Ko-fi</a>
    </section>
  </main>
<script src="os.js?v=1" defer></script>
<?php site_footer(); ?>
