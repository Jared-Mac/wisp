# Home servers and room activity

The first server joined becomes a home server automatically. The server dropdown
shows homes first; both groups otherwise start in join order. Use the home icon
to add or remove homes, drag a row's handle to reorder its group, or use its
up/down buttons. At least one joined server remains a home. These choices are
saved on this device in `~/.config/wisp/servers.json`, scoped to the primary
server/account identity. They are shared by the app and tray.

**Settings → Notifications → Room activity alerts** controls desktop banners
when friends join any accessible persistent room on a home server:

- Enabled by default when you are not in voice, with a five-minute cooldown per
  room and no extra sound.
- Timing can instead require Wisp to be in the background, or allow alerts while
  using Wisp. Your current voice room is always excluded.
- Optionally alert only when an empty room becomes active, customize the
  cooldown (0–120 minutes), and enable/customize the alert sound.
- Manage multiple home servers here or directly in the server dropdown.

Clicking a banner opens the room's text chat. It never joins voice or starts
media. Initial snapshots, reconnects, newly accepted friendships, and new access
to a room do not replay old joins. Several friends joining in one snapshot share
one banner. Notification jobs expire after at most a minute, with at most three
running at once. Only the standalone desktop host sends banners; a separate tray
adapter does not duplicate them.

Linux banners use `notify-send` and the desktop notification service. Install
`libnotify` on Arch or `libnotify-bin` on Debian/Ubuntu if it is missing. A test
button reports notification-service errors in settings. Timing, cooldown, and
sound preferences persist in `~/.config/wisp/notifications.json`; the global
sound mute/volume also apply to the optional room sound.

Run `bash scripts/test-room-activity.sh` with Node.js and Quickshell available.
It checks room/identity filtering and cooldowns, then tests real dropdown clicks,
dragging, cross-process persistence, and a stubbed desktop notification action
without contacting a daemon or joining voice.
