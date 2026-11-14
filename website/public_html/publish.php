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
// "replace" swaps the files of the version that's already out (a rebuild without a version number change)
$replace = ($_POST['replace'] ?? '') === '1' && $current !== null && version_compare($version, $current['version'], '==');
if ($current !== null && !$replace && version_compare($version, $current['version'], '<=')) {
    $fail(409, "Version $version is not newer than the published {$current['version']}. Nothing changed.");
}
$dir = PRIVATE_DIR . '/releases';
if (!is_dir($dir)) {
    mkdir($dir, 0750, true);
}
$dest = "$dir/$name";
if ($replace) {
    @unlink("$dest.sha256");
}
if (!move_uploaded_file($file['tmp_name'], $dest)) {
    $fail(500, 'Could not save the installer.');
}
file_put_contents("$dest.sig", $signature);
file_put_contents("$dest.notes", mb_substr($notes, 0, 4000));
// The Mac and Linux builds live on the GitHub release; only their links, sizes, fingerprints and
// update signatures are kept here. Anything not shaped like that is left out.
$platforms = [];
$given = json_decode((string) ($_POST['platforms'] ?? ''), true);
if (is_array($given)) {
    foreach (['mac', 'mac_update', 'linux_appimage', 'linux_deb'] as $k) {
        $p = $given[$k] ?? null;
        if (!is_array($p) || !preg_match('#^https://github\.com/Sompio-Suite/[A-Za-z0-9._-]+/releases/download/v[0-9.]+/[A-Za-z0-9._-]+$#', (string) ($p['url'] ?? ''))) {
            continue;
        }
        $platforms[$k] = [
            'name' => basename((string) $p['url']),
            'url' => (string) $p['url'],
            'size' => (int) ($p['size'] ?? 0),
            'sha256' => preg_match('/^[a-f0-9]{64}$/i', (string) ($p['sha256'] ?? '')) ? strtolower($p['sha256']) : '',
            'signature' => preg_match('/^[A-Za-z0-9+\/=\s]{1,4096}$/', (string) ($p['signature'] ?? '')) ? trim($p['signature']) : '',
        ];
    }
}
file_put_contents("$dest.platforms.json", json_encode($platforms, JSON_UNESCAPED_SLASHES));
// Keep the newest two versions, so a download already running can finish
$all = glob("$dir/*.exe") ?: [];
usort($all, fn ($a, $b) => version_compare(preg_match('/(\d+\.\d+\.\d+)/', basename($b), $m) ? $m[1] : '0', preg_match('/(\d+\.\d+\.\d+)/', basename($a), $n) ? $n[1] : '0'));
foreach (array_slice($all, 2) as $old) {
    foreach (['', '.sig', '.notes', '.sha256', '.platforms.json'] as $ext) {
        @unlink($old . $ext);
    }
}
echo ($replace ? "Replaced" : "Published") . " Sompio $version.\n";
