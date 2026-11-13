<?php
// "Join the beta" form on the front page. Each sign-up is emailed to the feedback address in
// config.php, and nothing is kept on the server (only while email can't go out, in private/outbox).
// No cookie, no IP address kept:
// spam is held off by a hidden field, a signed time stamp and a per-network limit (only a hash of
// the address, for one hour).
declare(strict_types=1);
require __DIR__ . '/private/lib.php';


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

$body = "New beta sign-up:\n\n";
foreach ($row as $k => $v) {
    $body .= str_pad($k, 12) . ' ' . ($v === '' ? '-' : $v) . "\n";
}
$body .= "\nThis email is the only copy: nothing is kept on the server.\n";
// Emailed only; kept on the server just until it can be sent, if mail is down
if (!deliver('Sompio beta sign-up: ' . $email, $body)) {
    done('join_error=server');
}

done('joined=1');
