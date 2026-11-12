<?php
// Receives a new version from the cloud build (GitHub Actions) and puts it up for testers and for
// the in-app updater. Only the build knows the publish code; everyone else gets a 404.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

$token = (string) (config()['publish_token'] ?? '');
if ($_SERVER['REQUEST_METHOD'] !== 'POST' || $token === '' || !hash_equals($token, (string) ($_SERVER['HTTP_X_SOMPIO_PUBLISH'] ?? ''))) {
    http_response_code(404);
    exit;
}
header('Content-Type: text/plain; charset=utf-8');
$fail = function (int $code, string $text): never {
    http_response_code($code);
    exit($text . "\n");
};

$version = (string) ($_POST['version'] ?? '');
$signature = trim((string) ($_POST['signature'] ?? ''));
$notes = trim((string) ($_POST['notes'] ?? ''));
$file = $_FILES['setup'] ?? null;
if (!preg_match('/^\d+\.\d+\.\d+$/', $version)) {
    $fail(400, 'No valid version.');
}
if ($signature === '' || strlen($signature) > 4096 || !preg_match('/^[A-Za-z0-9+\/=\s]+$/', $signature)) {
    $fail(400, 'No valid signature.');
}
if (!$file || ($file['error'] ?? 1) !== UPLOAD_ERR_OK || !is_uploaded_file($file['tmp_name'])) {
    $fail(400, 'The installer did not arrive (upload error ' . (int) ($file['error'] ?? -1) . '). Check upload_max_filesize in cPanel.');
}
$name = 'Sompio_' . $version . '_x64-setup.exe';
if ((string) $file['name'] !== $name) {
    $fail(400, "Expected $name.");
}
$current = latest_release();
if ($current !== null && version_compare($version, $current['version'], '<=')) {
    $fail(409, "Version $version is not newer than the published {$current['version']}. Nothing changed.");
}
$dir = PRIVATE_DIR . '/releases';
if (!is_dir($dir)) {
    mkdir($dir, 0750, true);
}
$dest = "$dir/$name";
if (!move_uploaded_file($file['tmp_name'], $dest)) {
    $fail(500, 'Could not save the installer.');
}
file_put_contents("$dest.sig", $signature);
file_put_contents("$dest.notes", mb_substr($notes, 0, 4000));
// Keep the newest two versions, so a download already running can finish
$all = glob("$dir/*.exe") ?: [];
usort($all, fn ($a, $b) => version_compare(preg_match('/(\d+\.\d+\.\d+)/', basename($b), $m) ? $m[1] : '0', preg_match('/(\d+\.\d+\.\d+)/', basename($a), $n) ? $n[1] : '0'));
foreach (array_slice($all, 2) as $old) {
    foreach (['', '.sig', '.notes', '.sha256'] as $ext) {
        @unlink($old . $ext);
    }
}
echo "Published Sompio $version.\n";
