<?php
// Where installed copies of Sompio look for updates (Tauri's updater). Answers only apps that send
// the update code built into Sompio, so the beta installer stays private like the rest of the site.
//   update.php        the newest version, its signature and where to get it (JSON)
//   update.php?get=1  the installer itself
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

$token = (string) (config()['update_token'] ?? '');
if ($token === '' || !hash_equals($token, (string) ($_SERVER['HTTP_X_SOMPIO_UPDATE'] ?? ''))) {
    http_response_code(404);
    exit;
}
header('Cache-Control: private, no-store');
$r = latest_release();
$sig = $r ? @file_get_contents($r['path'] . '.sig') : false;
if ($r === null || $sig === false) {
    // Nothing to offer (Tauri reads 204 as "up to date")
    http_response_code(204);
    exit;
}
if (isset($_GET['get'])) {
    @ini_set('zlib.output_compression', 'Off');
    while (ob_get_level()) {
        ob_end_clean();
    }
    header('Content-Type: application/octet-stream');
    header('Content-Disposition: attachment; filename="' . $r['name'] . '"');
    header('Content-Length: ' . $r['size']);
    readfile($r['path']);
    exit;
}
$notes = @file_get_contents($r['path'] . '.notes');
$platforms = [
    'windows-x86_64' => [
        'signature' => trim($sig),
        'url' => 'https://' . ($_SERVER['HTTP_HOST'] ?? 'sompio.org') . '/update.php?get=1',
    ],
];
// Mac (one build for Apple silicon and Intel) and Linux (the AppImage), downloaded from GitHub
$mac = $r['platforms']['mac_update'] ?? null;
if ($mac && $mac['signature'] !== '') {
    $platforms['darwin-aarch64'] = $platforms['darwin-x86_64'] = ['signature' => $mac['signature'], 'url' => $mac['url']];
}
$linux = $r['platforms']['linux_appimage'] ?? null;
if ($linux && $linux['signature'] !== '') {
    $platforms['linux-x86_64'] = ['signature' => $linux['signature'], 'url' => $linux['url']];
}
header('Content-Type: application/json');
echo json_encode([
    'version' => $r['version'],
    'notes' => $notes === false ? '' : trim($notes),
    'pub_date' => gmdate('Y-m-d\TH:i:s\Z', $r['date']),
    'platforms' => $platforms,
], JSON_UNESCAPED_SLASHES);
