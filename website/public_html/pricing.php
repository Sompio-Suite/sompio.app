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
    <p class="lead">Sompio is in beta, so there's nothing to buy yet. Testers use it free for the whole beta.</p>
    <ul class="ticks">
      <li>Already testing and your time is running out? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a> and we'll send you a new tester key.</li>
      <li>Want to hear when keys are available? <a href="./#join">Join the beta</a> and tick "Also email me once when Sompio launches".</li>
    </ul>
  </main>
<?php site_footer(); ?>
