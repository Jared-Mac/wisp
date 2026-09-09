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
