<?php
// Shared by every page: settings, the newest release, mail, spam limits, small helpers.
// The site sets no cookies and keeps no sessions.
declare(strict_types=1);

const PRIVATE_DIR = __DIR__;
date_default_timezone_set('Europe/Amsterdam');
// Never show PHP errors to visitors: they can reveal folder paths and settings
ini_set('display_errors', '0');
ini_set('log_errors', '1');
header_remove('X-Powered-By');

function config(): array
{
    static $c = null;
    return $c ??= require PRIVATE_DIR . '/config.php';
}

function e(string $s): string
{
    return htmlspecialchars($s, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}

/** The newest Sompio_X.Y.Z_x64-setup.exe in private/releases. */
function latest_release(): ?array
{
    $best = null;
    foreach (glob(PRIVATE_DIR . '/releases/*.exe') ?: [] as $path) {
        if (!preg_match('/(\d+\.\d+\.\d+)/', basename($path), $m)) {
            continue;
        }
        if ($best === null || version_compare($m[1], $best['version'], '>')) {
            $best = ['version' => $m[1], 'path' => $path, 'name' => basename($path)];
        }
    }
    if ($best === null) {
        return null;
    }
    $best['size'] = filesize($best['path']);
    $best['date'] = filemtime($best['path']);
    // The SHA-256 fingerprint, worked out once per file
    $cache = $best['path'] . '.sha256';
    if (!is_file($cache) || filemtime($cache) < $best['date']) {
        @file_put_contents($cache, hash_file('sha256', $best['path']));
    }
    $best['sha256'] = is_file($cache) ? trim((string) file_get_contents($cache)) : hash_file('sha256', $best['path']);
    // The Mac and Linux builds of the same version, on the GitHub release (see publish.php)
    $p = @file_get_contents($best['path'] . '.platforms.json');
    $best['platforms'] = $p === false ? [] : (json_decode($p, true) ?: []);
    return $best;
}

/** The systems the newest version is out for: "Windows, Mac and Linux", or "Windows 10 and 11". */
function systems_text(): string
{
    $p = latest_release()['platforms'] ?? [];
    $more = array_values(array_filter([isset($p['mac']) ? 'Mac' : null, (isset($p['linux_appimage']) || isset($p['linux_deb'])) ? 'Linux' : null]));
    return $more ? 'Windows, ' . implode(' and ', $more) : 'Windows 10 and 11';
}

function human_size(int $bytes): string
{
    return $bytes >= 1048576 ? number_format($bytes / 1048576, 1) . ' MB' : max(1, (int) round($bytes / 1024)) . ' KB';
}

/** Sends a plain-text email, with one picture attached when given. */
function send_mail(string $subject, string $body, ?array $attachment = null): bool
{
    $c = config();
    $to = $c['feedback_to'];
    $from = $c['mail_from'];
    $headers = ['From: Sompio beta <' . $from . '>', 'MIME-Version: 1.0'];
    $subject = '=?UTF-8?B?' . base64_encode($subject) . '?=';
    if ($attachment === null) {
        $headers[] = 'Content-Type: text/plain; charset=UTF-8';
        $headers[] = 'Content-Transfer-Encoding: base64';
        return @mail($to, $subject, chunk_split(base64_encode($body)), implode("\r\n", $headers), '-f' . $from);
    }
    $boundary = 'sompio-' . bin2hex(random_bytes(8));
    $headers[] = 'Content-Type: multipart/mixed; boundary="' . $boundary . '"';
    $msg = "--$boundary\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"
        . chunk_split(base64_encode($body))
        . "--$boundary\r\nContent-Type: {$attachment['type']}; name=\"{$attachment['name']}\"\r\n"
        . "Content-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"{$attachment['name']}\"\r\n\r\n"
        . chunk_split(base64_encode($attachment['data']))
        . "--$boundary--\r\n";
    return @mail($to, $subject, $msg, implode("\r\n", $headers), '-f' . $from);
}

/**
 * Emails a sign-up or feedback message to the mailbox. Nothing is kept on the server, unless the
 * email can't be sent right now: then it waits in private/outbox until it can, and is deleted as
 * soon as it goes out. Returns false only when it could neither be sent nor kept.
 */
function deliver(string $subject, string $body, ?array $attachment = null): bool
{
    flush_outbox();
    if (send_mail($subject, $body, $attachment)) {
        return true;
    }
    $dir = PRIVATE_DIR . '/outbox';
    if (!is_dir($dir)) {
        @mkdir($dir, 0700, true);
    }
    $item = ['subject' => $subject, 'body' => $body, 'attachment' => $attachment === null ? null : [
        'name' => $attachment['name'], 'type' => $attachment['type'], 'data' => base64_encode($attachment['data']),
    ]];
    return @file_put_contents($dir . '/' . date('Ymd-His') . '-' . bin2hex(random_bytes(4)) . '.json', json_encode($item), LOCK_EX) !== false;
}

/** Tries again to send what's waiting in the outbox; whatever goes out is deleted. */
function flush_outbox(): void
{
    foreach (glob(PRIVATE_DIR . '/outbox/*.json') ?: [] as $f) {
        $item = json_decode((string) @file_get_contents($f), true);
        if (!is_array($item)) {
            @unlink($f);
            continue;
        }
        $a = $item['attachment'] ?? null;
        if ($a !== null) {
            $a['data'] = base64_decode((string) $a['data']);
        }
        if (send_mail((string) $item['subject'], (string) $item['body'], $a)) {
            @unlink($f);
        } else {
            return; // mail still down: try again next time
        }
    }
}

/** What the "Get updates" form offers (index.php shows them, signup.php keeps only these). */
const CHOICES = [
    'tools' => ['Vault', 'Trace', 'Calendar'],
];

/**
 * True when this network already did `$what` `$max` times in the last `$seconds`. Counts are kept
 * under a scrambled code made from the address (never the address itself) and deleted after.
 */
function over_net_limit(string $what, int $max, int $seconds): bool
{
    $ip = $_SERVER['HTTP_CF_CONNECTING_IP'] ?? $_SERVER['REMOTE_ADDR'] ?? 'unknown';
    $dir = PRIVATE_DIR . '/data';
    if (!is_dir($dir)) {
        @mkdir($dir, 0700, true);
    }
    $f = $dir . '/' . $what . '-' . substr(hash('sha256', 'sompio-' . $what . '|' . $ip), 0, 24) . '.json';
    $t = is_file($f) ? (json_decode((string) file_get_contents($f), true) ?: []) : [];
    $t = array_values(array_filter($t, fn($x) => $x > time() - $seconds));
    if (count($t) >= $max) {
        return true;
    }
    $t[] = time();
    @file_put_contents($f, json_encode($t), LOCK_EX);
    foreach (glob($dir . '/' . $what . '-*.json') ?: [] as $old) {
        if (filemtime($old) < time() - $seconds) {
            @unlink($old);
        }
    }
    return false;
}

/** A form's signed time stamp is real, at least 3 seconds old (people type; bots don't) and under a day. */
function stamp_ok(string $stamp): bool
{
    [$t, $sig] = array_pad(explode('.', $stamp, 2), 2, '');
    if (!ctype_digit($t) || !hash_equals(hash_hmac('sha256', 'signup|' . $t, form_secret()), $sig)) {
        return false;
    }
    $age = time() - (int) $t;
    return $age >= 3 && $age <= 86400;
}

/** A secret only this server knows, made once, for signing the form's time stamp. */
function form_secret(): string
{
    $f = PRIVATE_DIR . '/data/form-secret';
    if (!is_file($f)) {
        @file_put_contents($f, bin2hex(random_bytes(32)), LOCK_EX);
    }
    return trim((string) @file_get_contents($f)) ?: 'sompio-fallback';
}

/** Put in the form when the page is shown: the time, signed. */
function signup_stamp(): string
{
    $t = (string) time();
    return $t . '.' . hash_hmac('sha256', 'signup|' . $t, form_secret());
}

require_once PRIVATE_DIR . '/layout.php';
