# Security

## Reporting a problem

Found a security problem in Sompio or on sompio.org? Write to **hello@sompio.org** with "Security" in the subject.
Tell us what you found and how to see it happen. We'll reply within a few days and keep you posted while we fix it.
Please give us a fair chance to fix it before you make it public. If you'd like, we'll thank you by name.

Please don't open a public GitHub issue for security problems.

Sompio is a small, independent project, so there's no paid bug bounty yet.

## Supported versions

Only the newest version gets fixes. Installed copies offer new versions themselves.

## How Sompio protects data

See [sompio.org/security.php](https://sompio.org/security.php) for the plain-language version, and
[The Sompio Lock](README.md#the-sompio-lock) for the technical one. In short: a password through Argon2id plus a key
picture's random secret unlock keys sealed with XChaCha20-Poly1305; passwords are kept in KeePass (KDBX 4) files and
everything else in encrypted SQLite databases (ChaCha20-Poly1305). No outside expert has reviewed the code yet; an
independent review is planned before launch.
