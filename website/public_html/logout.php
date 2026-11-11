<?php
declare(strict_types=1);
require __DIR__ . '/private/lib.php';
start_session();
$_SESSION = [];
session_destroy();
setcookie('sompio_beta', '', ['expires' => time() - 3600, 'path' => '/']);
header('Location: ./', true, 303);
