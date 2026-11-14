<?php
// Keys aren't on sale yet. The app's "Get a key" buttons land here. The full pricing page waits in
// private/drafts/pricing.php for launch.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Get a key · Sompio', 'Sompio is in beta. Keys go on sale at launch.', false);
site_header('pricing');
?>
  <main class="wrap pricing-head">
    <p class="eyebrow">Licence keys</p>
    <h1>Keys go on sale at launch</h1>
    <p class="lead">Sompio is in open beta, so there's nothing to buy yet: everything is unlocked, free, until launch. After launch, you get the 30-day free trial on top.</p>
    <ul class="ticks">
      <li>Using an older test version that asks for a key? <a href="download.php">Download the newest beta</a>: it unlocks everything.</li>
      <li>Want to hear when keys are available? <a href="./#join">Get updates by email</a>.</li>
    </ul>
  </main>
<?php site_footer(); ?>
