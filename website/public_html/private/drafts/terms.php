<?php
// Terms of sale, including refunds. Written for consumers in plain English.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Terms of sale · Sompio', 'The terms for buying a Sompio key: what you get, how keys work, refunds within 14 days, and your rights.');
site_header('terms');
$p = PRICES;
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#who">Who sells</a>
      <a href="#buy">What you buy</a>
      <a href="#licence">Using your key</a>
      <a href="#end">When your time ends</a>
      <a href="#refunds">Refunds</a>
      <a href="#updates">Updates and support</a>
      <a href="#data">Your data</a>
      <a href="#liability">Guarantees and liability</a>
      <a href="#law">Law and changes</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Last updated 13 November 2026</p>
      <h1>Terms of sale</h1>
      <p class="lead">These terms apply when you buy a key for Sompio. We've kept them short and plain. Nothing here takes away the rights consumer law gives you.</p>

      <section id="who">
        <h2>Who sells</h2>
        <p>Sompio is made by Sompio (contact: <a href="mailto:hello@sompio.org">hello@sompio.org</a>). Keys are sold through an online reseller that acts as the seller of record: it handles the payment, VAT, your invoice and refunds. The checkout page names the reseller, and its buyer terms apply to the payment itself. These terms cover Sompio and your key.</p>
      </section>

      <section id="buy">
        <h2>What you buy</h2>
        <p>A key that unlocks all of Sompio (Calendar, Trace and Vault) for a set time:</p>
        <div class="table-wrap">
          <table>
            <tr><th>Plan</th><th>Price</th><th>Unlocks Sompio for</th></tr>
            <tr><td>Month</td><td>€<?= e((string) $p['month']) ?></td><td>31 days</td></tr>
            <tr><td>Year</td><td>€<?= e((string) $p['year']) ?></td><td>365 days</td></tr>
            <tr><td>Forever</td><td>€<?= e((string) $p['forever']) ?></td><td>Without end, including every future version</td></tr>
          </table>
        </div>
        <ul class="points">
          <li>Prices include VAT for buyers in the EU. The checkout shows the exact amount before you pay.</li>
          <li><strong>Nothing renews by itself.</strong> A month or a year is a one-time payment for that time. We never charge you again unless you buy a new key.</li>
          <li>Your key arrives by email right after payment. Add it in Sompio within one year of buying. Its time starts when you add it, or when your current time ends if you still have some.</li>
          <li>Before buying, you can try everything free for 30 days, without a key.</li>
        </ul>
      </section>

      <section id="licence">
        <h2>Using your key</h2>
        <ul class="points">
          <li><strong>One key, one person, all your devices.</strong> You may use your key on every device you own, on every platform Sompio is available for, now and in the future. Where an app store requires purchases to go through its own payment system, we'll do our best to honour keys bought from us, as far as that store's rules allow.</li>
          <li>Don't share, sell or publish your key. A key that appears publicly may be blocked in a later version of Sompio. If that happens to a key you didn't share yourself, write to us and we'll replace it.</li>
          <li>Keys stack: a new key's time starts when your current time ends.</li>
          <li>Sompio's source code is public so anyone can check what it does. Its own license, on GitHub, says what you may do with the code.</li>
        </ul>
      </section>

      <section id="end">
        <h2>When your time ends</h2>
        <p>Sompio reminds you a week before. When the time is up, Sompio becomes read-only: your reminders still pop up, and everything you added, including your passwords, stays readable and can be exported. You can't add or change things until you add a new key. <strong>We never lock you out of your own data.</strong></p>
      </section>

      <section id="refunds">
        <h2>Refunds: 14 days, no questions asked</h2>
        <p>If you're not happy, you can get your money back within <strong>14 days of buying</strong>, for any plan, without giving a reason. Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a> with your order number (it's in your receipt), or use the refund option in the reseller's receipt email. The money goes back the way you paid, usually within a few working days. After a refund, the key stops being valid and is blocked in a later version of Sompio.</p>
        <p class="small muted">About your legal right of withdrawal: for digital content that's delivered straight away, EU and Dutch law let a seller ask you, at checkout, to agree that delivery starts immediately and that you lose the legal 14-day right of withdrawal. Our 14-day refund promise applies regardless, so you keep the same 14 days either way.</p>
        <p>After 14 days, we don't refund the remaining time, but you keep your consumer rights if Sompio doesn't work as described (see below).</p>
      </section>

      <section id="updates">
        <h2>Updates and support</h2>
        <ul class="points">
          <li>While your key is valid, you get every update we release. With Forever, that's every future version. Sompio installs updates only when you click <em>Update now</em>.</li>
          <li>Questions or problems? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>. We're a small team and answer as soon as we can, usually within a few working days.</li>
          <li>Sompio runs on Windows 10 and 11 (64-bit) today. Other platforms follow over time; we don't promise dates.</li>
        </ul>
      </section>

      <section id="data">
        <h2>Your data</h2>
        <p>Everything you put in Sompio stays on your own device, encrypted with your lock. We can't see it, and we can't recover it: if you forget your password and lose your recovery code, nobody can open your data, not even us. Keep your recovery code safe and make backups of what matters to you. How we handle the few details we do receive, like your email address when you buy, is explained in our <a href="privacy.php">privacy statement</a>.</p>
      </section>

      <section id="liability">
        <h2>Guarantees and liability</h2>
        <ul class="points">
          <li>Sompio must work as we describe it. If it doesn't, you have the rights consumer law gives you: we'll fix it, and if we can't, you can get a (partial) refund. This applies on top of the 14-day refund.</li>
          <li>Trace helps you find and remove what you've shared, and writes letters to companies for you. It can't guarantee that every copy is found or that a company deletes everything; that's up to the platforms and companies themselves.</li>
          <li>As far as the law allows, our liability is limited to the amount you paid for your key, except for damage caused by our intent or gross negligence. Nothing in these terms limits rights you have under mandatory consumer law.</li>
        </ul>
      </section>

      <section id="law">
        <h2>Law and changes</h2>
        <ul class="points">
          <li>Dutch law applies. If you're a consumer living elsewhere in the EU, you also keep the protection the law of your own country gives you.</li>
          <li>If we change these terms, the new version applies to new purchases only. Keys you already bought keep what they were sold with.</li>
          <li>Disagree with us about something? Write to us first; we'll do our best to sort it out. You can always go to a court or a dispute body in your country.</li>
        </ul>
      </section>
    </article>
  </main>
<?php site_footer(); ?>
