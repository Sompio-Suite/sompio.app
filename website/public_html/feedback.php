<?php
// Feedback from testers: saved in private/feedback (one text file each, plus the screenshot) and
// emailed to the address in config.php. No IP address or browser details are kept.
//
// Testers are signed out after 15 minutes without activity. Writing feedback can take longer, so
// a message sent after that is kept, the password is asked again, and then it's sent (?resume=1).
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

function back(bool $ok, string $text): void
{
    $_SESSION['flash'] = ['ok' => $ok, 'text' => $text];
    header('Location: ./#feedback', true, 303);
    exit;
}

/** The uploaded screenshot, checked to really be a picture; null when there is none. */
function uploaded_shot(): ?array
{
    if (empty($_FILES['shot']) || ($_FILES['shot']['error'] ?? UPLOAD_ERR_NO_FILE) === UPLOAD_ERR_NO_FILE) {
        return null;
    }
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
    return ['data' => base64_encode((string) file_get_contents($f['tmp_name'])), 'ext' => $ext, 'type' => $mime];
}

$fieldNames = ['name', 'contact', 'area', 'kind', 'message', 'details'];

if (!signed_in()) {
    // Timed out while writing: keep the message, ask for the password, then send it
    if ($_SERVER['REQUEST_METHOD'] === 'POST' && csrf_ok()) {
        $_SESSION['pending'] = [
            'fields' => array_intersect_key(array_map('strval', $_POST), array_flip($fieldNames)),
            'shot' => uploaded_shot(),
        ];
    }
    header('Location: ./', true, 303);
    exit;
}

if (isset($_GET['resume']) && isset($_SESSION['pending'])) {
    $input = $_SESSION['pending']['fields'];
    $shot = $_SESSION['pending']['shot'];
    unset($_SESSION['pending']);
} elseif ($_SERVER['REQUEST_METHOD'] === 'POST' && csrf_ok()) {
    $input = $_POST;
    $shot = uploaded_shot();
} else {
    back(false, 'The page was open too long. Please send it again.');
}

// At most 10 messages an hour from one signed-in browser
$_SESSION['sent'] = array_values(array_filter($_SESSION['sent'] ?? [], fn($t) => $t > time() - 3600));
if (count($_SESSION['sent']) >= 10) {
    back(false, 'That\'s a lot of feedback for one hour, thank you! Please wait a bit before sending more.');
}

$clip = fn(string $k, int $max) => mb_substr(trim(str_replace("\r", '', (string) ($input[$k] ?? ''))), 0, $max);
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
$picture = $shot ? base64_decode($shot['data']) : null;
if ($shot) {
    @file_put_contents("$dir/$id.{$shot['ext']}", $picture, LOCK_EX);
}
$subject = "Sompio beta: $kind ($area)" . ($name !== '' ? " from $name" : '');
$mailed = send_mail($subject, $body, $shot ? ['name' => "$id.{$shot['ext']}", 'type' => $shot['type'], 'data' => $picture] : null);

if (!$saved && !$mailed) {
    back(false, 'Sorry, your feedback couldn\'t be saved. Please try again later, or send it another way.');
}
$_SESSION['sent'][] = time();
back(true, 'Thank you! Your feedback was sent.');
