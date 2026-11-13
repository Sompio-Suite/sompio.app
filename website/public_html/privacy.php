<?php
// Privacy statement for sompio.org (the website, the beta sign-up, the tester portal and the
// update check). The Sompio app itself is covered in the documentation: it keeps everything on your PC.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Privacy statement · Sompio', 'What sompio.org collects, why, how long it is kept, and how to have it deleted.');
site_header('privacy');
?>
  <main class="wrap docs guide">
    <aside class="docs-toc" aria-label="On this page">
      <p class="toc-title">On this page</p>
      <a href="#short">In short</a>
      <a href="#who">Who we are</a>
      <a href="#what">What we collect</a>
      <a href="#cookies">Cookies</a>
      <a href="#others">Who else handles it</a>
      <a href="#rights">Your rights</a>
      <a href="#app">The Sompio app</a>
      <a href="#changes">Changes</a>
    </aside>

    <article class="docs-body">
      <p class="eyebrow">Last updated 13 November 2026</p>
      <h1>Privacy statement</h1>
      <p class="lead">This page explains what sompio.org collects about you, why, how long it's kept, and how to have it removed. It covers this website, the beta sign-up, the tester portal and Sompio's update check.</p>

      <section id="short">
        <h2>In short</h2>
        <ul class="points">
          <li><strong>We collect as little as we can.</strong> Only what you type into a form yourself, plus what any web server needs to work.</li>
          <li><strong>No tracking.</strong> No analytics, no ads, no tracking pixels, no social media buttons, and nothing loaded from other websites.</li>
          <li><strong>We never sell or share your data</strong> for marketing, and never will.</li>
          <li><strong>You can have it deleted any time</strong> by writing to <a href="mailto:hello@sompio.org">hello@sompio.org</a>.</li>
        </ul>
      </section>

      <section id="who">
        <h2>Who we are</h2>
        <p>Sompio is a small, independent project that makes privacy software for Windows. We're responsible for the personal data described here. For anything about your data, write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>. We aim to answer within a week, and always within the one month the law allows.</p>
      </section>

      <section id="what">
        <h2>What we collect, and why</h2>

        <h3>When you join the beta</h3>
        <ul class="points">
          <li><strong>What:</strong> your email address, the setup answers you pick (Windows version, browsers, apps, the tools you're interested in), your note if you write one, whether you ticked "email me at launch", and the date.</li>
          <li><strong>Why:</strong> to choose testers for each group and send you an invite. If you ticked the box, also to send you one email when Sompio launches.</li>
          <li><strong>Legal basis:</strong> your consent. You can withdraw it at any time by writing to us; we then delete your sign-up.</li>
          <li><strong>How long:</strong> until the beta ends. Then we delete every sign-up, except the email addresses of people who asked to hear about the launch. Those are deleted right after that one launch email.</li>
          <li><strong>Where:</strong> in a file on our web server, and as a notification email in our mailbox.</li>
        </ul>

        <h3>When you send feedback as a tester</h3>
        <ul class="points">
          <li><strong>What:</strong> your message, the part of Sompio it's about, and anything optional you add: a name or nickname, an email address for a reply, bug report details and a screenshot. Bug report details come from Sompio's "Copy details for a bug report" and contain the app's version, your Windows version and recent app messages. They can include your Windows user name as part of a folder path; remove it before sending if you prefer.</li>
          <li><strong>Why:</strong> to find and fix problems, and to reply if you asked us to.</li>
          <li><strong>Legal basis:</strong> our legitimate interest in improving Sompio, which you choose to help with by sending feedback.</li>
          <li><strong>How long:</strong> until the issue is dealt with, and at most six months after the beta ends.</li>
          <li><strong>Where:</strong> in a file on our web server, and as an email in our mailbox.</li>
        </ul>
        <p>Please don't put real passwords, bank details or other sensitive information in feedback or screenshots.</p>

        <h3>When you sign in to the tester portal</h3>
        <p>A cookie keeps you signed in (see <a href="#cookies">Cookies</a>). Nothing else is stored about you when you browse the portal or download Sompio.</p>

        <h3>Protection against abuse</h3>
        <p>To stop password guessing and spam, we briefly count attempts per network. We don't store your internet (IP) address itself, only a scrambled code made from it, and only for 15 minutes (tester passwords) or one hour (beta sign-ups).</p>

        <h3>When Sompio checks for updates</h3>
        <p>Sompio asks sompio.org for one small file with the newest version number, every few hours or when you click <em>Check now</em>. The request contains nothing about you or your data. You can switch this off in Sompio's Security settings.</p>

        <h3>What every web server sees</h3>
        <p>Like any website, sompio.org records each request in a log: the IP address, the time, the page asked for and the browser's self-description. This is needed to run the website and keep it secure. We don't use these logs to track or profile anyone, and we don't keep archives of them: the server discards them within about a day.</p>
        <p>All visits to sompio.org pass through Cloudflare, which protects the site against attacks and overload. To do that, Cloudflare sees the same request details, including your IP address, and keeps them under its own short retention periods.</p>

        <h3>When you buy a key</h3>
        <ul class="points">
          <li><strong>What:</strong> the reseller that handles the payment (named at checkout) passes us your email address, your country, what you bought and the order number. We never see your card or bank details.</li>
          <li><strong>Why:</strong> to send your key, help you if something goes wrong, and handle refunds.</li>
          <li><strong>Legal basis:</strong> it's needed for the purchase (our contract with you), and bookkeeping law.</li>
          <li><strong>How long:</strong> Dutch tax law requires us to keep sales records for seven years. We use them for nothing else, and never for marketing.</li>
        </ul>

        <h3>When you email us</h3>
        <p>We keep your email for as long as needed to help you, and delete conversations that are no longer needed.</p>

        <h3>When you leave a tip</h3>
        <p>Tips go through Ko-fi and PayPal. They handle your payment details under their own privacy policies; we only see what they show the recipient of a payment, such as your name and message if you add one. We use that only to say thank you and for our own bookkeeping.</p>
      </section>

      <section id="cookies">
        <h2>Cookies</h2>
        <p>sompio.org sets one cookie, and only when you sign in to the tester portal: <code>sompio_beta</code>, which keeps you signed in. It's removed when you close your browser, and stops working after 15 minutes without activity. It's strictly necessary for the portal to work, so we don't ask for consent for it. Cloudflare, which protects the site, may also set a short-lived security cookie to tell people from bots. That's all: there are no tracking, advertising or analytics cookies of any kind.</p>
      </section>

      <section id="others">
        <h2>Who else handles your data</h2>
        <p>We use a few services to run sompio.org. They only process data on our behalf, or for their own part of a payment:</p>
        <div class="table-wrap">
          <table>
            <tr><th>Service</th><th>What for</th><th>Where</th></tr>
            <tr><td>Cyberfolks</td><td>Web hosting: the website, its files and server logs</td><td>EU</td></tr>
            <tr><td>Cloudflare</td><td>Protection against attacks; all visits to sompio.org pass through it</td><td>Global; covered by the EU–US Data Privacy Framework</td></tr>
            <tr><td>Proton</td><td>Our mailbox: notification emails and your emails to us</td><td>Switzerland, recognised by the EU as giving equal protection</td></tr>
            <tr><td>Ko-fi and PayPal</td><td>Tips, only if you choose to leave one</td><td>UK and EU</td></tr>
            <tr><td>GitHub</td><td>Hosts Sompio's source code. You only reach it if you click a link to it.</td><td>US</td></tr>
          </table>
        </div>
      </section>

      <section id="rights">
        <h2>Your rights</h2>
        <p>Under the GDPR, you can ask us to:</p>
        <ul class="points">
          <li>show you what we hold about you, and give you a copy;</li>
          <li>correct it;</li>
          <li>delete it;</li>
          <li>stop or limit using it, or withdraw your consent.</li>
        </ul>
        <p>Write to <a href="mailto:hello@sompio.org">hello@sompio.org</a>, from the email address you used with us if you can, so we know it's you. It's free. If you're unhappy with how we handle your data, you can complain to the Dutch data protection authority, the <a href="https://autoriteitpersoonsgegevens.nl" rel="noopener noreferrer">Autoriteit Persoonsgegevens</a>, or the authority in your own country.</p>
      </section>

      <section id="app">
        <h2>The Sompio app</h2>
        <p>The Sompio app doesn't send us your data. There's no account, and everything you put in it stays on your PC, encrypted. The only time it contacts sompio.org on its own is the update check described above. The <a href="docs.php#private">documentation</a> lists every time Sompio goes online, and the <a href="<?= e(SOURCE_URL) ?>" rel="noopener noreferrer">source code</a> lets anyone check.</p>
      </section>

      <section id="changes">
        <h2>Changes to this statement</h2>
        <p>If we change what we collect, we'll update this page and the date at the top. If you've given us your email and the change affects you, we'll tell you before it applies.</p>
      </section>
    </article>
  </main>
<?php site_footer(); ?>
