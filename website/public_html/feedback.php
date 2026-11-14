<?php
// Feedback from anyone using the beta: emailed to the address in config.php, with the screenshot
// attached. Nothing stays on the server: a message waits in private/outbox only while email can't
// go out. No IP address or browser details are kept. Spam is held off by a hidden field, a signed
// time stamp and a per-network limit (only a scrambled code made from the address, for one hour).
declare(strict_types=1);
require __DIR__ . '/private/lib.php';

const SENT_MESSAGES = [
    'sent' => [true, 'Thank you! Your message was sent.'],
    'short' => [false, 'Please write a few words about what happened.'],
    'contact' => [false, 'That email address doesn\'t look right. Leave it empty if you don\'t need a reply.'],
    'shot' => [false, 'The screenshot didn\'t come through. It needs to be a PNG or JPG picture under 5 MB.'],
    'stale' => [false, 'The page was open too long, or sent too fast. Please send it again.'],
    'limit' => [false, 'That\'s a lot of messages from your network in one hour. Please wait a bit before sending more.'],
    'server' => [false, 'Sorry, your message couldn\'t be sent. Please try again later, or write to hello@sompio.org.'],
];

function back(string $code): void
{
    header('Location: feedback.php?r=' . $code . '#form', true, 303);
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
        back('shot');
    }
    $info = @getimagesize($f['tmp_name']);
    $types = [IMAGETYPE_PNG => ['png', 'image/png'], IMAGETYPE_JPEG => ['jpg', 'image/jpeg'], IMAGETYPE_WEBP => ['webp', 'image/webp']];
    if (!$info || !isset($types[$info[2]])) {
        back('shot');
    }
    [$ext, $mime] = $types[$info[2]];
    return ['data' => (string) file_get_contents($f['tmp_name']), 'ext' => $ext, 'type' => $mime];
}

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    // Bots fill the hidden field: they get the same thank-you, and nothing is sent
    if (($_POST['website'] ?? '') !== '') {
        back('sent');
    }
    if (!stamp_ok((string) ($_POST['stamp'] ?? ''))) {
        back('stale');
    }
    $clip = fn(string $k, int $max) => mb_substr(trim(str_replace("\r", '', (string) ($_POST[$k] ?? ''))), 0, $max);
    $one = fn(string $s) => preg_replace('/\s+/', ' ', $s);
    $name = $one($clip('name', 80));
    $contact = $one($clip('contact', 120));
    $area = in_array($a = $one($clip('area', 40)), ['Vault', 'Trace', 'Calendar', 'Installing or starting', 'The lock', 'Something else'], true) ? $a : 'Something else';
    $kind = in_array($k = $one($clip('kind', 40)), ['Something didn\'t work', 'Something was confusing', 'An idea', 'Something I liked'], true) ? $k : 'An idea';
    $message = $clip('message', 8000);
    $details = $clip('details', 8000);
    if (mb_strlen($message) < 3) {
        back('short');
    }
    if ($contact !== '' && !filter_var($contact, FILTER_VALIDATE_EMAIL)) {
        back('contact');
    }
    $shot = uploaded_shot();
    if (over_net_limit('feedback', 10, 3600)) {
        back('limit');
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
    $subject = "Sompio beta: $kind ($area)" . ($name !== '' ? " from $name" : '');
    // Emailed only; kept on the server just until it can be sent, if mail is down
    if (!deliver($subject, $body, $shot ? ['name' => "$id.{$shot['ext']}", 'type' => $shot['type'], 'data' => $shot['data']] : null)) {
        back('server');
    }
    back('sent');
}

$result = SENT_MESSAGES[$_GET['r'] ?? ''] ?? null;
$c = config();
page_head('Send feedback · Sompio', 'Tell us what worked, what broke and what you\'d like in Sompio.');
site_header('feedback');
?>
  <main class="wrap narrow">
    <p class="eyebrow">Beta</p>
    <h1>Tell us what you found</h1>
    <p class="lead">Bugs, confusing bits, ideas, or what you liked. Everything helps, and it decides what gets fixed first.
      <?php if (!empty($c['discord'])): ?>You can also post in our <a href="<?= e($c['discord']) ?>" rel="noopener noreferrer">Discord</a>.<?php endif; ?></p>

    <section id="form">
      <?php if ($result): ?><p class="<?= $result[0] ? 'success' : 'error' ?>" role="status"><?= e($result[1]) ?></p><?php endif; ?>
      <form method="post" action="feedback.php" enctype="multipart/form-data" class="card feedback">
        <input type="hidden" name="stamp" value="<?= e(signup_stamp()) ?>">
        <div class="hp" aria-hidden="true"><label>Leave this empty <input name="website" tabindex="-1" autocomplete="off"></label></div>
        <div class="row2">
          <label>Name or nickname <span class="muted">(optional)</span><input name="name" maxlength="80"></label>
          <label>Email, if you'd like a reply <span class="muted">(optional)</span><input name="contact" type="email" maxlength="120"></label>
        </div>
        <div class="row2">
          <label>Which part?
            <select name="area">
              <option>Vault</option><option>Trace</option><option>Calendar</option>
              <option>Installing or starting</option><option>The lock</option><option>Something else</option>
            </select>
          </label>
          <label>What kind?
            <select name="kind">
              <option>Something didn't work</option><option>Something was confusing</option>
              <option>An idea</option><option>Something I liked</option>
            </select>
          </label>
        </div>
        <label>What did you do, and what happened? <textarea name="message" rows="6" maxlength="8000" required placeholder="I clicked …, and then …"></textarea></label>
        <label>Bug report details <span class="muted">(optional: in Sompio, Security, "Copy details for a bug report", then paste here)</span>
          <textarea name="details" rows="4" maxlength="8000" class="mono"></textarea></label>
        <label>Screenshot <span class="muted">(optional, PNG or JPG up to 5 MB. Windows key + Shift + S takes one. Please don't show real passwords or personal details.)</span>
          <input type="file" name="shot" accept="image/png,image/jpeg,image/webp"></label>
        <button class="primary" type="submit">Send feedback</button>
        <p class="small muted">Your message is emailed to us and not kept on this website. How we handle it: see the <a href="privacy.php#what">privacy statement</a>.</p>
      </form>
    </section>
  </main>
<?php site_footer(); ?>
