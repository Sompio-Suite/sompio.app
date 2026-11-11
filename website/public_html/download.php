<?php
// Hands the newest setup file to signed-in testers only. The file itself sits in private/,
// which the web server never serves, so a shared link without the password gets nothing.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

if (!signed_in()) {
    header('Location: ./', true, 303);
    exit;
}
$r = latest_release();
if ($r === null) {
    http_response_code(404);
    exit('No test version is up yet.');
}
while (ob_get_level()) {
    ob_end_clean();
}
header('Content-Type: application/octet-stream');
header('Content-Disposition: attachment; filename="' . $r['name'] . '"');
header('Content-Length: ' . $r['size']);
header('X-Content-Type-Options: nosniff');
header('Cache-Control: private, no-store');
readfile($r['path']);
