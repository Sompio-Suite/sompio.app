<?php
// Guide: using GDPR rights to see and erase data companies hold.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('How to make companies delete your data · Sompio', 'The GDPR gives you the right to see and erase what companies hold about you. How to ask, what they must do, how long they have, and what to do if they ignore you.');
site_header('guide');
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#rights">Your rights</a>
      <a href="#who">Who to ask</a>
      <a href="#how">How to ask</a>
      <a href="#deadline">How long they have</a>
      <a href="#ignored">If they ignore you</a>
      <a href="#refuse">When they may say no</a>
      <a href="guides.php" class="toc-back">← All guides</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Your rights · updated November 2026</p>
      <h1>How to make companies delete your data</h1>
      <p class="lead">If you live in the EU (or the company does business there), the GDPR lets you ask any company what it knows about you, and to erase it. It's free, you don't need a lawyer, and companies have to answer.</p>

      <section id="rights">
        <h2>Your rights, in short</h2>
        <ul class="points">
          <li><strong>See what they hold</strong> (Article 15): a copy of your data, what they use it for, where they got it and who they share it with.</li>
          <li><strong>Have it erased</strong> (Article 17), for example when they no longer need it, or when they only had it because you agreed and you withdraw that.</li>
          <li><strong>Make them stop using it</strong> (Article 21), for example for marketing. That one they must always respect.</li>
        </ul>
        <p>All of this is free. A company can only charge, or refuse, if requests are clearly excessive, like asking the same thing every week.</p>
      </section>

      <section id="who">
        <h2>Who to ask</h2>
        <ul class="points">
          <li><strong>Apps and sites you use:</strong> Discord, Reddit, shops, old forums and games you've forgotten about.</li>
          <li><strong>Data brokers:</strong> companies that collect and sell information about people, often without you ever having heard of them. They're worth asking first, because they spread your data furthest.</li>
          <li><strong>Companies that email you</strong> but that you don't remember signing up for. Ask where they got your address.</li>
        </ul>
        <p>Look for the company's privacy contact in its privacy policy, usually an email address like <code>privacy@</code> or <code>dpo@</code>, or a privacy request form.</p>
      </section>

      <section id="how">
        <h2>How to ask</h2>
        <p>A short, clear email is enough. Say:</p>
        <ol class="guide-steps">
          <li>That you're making a request under the GDPR, and which: a copy of your data (Article 15), erasure (Article 17), or both.</li>
          <li>How they can find you: the email address or username on your account. Don't send more than needed, and never a copy of your passport unless they have a real reason to doubt it's you.</li>
          <li>For erasure, what you want deleted, as specifically as you can: your whole account, or particular messages or posts with their links.</li>
          <li>That you expect an answer within one month.</li>
        </ol>
        <p>Keep a copy and write down the date you sent it. <a href="docs.php#trace">Sompio Trace</a> does this part for you: pick a company from about 3,000, choose what you want, and it writes the letter with the right articles, opens it in your own email app, tracks the deadline and reminds you when it's due.</p>
      </section>

      <section id="deadline">
        <h2>How long they have</h2>
        <ul class="points">
          <li><strong>One month</strong> from receiving your request (Article 12).</li>
          <li>For complicated or many requests, they may extend by <strong>two more months</strong>, but they must tell you within the first month, and explain why.</li>
          <li>If they won't do what you asked, they must tell you why, also within one month, and that you can complain.</li>
        </ul>
      </section>

      <section id="ignored">
        <h2>If they ignore you</h2>
        <ol class="guide-steps">
          <li><strong>Send a reminder</strong> once the month has passed, referring to your first request and its date, with a short new deadline, like 14 days.</li>
          <li><strong>Complain to the data protection authority.</strong> In the Netherlands that's the <a href="https://autoriteitpersoonsgegevens.nl" rel="noopener noreferrer">Autoriteit Persoonsgegevens</a>; every EU country has one. It's free, usually done through an online form, and you attach your request and the reminder.</li>
        </ol>
        <p>Trace writes both the reminder and the complaint, filled in with your dates.</p>
      </section>

      <section id="refuse">
        <h2>When they may say no</h2>
        <p>Erasure isn't absolute. A company may keep data it's legally required to hold, like invoices for the tax office, or that it needs for a legal claim. Even then, it must stop using it for anything else, and tell you what it's keeping and why. A copy of your data (Article 15) they must almost always give.</p>
        <p class="small muted">This guide explains the general rules. It isn't legal advice for a particular case.</p>
      </section>

<?php more_guides('make-companies-delete-your-data.php'); ?>
    </article>
  </main>
<?php site_footer(); ?>
