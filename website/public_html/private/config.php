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

    // What changed, per version. The newest setup file in private/releases is offered for download;
    // its version number is read from the file name (Sompio_0.7.1_x64-setup.exe).
    'changes' => [
        '0.7.1' => [
            'One logo everywhere: window, taskbar, tray and installer.',
            'Vault: entries get their website\'s icon automatically. Click "Icon" on an entry for another address or a picture.',
            'Apps are listed as Calendar, Clean, Vault.',
        ],
        '0.7.0' => [
            'A clean white look, like KeePassXC.',
            'Clean: everything found in one list, with the message underneath.',
            'Vault: works like KeePassXC, with its keyboard shortcuts, sorting, clone and drag to move.',
            'First start: how the lock works, then your key picture, then your password.',
        ],
    ],
];
