<?php
// Public guide: how to find and delete your old Discord messages.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('How to delete your old Discord messages · Sompio', 'Discord has no button to delete all your messages, and deleting your account leaves them behind. A step-by-step guide to finding what you\'ve shared and getting it removed.');
site_header('guide');
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#why">Why it's hard</a>
      <a href="#step1">1. Get your data</a>
      <a href="#step2">2. Find what matters</a>
      <a href="#step3">3. Delete it</a>
      <a href="#ask">4. Ask Discord to erase it</a>
      <a href="#scripts">About delete-everything scripts</a>
      <a href="#faq">Questions</a>
      <a href="guides.php" class="toc-back">← All guides</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Discord · updated November 2026</p>
      <h1>How to delete your old Discord messages</h1>
      <p class="lead">Years of chats add up. Somewhere in there is probably a password you pasted, a bank number you sent a friend, or your home address. Here's how to find those messages and get rid of them, without risking your account.</p>

      <section id="why">
        <h2>Why it's harder than it should be</h2>
        <ul class="points">
          <li><strong>There's no "delete all" button.</strong> Discord only lets you delete messages one at a time.</li>
          <li><strong>Deleting your account doesn't delete your messages.</strong> They stay in every server and DM, shown under "Deleted User".</li>
          <li><strong>Leaving a server locks you out of your own messages there.</strong> Once you've left, you can't reach them to delete them. Clean up first, leave after.</li>
          <li><strong>You don't remember what you wrote.</strong> Scrolling back through years of chats isn't realistic. You need a list.</li>
        </ul>
      </section>

      <section id="step1">
        <h2>1. Ask Discord for a copy of your data</h2>
        <p>Discord has to give you everything it holds about you, including every message you've sent. This copy is called your <strong>data package</strong>.</p>
        <ol class="guide-steps">
          <li><strong>On a computer:</strong> click the cogwheel at the bottom left to open <strong>User Settings</strong>, then <strong>Data &amp; Privacy</strong>. Scroll to <strong>Request your data</strong> and click <strong>Request Data</strong>.</li>
          <li><strong>On a phone:</strong> tap your picture at the bottom right, then the cogwheel, then <strong>Data &amp; Privacy</strong>, then <strong>Request all of my data</strong>.</li>
          <li>Make sure <strong>Messages</strong> is included, then confirm with <strong>Request My Data</strong>.</li>
        </ol>
        <p>Discord says it can take up to 30 days, but it often arrives much sooner. You get an email with a download link that works for 30 days. Download the zip file and keep it somewhere safe: it holds everything you've ever written on Discord.</p>
        <p class="small muted">More on what's inside: <a href="discord-data-package.php">What's in your Discord data package</a>.</p>
      </section>

      <section id="step2">
        <h2>2. Find the messages that matter</h2>
        <p>You don't need to delete everything. The messages worth deleting are the ones with something personal or dangerous in them. Inside the zip, the <code>messages</code> folder has one folder per channel and DM, with your messages in it. That's fine for a quick look, but not for going through thousands of them.</p>

        <h3>With Sompio Trace</h3>
        <p><a href="docs.php#trace">Trace</a> reads the zip on your own PC and lists what's risky. It never logs into your Discord account and never sends your messages anywhere.</p>
        <ol class="guide-steps">
          <li>In Sompio, open <strong>Trace</strong>, then <strong>Sources</strong>, and add your Discord zip.</li>
          <li>Trace looks for passwords and access tokens, bank and card numbers, addresses, phone numbers, email addresses and more. Everything it finds lands in one list, sorted by risk.</li>
          <li>Add <strong>watch terms</strong> for things no pattern could guess, like your real name, your school or your street.</li>
        </ol>
        <figure class="shot"><img src="img/trace.webp" width="1280" height="780" alt="Trace listing risky items found in sample Discord messages, with advice for each." loading="lazy"></figure>
        <p class="callout"><strong>Deleting doesn't undo a leak.</strong> If a password or card number was out there, others may have seen it already. Change the password, or call your bank, first. Then delete the message.</p>
      </section>

      <section id="step3">
        <h2>3. Delete them, one by one</h2>
        <p>This is the safe way: you delete each message yourself, in Discord, like you always could.</p>
        <ol class="guide-steps">
          <li>In Trace, click <strong>Clean up</strong>. It starts with the most serious findings.</li>
          <li>Click <strong>Open in Discord</strong>. Discord jumps to that exact message.</li>
          <li>Hover over the message, click <strong>⋯</strong> and choose <strong>Delete Message</strong>. Tip: hold <strong>Shift</strong> while you hover, then click the bin icon, and it deletes without asking again.</li>
          <li>Back in Trace, click <strong>I deleted it</strong>, and the next one comes up.</li>
        </ol>
        <p>It's not instant, but most people only have a few dozen messages that really matter, and Trace finds those for you.</p>
      </section>

      <section id="ask">
        <h2>4. Ask Discord to erase what you can't reach</h2>
        <p>Already left a server, or got banned? Under the GDPR in Europe, and similar laws elsewhere, you can ask a company to erase your personal data. Discord has a privacy request form on its support site for this.</p>
        <ul class="points">
          <li>Be specific: say which messages, with their channel and message numbers. Both are in your data package.</li>
          <li>Trace's <strong>Requests</strong> tab writes the letter for you, with the right legal articles, and keeps track of the one-month deadline.</li>
          <li>People's experiences vary: some get messages removed, some get asked for more detail. If there's no proper answer after a month, you can complain to your country's data protection authority, for free. Trace writes that letter too.</li>
        </ul>
      </section>

      <section id="scripts">
        <h2>What about scripts that delete everything at once?</h2>
        <p>There are tools that log in as you and delete messages automatically. They work by automating your user account, which Discord's rules don't allow, and accounts have been banned for it. Some also ask you to paste your login token, which gives full control of your account to whatever runs it.</p>
        <p>Sompio deliberately doesn't do this. It never touches your Discord account; it only reads the copy Discord gave you, and you stay in control of every deletion.</p>
      </section>

      <section id="faq">
        <h2>Questions</h2>
        <dl class="faq">
          <dt>Does Sompio need my Discord password or token?</dt>
          <dd>No. It only reads the data package you downloaded, on your own PC.</dd>
          <dt>Does anything get uploaded?</dt>
          <dd>No. Sompio has no account and no server that holds your data. Everything it finds stays on your PC, encrypted with your lock.</dd>
          <dt>Can I delete messages in DMs?</dt>
          <dd>Yes, your own messages, as long as you can still open the DM. "Open in Discord" works for DMs and group chats too.</dd>
          <dt>Does it work for other apps?</dt>
          <dd>Yes. Trace also reads exports from Reddit, WhatsApp, Telegram, X, Facebook, Instagram, TikTok, Snapchat, LinkedIn and your mailboxes. See the <a href="docs.php#trace">documentation</a>.</dd>
          <dt>Can I use Sompio now?</dt>
          <dd>Sompio is in beta with a small group of testers, and the public release follows soon. Want to help test it? <a href="./#join">Join the beta</a>. Questions? Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>.</dd>
        </dl>
      </section>

<?php more_guides('delete-discord-messages.php'); ?>
    </article>
  </main>
<?php site_footer(); ?>
