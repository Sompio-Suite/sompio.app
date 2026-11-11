<?php
// Sompio beta site settings. This is the only file you need to edit.
return [
    // The password testers type to get in. Change it to your own, and change it again to lock
    // everyone out (they then need the new one).
    'password' => 'CHANGE-THIS-PASSWORD',

    // Where feedback is sent. A mailbox on your own domain works best (create it in cyberfolks).
    'feedback_to' => 'feedback@sompio.org',
    // The sender of those emails. Must be an address on this domain, or many mail servers refuse it.
    'mail_from' => 'noreply@sompio.org',

    // Optional: your Discord invite, shown next to the feedback form. Leave '' to hide it.
    'discord' => '',

    // The newest setup file in private/releases is offered for download; its version number is
    // read from the file name (Sompio_0.7.1_x64-setup.exe).
];
