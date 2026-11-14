<?php
// Guide: deleting old Reddit posts and comments.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('How to delete your old Reddit posts and comments · Sompio', 'Find the Reddit posts and comments that give away too much, delete them properly, and know what deleting your account does and doesn\'t do.');
site_header('guide');
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#why">Before you start</a>
      <a href="#step1">1. Get your data</a>
      <a href="#step2">2. Find what matters</a>
      <a href="#step3">3. Delete it</a>
      <a href="#account">Deleting your account</a>
      <a href="#copies">Copies elsewhere</a>
      <a href="guides.php" class="toc-back">← All guides</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Reddit · updated November 2026</p>
      <h1>How to delete your old Reddit posts and comments</h1>
      <p class="lead">An anonymous username feels safe, but years of comments add up: your city, your job, your age, a photo of your street. Here's how to find the posts that give too much away and remove them.</p>

      <section id="why">
        <h2>Before you start</h2>
        <ul class="points">
          <li><strong>Deleting your account doesn't delete your posts.</strong> They stay on Reddit, shown as written by "[deleted]". Delete what matters first, then the account if you want.</li>
          <li><strong>It's rarely one comment that gives you away.</strong> It's the combination: "I'm a nurse", "here in Utrecht", "my daughter's school". Look for those details as well as obvious things like phone numbers.</li>
        </ul>
      </section>

      <section id="step1">
        <h2>1. Ask Reddit for a copy of your data</h2>
        <ol class="guide-steps">
          <li>On reddit.com, signed in, click your profile picture at the top right, then <strong>Settings</strong>.</li>
          <li>Open <strong>Privacy &amp; Security</strong> (on some versions, <strong>Privacy</strong>), scroll to <strong>Request a copy of your data</strong>, and click <strong>Request</strong>.</li>
          <li>Reddit emails you a download link when it's ready, often within hours. Download the zip before the link expires.</li>
        </ol>
        <p>The zip holds spreadsheets (CSV files) of everything you've done: <code>posts.csv</code> and <code>comments.csv</code> with the text and a link to each, plus your messages, saved posts and the communities you joined. Menus change over time; if you can't find the option, Reddit's help centre has the current path.</p>
      </section>

      <section id="step2">
        <h2>2. Find what matters</h2>
        <p>You can open <code>comments.csv</code> in Excel and search for your street, city or employer. That works for a few hundred comments, but not for ten years of them.</p>
        <p><a href="docs.php#trace">Sompio Trace</a> reads the Reddit zip on your own computer and finds addresses, phone numbers, email addresses, bank details and more. Add <strong>watch terms</strong> for the personal details no pattern can guess, like your town, school or job. Everything found lands in one list, sorted by risk, and nothing is uploaded.</p>
      </section>

      <section id="step3">
        <h2>3. Delete them, one by one</h2>
        <ol class="guide-steps">
          <li>In Trace, click <strong>Clean up</strong>, then <strong>Open on Reddit</strong>. Reddit opens that exact post or comment.</li>
          <li>Click <strong>⋯</strong> under it and choose <strong>Delete</strong>.</li>
          <li>Back in Trace, click <strong>I deleted it</strong> and move on to the next.</li>
        </ol>
        <p>Without Trace, use the link in the <code>permalink</code> column of the spreadsheet to reach each one.</p>
        <p>Can't delete something, for example because the community was banned or you were removed from it? You can ask Reddit to erase it under the GDPR; see <a href="make-companies-delete-your-data.php">How to make companies delete your data</a>.</p>
      </section>

      <section id="account">
        <h2>Deleting your account</h2>
        <p>Once the posts that matter are gone, you can delete the account in <strong>Settings</strong>, at the bottom of the account page. Remember: what you didn't delete yourself stays visible, just without your name on it. And a deleted username can't be used again, by you or anyone else.</p>
      </section>

      <section id="copies">
        <h2>What about copies elsewhere?</h2>
        <p>Public Reddit posts can be copied by archive sites, search engines and data collectors, and Reddit itself licenses posts to other companies. Deleting on Reddit removes the original, and that's the most important step. Copies elsewhere are harder: for search engines, you can ask them to remove outdated pages, and for companies that hold your data, the GDPR guide shows how to ask them to erase it.</p>
      </section>

<?php more_guides('delete-reddit-comments.php'); ?>
    </article>
  </main>
<?php site_footer(); ?>
