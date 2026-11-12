<?php
// The parts every page shares: <head>, the top menu and the footer.
declare(strict_types=1);

const SOURCE_URL = 'https://github.com/Sompio-Suite/sompio.app';

/**
 * Opens the page. $public pages may be found by search engines; the tester portal never is.
 */
function page_head(string $title, string $description, bool $public = true): void
{
    $css = 'style.css?v=' . filemtime(dirname(__DIR__) . '/style.css');
    ?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <?php if (!$public): ?><meta name="robots" content="noindex, nofollow">
  <?php endif; ?><title><?= e($title) ?></title>
  <meta name="description" content="<?= e($description) ?>">
  <link rel="icon" type="image/svg+xml" href="logo.svg">
  <link rel="stylesheet" href="<?= e($css) ?>">
</head>
<body>
<?php
}

/** The top menu. $here is the page you're on: home, guide, docs or tester. */
function site_header(string $here): void
{
    $link = function (string $href, string $label, string $key) use ($here): string {
        $cur = $key === $here ? ' aria-current="page"' : '';
        return '<a href="' . e($href) . '"' . $cur . '>' . e($label) . '</a>';
    };
    ?>
  <header class="top">
    <div class="wrap top-row">
      <a class="brand" href="./"><img src="logo.svg" alt="" width="28" height="28"> sompio <?php if ($here === 'tester'): ?><span class="tag">beta</span><?php endif; ?></a>
      <nav aria-label="Site">
        <?= $link('./#tools', 'Features', 'features') ?>
        <?= $link('delete-discord-messages.php', 'Discord guide', 'guide') ?>
        <?= $link('docs.php', 'Documentation', 'docs') ?>
        <a href="<?= e(SOURCE_URL) ?>" rel="noopener noreferrer">Source code</a>
        <a class="nav-tester<?= $here === 'tester' ? ' on' : '' ?>" href="tester.php"<?= $here === 'tester' ? ' aria-current="page"' : '' ?>>Tester portal</a>
      </nav>
    </div>
  </header>
<?php
}

function site_footer(): void
{
    ?>
  <footer class="site-foot">
    <div class="wrap foot-row">
      <div class="foot-brand"><img src="logo.svg" alt="" width="22" height="22"> <strong>sompio</strong> <span class="muted">Private tools for your PC.</span></div>
      <nav aria-label="More">
        <a href="docs.php">Documentation</a>
        <a href="delete-discord-messages.php">Delete your Discord messages</a>
        <a href="<?= e(SOURCE_URL) ?>" rel="noopener noreferrer">Source code</a>
        <a href="mailto:hello@sompio.org">hello@sompio.org</a>
      </nav>
      <p class="small muted">This site has no trackers, no ads and no analytics. The only cookie is the one that keeps testers signed in.</p>
    </div>
  </footer>
</body>
</html>
<?php
}
