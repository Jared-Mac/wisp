# UI experimentation

Branch: `codex/ui-experimentation`, based on main `2f4d9b4`.

Choose a style in **Settings → Appearance**:

- **Soft Graphite** is the default for new standalone desktop installations: charcoal surfaces, soft blue accents, and outline controls.
- **Daylight** uses warm light surfaces and forest-green accents.
- **Hearth** uses plum surfaces, peach accents, rounder controls, chat bubbles, and small ghost avatars.

The original Wisp wordmark stays in the header. Existing saved styles are retained, including Classic, Terminal Grid, Clean TUI, Performative, and Herdr. Host-managed shell styling remains owned by the host. Each new style remembers its chosen palette; returning to a traditional style restores its previous palette. The six independent accent switches remain available.

Room navigation, chat/DM tiles, permissions, and media commands stay shared across styles. Voice controls use icons with hover/focus tooltips and remain visible below the room list. Disconnect sits beside the connection status. The connected room's participant list provides stream controls without repeating them below the voice buttons.

Settings now show common options first, with expandable sections for passwords, custom emojis, microphone testing, shortcuts, reconnection, encoding, custom sounds, encryption details, invitations, and server administration. Audio and Video have separate tabs. Search opens the matching category, expands the relevant section, and scrolls to its setting. Device, quality, palette, and tray-position choices use compact selectors. Microphone testing stops and discards its sample when its section is closed.

## Validation

- `test-interface-styles.sh`: all three styles at 1180 and 840 pixels, original branding, visible voice controls, preserved chat tiles, settings access, keyboard expansion, and no media commands from appearance changes. `WISP_INTERFACE_SCREENSHOTS=/existing/directory` saves previews with synthetic identities.
- Appearance migration, palette independence, persistence, terminal controls, room navigation, participant controls, stream window/tile lifecycle, settings search, audio selection/testing, camera confirmation, onboarding, and workspace persistence have targeted coverage.
- Chat workspace fixtures cover the main window and tray, attachments, editing, geometry, navigation, permissions, settings, and appearance switching.
- Two older suites, `test-local-controls.sh` and `test-friends.sh`, also fail on the unchanged base commit. The former fails at its unread-navigation fixture before reaching stream assertions; the latter fails its friend-order expectation. These are not counted as passing validation for this branch.
- The release workspace build succeeds. This branch changes no server code or migrations and does not deploy to production. The current production build includes separate soundboard work above this branch's base; it must be preserved.

No Windows build is introduced by this UI experiment. The shared QML controls use local SVGs rather than icon fonts or downloaded assets.

## September 9 refinements

The activity divider now remembers an explicit width and stops at 24 logical
pixels. Narrow sidebars shorten names, wrap room and voice actions, and move
friend actions into a keyboard-accessible menu. Server selection, settings and
invitations retain separate targets; menus keep a readable width. Collapse is
still explicit. Main-window call controls use the same compact icons as the tray.

Appearance includes a persistent avatar visibility option. Profile has a local
preview, upload, and removal for account pictures. The server authenticates each
write as the current account, bounds image size and decoding, crops to 256px PNG,
and announces changes. Clients validate and mask thumbnails again, cache by
server and user, and discard stale image responses. Old servers show placeholder
avatars and an actionable update message when uploading. Migration 25 adds avatar
storage; version 24 is reserved for the existing unpublished soundboard release.
Do not replace that production build without recovering its matching source.

Confirmed mute/deafen state changes play four original cues. Startup, repeated
snapshots, server moderation and push-to-talk transitions do not produce those
cues. Notifications includes a separate control-sound switch and per-event audio
file customization, honoring global sound mute and volume.

Validation: workspace Rust tests (138 passed, three pre-existing ignored),
Clippy for server and daemon, profile-image lifecycle and hostile-image tests,
UI checks at 24/48/88/140/200/360px on both sides in all three styles, persistence,
profile shortcuts, stream window/tile/leave behavior, settings search, room flow,
chat navigation, and audio preferences. Synthetic UI previews contain no real
account or server information. Discord notes have payload/privacy tests and are
sent only by the main-push workflow.
