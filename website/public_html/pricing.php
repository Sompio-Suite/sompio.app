<?php
// Pricing: a month, a year or forever. Keys don't renew by themselves. Prices live in layout.php.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Pricing · Sompio', 'Sompio for a month, a year or forever. One key works on all your own devices, nothing renews by itself, and you can try everything free for 14 days.');
site_header('pricing');
$p = PRICES;
$eur = fn(int $n): string => '€' . $n;
?>
  <main>
    <section class="wrap pricing-head">
      <p class="eyebrow">Pricing</p>
      <h1>Pay for the time you want. Nothing renews by itself.</h1>
      <p class="lead">Every plan unlocks all of Sompio: Calendar, Trace and Vault. One key works on all your own devices. Try everything free for 14 days first, no key needed.</p>
      <p class="note launch-note">Sompio is in beta, so keys aren't on sale yet. <a href="./#join">Join the beta</a> to try it now, or tick "email me at launch" there to hear when they are.</p>
    </section>

    <section class="wrap plans" aria-label="Plans">
      <article class="plan">
        <h2>Month</h2>
        <p class="price"><?= e($eur($p['month'])) ?></p>
        <p class="muted per">for 31 days</p>
        <ul class="ticks">
          <li>Everything unlocked for a month</li>
          <li>All updates during that month</li>
          <li>Good for a one-off clean-up of your old messages</li>
        </ul>
        <span class="button disabled" aria-disabled="true">On sale at launch</span>
      </article>

      <article class="plan featured">
        <p class="plan-badge">Recommended</p>
        <h2>Year</h2>
        <p class="price"><?= e($eur($p['year'])) ?></p>
        <p class="muted per">for 365 days</p>
        <ul class="ticks">
          <li>Everything unlocked for a year</li>
          <li>All updates during that year</li>
          <li>Works out at €<?= e(number_format($p['year'] / 12, 2)) ?> a month</li>
        </ul>
        <span class="button primary disabled" aria-disabled="true">On sale at launch</span>
      </article>

      <article class="plan">
        <h2>Forever</h2>
        <p class="price"><?= e($eur($p['forever'])) ?></p>
        <p class="muted per">once</p>
        <ul class="ticks">
          <li>Everything unlocked, for good</li>
          <li>Every future version and update, at no extra cost</li>
          <li>New platforms included when Sompio arrives there</li>
        </ul>
        <span class="button disabled" aria-disabled="true">On sale at launch</span>
      </article>
    </section>

    <section class="wrap plan-all">
      <h2>Every plan includes</h2>
      <div class="promises">
        <div><h3>All your devices</h3><p>One key per person, on every device you own and every platform Sompio is available for.</p></div>
        <div><h3>No subscription</h3><p>Nothing renews or charges you by itself. Sompio reminds you a week before your time runs out.</p></div>
        <div><h3>Never locked out</h3><p>When your time ends, everything stays readable and exportable, including your passwords. You just can't add or change things until you add a new key.</p></div>
        <div><h3>14 days to change your mind</h3><p>Not happy? Ask for your money back within 14 days of buying, no questions asked.</p></div>
        <div><h3>Works offline</h3><p>Keys are checked on your device. No account, nothing to sign in to.</p></div>
        <div><h3>Prices include VAT</h3><p>For buyers in the EU. What you see is what you pay.</p></div>
      </div>
    </section>

    <section class="wrap docs-body pricing-faq">
      <h2>Questions</h2>
      <dl class="faq">
        <dt>What happens when my month or year ends?</dt>
        <dd>Sompio switches to read-only: your reminders still pop up, and everything you added stays readable and exportable. Add a new key and you can carry on where you left off. A week before, Sompio shows a reminder with a link to get a new key.</dd>
        <dt>Can I buy more time before mine runs out?</dt>
        <dd>Yes. Keys stack: a new key's time starts when your current time ends, so you never lose days.</dd>
        <dt>Can I switch to Forever later?</dt>
        <dd>Yes, any time. Add the Forever key and you're done.</dd>
        <dt>Can I share my key with family or friends?</dt>
        <dd>A key is for one person, on all of their own devices. Everyone else needs their own key.</dd>
        <dt>How do I get my key?</dt>
        <dd>By email, right after you pay. In Sompio, go to Licences, paste it, and that's it.</dd>
        <dt>What does "forever" mean?</dt>
        <dd>You never pay again. Every future version of Sompio is included, for as long as we make it. And because keys are checked on your own device, the version you have keeps working even if we ever stopped.</dd>
        <dt>Where are the full terms?</dt>
        <dd>In our <a href="terms.php">terms of sale</a>, which also explain refunds.</dd>
      </dl>
    </section>
  </main>
<?php site_footer(); ?>
