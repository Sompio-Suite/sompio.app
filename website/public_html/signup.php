<?php
// "Join the beta" form on the front page. Each sign-up is added to private/data/signups.csv (open it
// in Excel) and emailed to the feedback address in config.php. No cookie, no IP address kept:
// spam is held off by a hidden field, a signed time stamp and a per-network limit (only a hash of
// the address, for one hour).
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

const SIGNUPS = PRIVATE_DIR . '/data/signups.csv';
const SIGNUP_COLUMNS = ['date', 'email', 'windows', 'browsers', 'apps', 'tools', 'launch_news', 'note'];

function done(string $query): void
{
    header('Location: ./?' . $query . '#join', true, 303);
    exit;
}

function stamp_ok(string $stamp): bool
{
    [$t, $sig] = array_pad(explode('.', $stamp, 2), 2, '');
    if (!ctype_digit($t) || !hash_equals(hash_hmac('sha256', 'signup|' . $t, form_secret()), $sig)) {
        return false;
    }
    $age = time() - (int) $t;
    // People take a few seconds to fill it in; bots don't. And forms left open a day are stale.
    return $age >= 3 && $age <= 86400;
}

function limit_file(): string
{
    $ip = $_SERVER['HTTP_CF_CONNECTING_IP'] ?? $_SERVER['REMOTE_ADDR'] ?? 'unknown';
    return PRIVATE_DIR . '/data/signup-' . substr(hash('sha256', 'sompio-signup|' . $ip), 0, 24) . '.json';
}

/** At most 5 sign-ups per network per hour. */
function over_limit(): bool
{
    $f = limit_file();
    $t = is_file($f) ? (json_decode((string) file_get_contents($f), true) ?: []) : [];
    $t = array_values(array_filter($t, fn($x) => $x > time() - 3600));
    if (count($t) >= 5) {
        return true;
    }
    $t[] = time();
    @file_put_contents($f, json_encode($t), LOCK_EX);
    foreach (glob(PRIVATE_DIR . '/data/signup-*.json') ?: [] as $old) {
        if (filemtime($old) < time() - 3600) {
            @unlink($old);
        }
    }
    return false;
}

/** Only the offered choices are kept, whatever was posted. */
function picked(string $field, bool $many): string
{
    $raw = $_POST[$field] ?? ($many ? [] : '');
    $vals = array_map('strval', $many ? (is_array($raw) ? $raw : []) : [$raw]);
    return implode(', ', array_values(array_intersect(CHOICES[$field], $vals)));
}

/** Spreadsheet programs run cells starting with these as formulas; a leading ' stops that. */
function cell(string $s): string
{
    $s = trim(preg_replace('/\s+/u', ' ', $s) ?? '');
    return preg_match('/^[=+\-@\t\r]/', $s) ? "'" . $s : $s;
}

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    header('Location: ./#join', true, 303);
    exit;
}
// Bots fill the hidden field; they get the same thank-you, and nothing is saved
if (($_POST['website'] ?? '') !== '') {
    done('joined=1');
}
if (!stamp_ok((string) ($_POST['stamp'] ?? ''))) {
    done('join_error=stale');
}
$email = strtolower(trim((string) ($_POST['email'] ?? '')));
if (strlen($email) > 120 || !filter_var($email, FILTER_VALIDATE_EMAIL)) {
    done('join_error=email');
}
if (over_limit()) {
    done('join_error=limit');
}

$row = [
    'date' => date('Y-m-d H:i'),
    'email' => $email,
    'windows' => picked('windows', false),
    'browsers' => picked('browsers', true),
    'apps' => picked('apps', true),
    'tools' => picked('tools', true),
    'launch_news' => isset($_POST['launch']) ? 'yes' : 'no',
    'note' => mb_substr((string) ($_POST['note'] ?? ''), 0, 500),
];
$row = array_map('cell', $row);

// One line per email: signing up again updates the answers instead of adding a second line
$fh = fopen(SIGNUPS, 'c+');
if ($fh === false) {
    done('join_error=server');
}
flock($fh, LOCK_EX);
$rows = [];
$updated = false;
while (($line = fgetcsv($fh, 0, ',', '"', '')) !== false) {
    $line[0] = preg_replace('/^\xEF\xBB\xBF/', '', (string) ($line[0] ?? ''));
    if ($line === [''] || $line[0] === 'date') {
        continue;
    }
    if (($line[1] ?? '') === $row['email']) {
        $line = array_values($row);
        $updated = true;
    }
    $rows[] = $line;
}
if (!$updated) {
    $rows[] = array_values($row);
}
ftruncate($fh, 0);
rewind($fh);
fwrite($fh, "\xEF\xBB\xBF"); // so Excel reads the letters correctly
fputcsv($fh, SIGNUP_COLUMNS, ',', '"', '');
foreach ($rows as $r) {
    fputcsv($fh, $r, ',', '"', '');
}
fflush($fh);
flock($fh, LOCK_UN);
fclose($fh);

$body = ($updated ? "Someone signed up again (answers updated):\n\n" : "New beta sign-up:\n\n");
foreach ($row as $k => $v) {
    $body .= str_pad($k, 12) . ' ' . ($v === '' ? '-' : $v) . "\n";
}
$body .= "\nAll sign-ups: private/data/signups.csv on the server (" . count($rows) . " so far).\n";
send_mail('Sompio beta sign-up: ' . $email, $body);

done('joined=1');
