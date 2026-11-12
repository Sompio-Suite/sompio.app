<?php
// Shared by every page: settings, the tester session, the newest release, small helpers.
declare(strict_types=1);

const PRIVATE_DIR = __DIR__;
date_default_timezone_set('Europe/Amsterdam');

function config(): array
{
    static $c = null;
    return $c ??= require PRIVATE_DIR . '/config.php';
}

function start_session(): void
{
    if (session_status() === PHP_SESSION_ACTIVE) {
        return;
    }
    $https = (!empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off') || ($_SERVER['HTTP_X_FORWARDED_PROTO'] ?? '') === 'https';
    session_name('sompio_beta');
    session_set_cookie_params([
        // Gone when the browser closes; and see SIGN_OUT_AFTER
        'lifetime' => 0,
        'path' => '/',
        'secure' => $https,
        'httponly' => true,
        'samesite' => 'Lax',
    ]);
    session_start();
}

/** Testers are signed out after this long without opening or sending anything. */
const SIGN_OUT_AFTER = 15 * 60;

/**
 * Signed in with the current password, and active in the last 15 minutes? Each visit resets the
 * clock. Changing the password in config.php signs everyone out.
 */
function signed_in(): bool
{
    start_session();
    $ok = isset($_SESSION['pw'], $_SESSION['seen'])
        && hash_equals(password_fingerprint(), (string) $_SESSION['pw'])
        && $_SESSION['seen'] > time() - SIGN_OUT_AFTER;
    if ($ok) {
        $_SESSION['seen'] = time();
    } elseif (isset($_SESSION['pw'])) {
        // Timed out: forget the sign-in (a feedback message being sent is kept, see feedback.php)
        unset($_SESSION['pw'], $_SESSION['seen']);
        $_SESSION['timed_out'] = true;
    }
    return $ok;
}

function password_fingerprint(): string
{
    return hash('sha256', 'sompio-beta|' . config()['password']);
}

function csrf_token(): string
{
    start_session();
    if (empty($_SESSION['csrf'])) {
        $_SESSION['csrf'] = bin2hex(random_bytes(16));
    }
    return $_SESSION['csrf'];
}

function csrf_ok(): bool
{
    start_session();
    return isset($_POST['csrf'], $_SESSION['csrf']) && hash_equals($_SESSION['csrf'], (string) $_POST['csrf']);
}

function e(string $s): string
{
    return htmlspecialchars($s, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}

/**
 * Wrong passwords are limited per network address: 8 tries per 15 minutes. Only a hash of the
 * address is kept, for those 15 minutes.
 */
function attempts_file(): string
{
    // Behind Cloudflare every visitor arrives from a Cloudflare address; it passes the real one along
    $ip = $_SERVER['HTTP_CF_CONNECTING_IP'] ?? $_SERVER['REMOTE_ADDR'] ?? 'unknown';
    return PRIVATE_DIR . '/data/tries-' . substr(hash('sha256', 'sompio|' . $ip), 0, 24) . '.json';
}

function too_many_tries(): bool
{
    $f = attempts_file();
    if (!is_file($f)) {
        return false;
    }
    $t = json_decode((string) file_get_contents($f), true) ?: [];
    $recent = array_filter($t, fn($x) => $x > time() - 900);
    return count($recent) >= 8;
}

function note_wrong_try(): void
{
    $f = attempts_file();
    $t = is_file($f) ? (json_decode((string) file_get_contents($f), true) ?: []) : [];
    $t = array_values(array_filter($t, fn($x) => $x > time() - 900));
    $t[] = time();
    @file_put_contents($f, json_encode($t), LOCK_EX);
    // Old counters go away by themselves
    foreach (glob(PRIVATE_DIR . '/data/tries-*.json') ?: [] as $old) {
        if (filemtime($old) < time() - 900) {
            @unlink($old);
        }
    }
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
    return $best;
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

/** What the "Join the beta" form offers (index.php shows them, signup.php keeps only these). */
const CHOICES = [
    'windows' => ['Windows 11', 'Windows 10', 'Not sure'],
    'browsers' => ['Chrome', 'Edge', 'Firefox', 'Brave', 'Other'],
    'apps' => ['Discord', 'Reddit', 'WhatsApp', 'Telegram', 'X', 'Instagram or Facebook', 'Email', 'Other'],
    'tools' => ['Trace', 'Vault', 'Calendar'],
];

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
