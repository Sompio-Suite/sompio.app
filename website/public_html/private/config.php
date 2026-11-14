<?php
// Sompio beta site settings. This is the only file you need to edit.
return [
    // No longer used: the beta is open to everyone now, so there's no tester password.
    'password' => 'CHANGE-THIS-PASSWORD',

    // Where feedback is sent. A mailbox on your own domain works best (create it in cyberfolks).
    'feedback_to' => 'feedback@sompio.org',
    // The sender of those emails. Must be an address on this domain, or many mail servers refuse it.
    'mail_from' => 'noreply@sompio.org',

    // Optional: your Discord invite, shown next to the feedback form. Leave '' to hide it.
    'discord' => '',

    // Updates (Set up updates.bat makes both codes; the same ones go into GitHub's secrets).
    // publish_token: lets the cloud build put a new version here. update_token: lets installed
    // Sompio apps download updates. Empty turns each off.
    'publish_token' => '',
    'update_token' => '',

    // The newest setup file in private/releases is offered for download; its version number is
    // read from the file name (Sompio_0.7.1_x64-setup.exe).
];
