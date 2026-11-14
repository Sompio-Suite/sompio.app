<?php
// Guide: what's in your Discord data package.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head("What's in your Discord data package · Sompio", 'Your Discord data package holds every message you ever sent, and more. How to request it, what each part means, and what to look for.');
site_header('guide');
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#what">What it is</a>
      <a href="#request">How to get it</a>
      <a href="#inside">What's inside</a>
      <a href="#look">What to look for</a>
      <a href="#safe">Keep it safe</a>
      <a href="guides.php" class="toc-back">← All guides</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Discord · updated November 2026</p>
      <h1>What's in your Discord data package</h1>
      <p class="lead">Discord has to give you a copy of everything it holds about you. That copy, the data package, is the best way to see what you've written over the years, and the starting point for cleaning it up.</p>

      <section id="what">
        <h2>What it is</h2>
        <p>Under privacy laws like the GDPR, you have the right to a copy of your personal data. Discord provides it as a zip file: your account details, the servers you're in, and every message you've sent in servers, DMs and group chats, with the date of each.</p>
      </section>

      <section id="request">
        <h2>How to get it</h2>
        <ol class="guide-steps">
          <li><strong>On a computer:</strong> open <strong>User Settings</strong> (the cogwheel at the bottom left), then <strong>Data &amp; Privacy</strong>. Under <strong>Request your data</strong>, click <strong>Request Data</strong>.</li>
          <li><strong>On a phone:</strong> tap your picture, then the cogwheel, then <strong>Data &amp; Privacy</strong>, then <strong>Request all of my data</strong>.</li>
          <li>Choose what to include. Make sure <strong>Messages</strong> is ticked, then confirm.</li>
        </ol>
        <p>Discord says it can take up to 30 days, but it often comes much sooner. It arrives by email, with a download link that works for 30 days.</p>
      </section>

      <section id="inside">
        <h2>What's inside</h2>
        <p>The part most people care about is the <code>messages</code> folder:</p>
        <ul class="points">
          <li><strong>One folder per conversation.</strong> Every server channel, DM and group chat you've written in gets its own folder, named with that channel's number (its ID).</li>
          <li><strong>A small file saying where it is:</strong> for server channels, the server and channel name; for DMs, who it was with (as user IDs).</li>
          <li><strong>Your messages:</strong> each one with its ID, the date and time, the text, and links to anything you attached.</li>
          <li><strong>An index</strong> that maps every channel number to its name, so you can tell which folder is which.</li>
        </ul>
        <p>Only <em>your</em> messages are in it, not what others wrote. Depending on what you ticked, there are also folders about your account and the servers you've joined. Support conversations with Discord aren't included.</p>
        <p class="small muted">Discord's own description: <a href="https://support.discord.com/hc/en-us/articles/360004957991" rel="noopener noreferrer">Your Discord data package</a>.</p>
      </section>

      <section id="look">
        <h2>What to look for</h2>
        <p>Most messages are harmless. The ones worth deleting are those with something that could be used against you:</p>
        <ul class="points">
          <li>Passwords, login codes and access tokens you pasted "just for a second"</li>
          <li>Bank account and card numbers sent to a friend</li>
          <li>Your home address, phone number or personal email</li>
          <li>Your real name, school or workplace, if you'd rather they weren't linked to your username</li>
        </ul>
        <p>Searching thousands of messages by hand isn't realistic. <a href="docs.php#trace">Sompio Trace</a> reads the zip on your own computer, lists what's risky sorted by danger, and takes you to each message so you can delete it. Nothing is uploaded. The step-by-step is in <a href="delete-discord-messages.php">How to delete your old Discord messages</a>.</p>
      </section>

      <section id="safe">
        <h2>Keep it safe</h2>
        <p>The zip holds everything you've ever written on Discord, so treat it like a diary. Don't upload it to websites that offer to "analyse" it, and don't share it. Keep it on your own computer, and delete it when you're done.</p>
      </section>

<?php more_guides('discord-data-package.php'); ?>
    </article>
  </main>
<?php site_footer(); ?>
