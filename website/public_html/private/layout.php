<?php
// The parts every page shares: <head>, the top menu and the footer.
declare(strict_types=1);

const SOURCE_URL = 'https://github.com/Sompio-Suite/sompio.org';
const SUPPORT_URL = 'https://ko-fi.com/7ab9ne3021sn4b';
const HEART_ICON = '<svg class="heart-icon" viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path fill="currentColor" d="M12 21s-7.5-4.6-9.6-9.3C.9 8.2 3 4.5 6.6 4.5c2.1 0 3.6 1.2 4.4 2.5.8-1.3 2.3-2.5 4.4-2.5 3.6 0 5.7 3.7 4.2 7.2C19.5 16.4 12 21 12 21z"/></svg>';
const GITHUB_ICON = '<svg class="gh-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/></svg>';

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
        <a href="<?= e(SOURCE_URL) ?>" class="gh-link" rel="noopener noreferrer" aria-label="Source code on GitHub" title="Source code on GitHub"><?= GITHUB_ICON ?><span class="gh-label">Source code</span></a>
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
        <a href="<?= e(SOURCE_URL) ?>" class="gh-link" rel="noopener noreferrer"><?= GITHUB_ICON ?>Source code</a>
        <a href="mailto:hello@sompio.org">hello@sompio.org</a>
        <a href="<?= e(SUPPORT_URL) ?>" class="support-link" rel="noopener noreferrer"><?= HEART_ICON ?>Support Sompio</a>
      </nav>
      <p class="small muted">This site has no trackers, no ads and no analytics. The only cookie is the one that keeps testers signed in.</p>
    </div>
  </footer>
</body>
</html>
<?php
}
