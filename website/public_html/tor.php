<?php
// "Open in Tor": how to visit sompio.org through Tor Browser, and its onion address once there is one.
// A web page can't start Tor Browser by itself, so this page gives the address to open there.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
$onion = onion_address();
page_head('Open in Tor · Sompio', 'Visit sompio.org privately through the Tor network with Tor Browser.');
site_header('tor');
?>
  <main class="wrap narrow">
    <p class="eyebrow"><?= TOR_ICON ?> Tor</p>
    <h1>Open sompio.org in Tor Browser</h1>
    <p class="lead">Tor Browser sends your visit through the Tor network, so websites, including this one, can't see where you are. Sompio doesn't need it, but if you'd rather visit this way, here's how.</p>

    <?php if ($onion): ?>
      <section class="card tor-card">
        <h2>Our onion address</h2>
        <p>This address works only in Tor Browser, and keeps your whole visit inside the Tor network.</p>
        <code class="hash"><?= e($onion) ?></code>
        <div class="cta-row">
          <a class="button primary" href="http://<?= e($onion) ?>/" rel="noopener noreferrer">Open the onion address</a>
        </div>
        <p class="small muted">Already in Tor Browser? It also shows <strong>.onion available</strong> in the address bar on every page of sompio.org.</p>
      </section>
    <?php endif; ?>

    <section>
      <h2>How to do it</h2>
      <ol class="steps">
        <li>
          <h3>1. Get Tor Browser</h3>
          <p>Download it from the Tor Project's own website, <strong>torproject.org</strong>, and nowhere else. It's free, and runs on Windows, Mac, Linux and Android.</p>
        </li>
        <li>
          <h3>2. Open the address there</h3>
          <p>Start Tor Browser, click <strong>Connect</strong>, then type <code><?= e($onion ?? 'sompio.org') ?></code> in its address bar. A normal browser can't hand a page over to Tor Browser, so this step is always by hand.</p>
        </li>
        <li>
          <h3>3. Download Sompio as usual</h3>
          <p>The download page works the same in Tor Browser. The file is identical, so its fingerprint matches the one on GitHub.</p>
        </li>
      </ol>
    </section>

    <section>
      <h2>Good to know</h2>
      <ul class="points">
        <li>Sompio itself works offline. It only goes online to check for new versions (you can switch that off), and when you connect your mailbox or Telegram yourself.</li>
        <li>Using Tails? See the Linux steps on the <a href="download.php#for-linux">download page</a>.</li>
      </ul>
    </section>
  </main>
<?php site_footer(); ?>
