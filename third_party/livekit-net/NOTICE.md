# Wisp LiveKit transport patch

Based on crates.io `livekit-net` 0.1.2 (Apache-2.0), checksum
`46155c42ad7acccca96a0d8be2bd0f379565f93e85144fb3aa6fa406371fee22`.

Wisp adds `src/dial.rs` and exports `connect_tcp`. Native Tokio WebSocket
connections (including proxy connections) use bounded, staggered IPv6/IPv4 TCP
attempts instead of waiting for the operating system's first-address timeout.
TLS certificate verification, SNI, HTTP Host, authentication, and proxy routing
remain unchanged. Wisp's control WebSocket shares this dialer.

Keep this patch until equivalent upstream fallback behavior is available. The
other source files are unchanged from the registry release.
