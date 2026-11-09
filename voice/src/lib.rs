//! Sompio Voice: self-hosted voice chat with channels, like TeamSpeak. One program is the
//! server (run it anywhere, reach it by IP or domain); the Sompio app is the client.
//! Transport: QUIC (TLS 1.3), with the server's certificate pinned on first use.

pub mod bookmarks;
pub mod client;
pub mod proto;
pub mod server;
