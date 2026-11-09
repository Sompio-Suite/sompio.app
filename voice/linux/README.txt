Sompio Voice server for Linux
=============================

1. Copy this folder to your server, e.g.:   scp -r "Sompio Voice server for Linux" you@your-server:~
2. On the server:                           cd "Sompio Voice server for Linux" && sudo sh install.sh
3. The output shows the fingerprint and a one-time admin key. Keep both.
4. In Sompio: Voice, Connect, enter your server's IP or domain. Check the fingerprint matches,
   then use the admin key (server menu, Use an admin key) to become admin.

Needs Debian 12, Ubuntu 22.04 or newer (64-bit). Uses UDP port 9987 unless you choose another:
sudo sh install.sh 9988

Your domain: add an A record (for example voice.example.com) pointing to the server's IP address.
People then connect to voice.example.com (or voice.example.com:9988 for another port).
