<?php
// "Get updates" form on the front page. Each sign-up is emailed to the feedback address in
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
if (over_net_limit('signup', 5, 3600)) {
    done('join_error=limit');
}

$row = [
    'date' => date('Y-m-d H:i'),
    'email' => $email,
    'tools' => picked('tools', true),
];
$row = array_map('cell', $row);

$body = "New sign-up for Sompio updates:\n\n";
foreach ($row as $k => $v) {
    $body .= str_pad($k, 12) . ' ' . ($v === '' ? '-' : $v) . "\n";
}
$body .= "\nThis email is the only copy: nothing is kept on the server.\n";
// Emailed only; kept on the server just until it can be sent, if mail is down
if (!deliver('Sompio updates sign-up: ' . $email, $body)) {
    done('join_error=server');
}

done('joined=1');
