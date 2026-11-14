<?php
// Security: how to report a problem, and how Sompio protects your data, in plain words.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Security · Sompio', 'How Sompio encrypts your data, what it can and can\'t protect you from, and how to report a security problem.');
site_header('security');
?>
  <main class="wrap narrow">
    <p class="eyebrow">Security</p>
    <h1>How Sompio keeps your data safe</h1>
    <p class="lead">Everything Sompio holds stays on your computer, encrypted. Here's how, what it can't protect you from, and how to tell us if you find a problem.</p>

    <section id="report">
      <h2>Found a security problem?</h2>
      <p>Write to <a href="mailto:hello@sompio.org?subject=Security">hello@sompio.org</a> with "Security" in the subject. Tell us what you found and how to see it happen. We'll reply within a few days, keep you posted while we fix it, and thank you by name here if you'd like. Please give us a fair chance to fix it before you make it public.</p>
      <p class="small muted">Sompio is a small, independent project, so there's no paid bug bounty yet.</p>
    </section>

    <section id="how">
      <h2>How your data is protected</h2>
      <ul class="points">
        <li><strong>One lock, two parts.</strong> Sompio opens with your password and your key picture. Your password goes through Argon2id, a method built to make guessing slow and expensive. The key picture holds a random secret. Neither is ever sent anywhere.</li>
        <li><strong>Strong, standard encryption.</strong> The keys to your data are sealed with XChaCha20-Poly1305. Your passwords are kept in the open KeePass (KDBX 4) format; Trace's and Calendar's data in encrypted databases (ChaCha20-Poly1305).</li>
        <li><strong>A recovery code</strong>, printed once on your recovery sheet, opens the lock if you lose the picture or forget your password. Nobody else can reset the lock, not even us.</li>
        <li><strong>Nothing on our servers.</strong> There's no account and no cloud copy. sompio.org only hands out new versions; it never sees your data.</li>
        <li><strong>Signed updates.</strong> Every update is signed, and Sompio refuses one that isn't ours. Each download's fingerprint is also published on <a href="<?= e(SOURCE_URL) ?>/releases" rel="noopener noreferrer">GitHub</a>, separately from this site.</li>
        <li><strong>Open source.</strong> The full code is <a href="<?= e(SOURCE_URL) ?>" rel="noopener noreferrer">on GitHub</a>, so anyone can check what it does.</li>
      </ul>
    </section>

    <section id="limits">
      <h2>What it can't protect you from</h2>
      <ul class="points">
        <li><strong>A computer that's already compromised.</strong> While Sompio is unlocked, malware or someone at your keyboard can see what you see. Keep your system updated, and lock Sompio when you step away (it does so by itself after a while).</li>
        <li><strong>Losing both your lock and your recovery sheet.</strong> Then your data can't be opened by anyone, including you. Keep the recovery sheet somewhere safe, away from your computer.</li>
        <li><strong>A beta is a beta.</strong> No outside expert has reviewed Sompio's code yet. We plan an independent review before the full launch. Until then, keep a copy of your passwords (Vault can save one as a KeePass file).</li>
      </ul>
    </section>
  </main>
<?php site_footer(); ?>
