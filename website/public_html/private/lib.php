<?php
// Shared by every page: settings, the tester session, the newest release, small helpers.
declare(strict_types=1);

const PRIVATE_DIR = __DIR__;

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
        'lifetime' => 60 * 60 * 24 * 30,
        'path' => '/',
        'secure' => $https,
        'httponly' => true,
        'samesite' => 'Lax',
    ]);
    session_start();
}

/** Signed in with the current password? Changing the password in config.php signs everyone out. */
function signed_in(): bool
{
    start_session();
    return isset($_SESSION['pw']) && hash_equals(password_fingerprint(), (string) $_SESSION['pw']);
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
    $ip = $_SERVER['REMOTE_ADDR'] ?? 'unknown';
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
