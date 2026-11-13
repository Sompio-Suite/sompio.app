<?php
// Guides: one card per guide. The list itself lives in GUIDES (private/layout.php).
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
page_head('Guides · Sompio', 'Free, step-by-step guides to finding and removing what you\'ve shared online: Discord, Reddit, and making companies delete your data.');
site_header('guide');
?>
  <main class="wrap guides-page">
    <header class="guides-head">
      <p class="eyebrow">Guides</p>
      <h1>Take back what you've shared online</h1>
      <p class="lead">Free, step-by-step guides to finding old messages and posts that give away too much, deleting them properly, and making companies delete what they hold about you. You don't need Sompio for any of them; it just makes the work faster.</p>
    </header>

    <div class="guide-grid">
      <?php foreach (GUIDES as [$href, $title, $summary, $tag, $min]): ?>
        <a class="guide-card-link" href="<?= e($href) ?>">
          <span class="guide-tag"><?= e($tag) ?></span>
          <strong><?= e($title) ?></strong>
          <span class="guide-summary"><?= e($summary) ?></span>
          <span class="guide-foot"><span class="muted small"><?= e((string) $min) ?> min read</span><span class="guide-more">Read guide →</span></span>
        </a>
      <?php endforeach; ?>
    </div>

    <p class="small muted guides-note">Missing a guide? Tell us which app or site you'd like covered: <a href="mailto:hello@sompio.org">hello@sompio.org</a>.</p>
  </main>
<?php site_footer(); ?>
