<?php
// Feedback from testers: saved in private/feedback (one text file each, plus the screenshot) and
// emailed to the address in config.php. No IP address or browser details are kept.
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

function back(bool $ok, string $text): void
{
    $_SESSION['flash'] = ['ok' => $ok, 'text' => $text];
    header('Location: ./#feedback', true, 303);
    exit;
}

if (!signed_in()) {
    header('Location: ./', true, 303);
    exit;
}
if ($_SERVER['REQUEST_METHOD'] !== 'POST' || !csrf_ok()) {
    back(false, 'The page was open too long. Please send it again.');
}

// At most 10 messages an hour from one signed-in browser
$_SESSION['sent'] = array_values(array_filter($_SESSION['sent'] ?? [], fn($t) => $t > time() - 3600));
if (count($_SESSION['sent']) >= 10) {
    back(false, 'That\'s a lot of feedback for one hour, thank you! Please wait a bit before sending more.');
}

$clip = fn(string $k, int $max) => mb_substr(trim(str_replace("\r", '', (string) ($_POST[$k] ?? ''))), 0, $max);
$one = fn(string $s) => preg_replace('/\s+/', ' ', $s);
$name = $one($clip('name', 80));
$contact = $one($clip('contact', 120));
$area = $one($clip('area', 40));
$kind = $one($clip('kind', 40));
$message = $clip('message', 8000);
$details = $clip('details', 8000);

if (mb_strlen($message) < 3) {
    back(false, 'Please write a few words about what happened.');
}
if ($contact !== '' && !filter_var($contact, FILTER_VALIDATE_EMAIL)) {
    back(false, 'That email address doesn\'t look right. Leave it empty if you don\'t need a reply.');
}

// The screenshot, if any: checked to really be a picture
$shot = null;
if (!empty($_FILES['shot']) && ($_FILES['shot']['error'] ?? UPLOAD_ERR_NO_FILE) !== UPLOAD_ERR_NO_FILE) {
    $f = $_FILES['shot'];
    if ($f['error'] !== UPLOAD_ERR_OK || $f['size'] > 5 * 1024 * 1024) {
        back(false, 'The screenshot didn\'t come through. It needs to be a PNG or JPG under 5 MB.');
    }
    $info = @getimagesize($f['tmp_name']);
    $types = [IMAGETYPE_PNG => ['png', 'image/png'], IMAGETYPE_JPEG => ['jpg', 'image/jpeg'], IMAGETYPE_WEBP => ['webp', 'image/webp']];
    if (!$info || !isset($types[$info[2]])) {
        back(false, 'The screenshot needs to be a PNG or JPG picture.');
    }
    [$ext, $mime] = $types[$info[2]];
    $shot = ['data' => (string) file_get_contents($f['tmp_name']), 'ext' => $ext, 'type' => $mime];
}

$id = date('Y-m-d_His') . '-' . bin2hex(random_bytes(3));
$version = latest_release()['version'] ?? '?';
$body = "Sompio beta feedback ($id)\n"
    . "Version on the site: $version\n"
    . "From: " . ($name !== '' ? $name : 'anonymous') . ($contact !== '' ? " <$contact>" : '') . "\n"
    . "Part: $area\nKind: $kind\n\n"
    . "What happened:\n$message\n"
    . ($details !== '' ? "\nBug report details:\n$details\n" : '')
    . ($shot ? "\nScreenshot attached ($id.{$shot['ext']}).\n" : '');

$dir = PRIVATE_DIR . '/feedback';
$saved = @file_put_contents("$dir/$id.txt", $body, LOCK_EX) !== false;
if ($shot) {
    @file_put_contents("$dir/$id.{$shot['ext']}", $shot['data'], LOCK_EX);
}
$subject = "Sompio beta: $kind ($area)" . ($name !== '' ? " from $name" : '');
$mailed = send_mail($subject, $body, $shot ? ['name' => "$id.{$shot['ext']}", 'type' => $shot['type'], 'data' => $shot['data']] : null);

if (!$saved && !$mailed) {
    back(false, 'Sorry, your feedback couldn\'t be saved. Please try again later, or send it another way.');
}
$_SESSION['sent'][] = time();
back(true, 'Thank you! Your feedback was sent.');
