# Performative terminal mode

**Performative** is the default on new Wisp-owned desktop windows, including
standalone windows opened from Omarchy. Existing saved appearance choices,
host-managed Omarchy adapter styling, and unknown environments stay unchanged.
Choose **Settings → Appearance → Performative** to switch an existing
installation. This mode enforces monospace
type, square corners, and a flat black canvas even when the base style is Classic.
Switching to another palette restores that base style.
It applies to the full app, tray, dialogs, and detached chat windows, without
changing chats, room membership, media controls, or the host-managed Omarchy bar
theme.

Inspired by the supplied btop screenshot: black canvas, restrained olive highlights and
online names, neutral text, yellow room accents, violet friend headings and remote
message authors, and subdued red chat frames. Errors still use red; warnings and
favorites use yellow. Other palettes retain their existing colors.
Mint is reserved for the small online-status indicators, not large surface fills.

Each conversation gets its own border and frame-label color. Assignments are
saved locally in `~/.config/wisp/chat-colors.json`, keyed by conversation ID, so
renaming, reordering, closing/reopening, splitting, and popping out chats retain
their colors. Focus thickens the border without changing its hue. The first
eight assignments use distinct terminal-friendly swatches; larger histories get
additional colors (which may be perceptually closer together). The host-managed
Omarchy adapter shares the compact frame structure while retaining its own
palette, typography, scaling, corners, and popup geometry.

The terminal treatment includes numbered curses-like panel frames, bracketed
controls, ash-gray inverse selections, a dark status bar, a `user@wisp` identity, compact chat logs with
24-hour timestamps, a prompt editor with a block caret, and a live status bar.
The prompt sends chat messages, never shell commands. The footer reflects actual
connection/media state and advertises existing shortcuts, not invented commands.
Room names and media actions stay grouped at their natural widths. Tray section
headers, friend rows, and the prompt editor use tighter spacing without changing
other themes' layout.
Chat dropdowns fit their labels (capped at 260 logical pixels and the available
header width), with a dark surface and subtle outline. Hover/focus gives feedback
without a large inverse fill; long names elide and remain available in a tooltip.

This is local UI only: no server migration, API update, or host deployment needed.

Verification:

```sh
bash scripts/test-appearance.sh
bash scripts/test-performative.sh
bash scripts/test-chat-colors.sh
WISP_TEST_THEME=terminal WISP_TEST_PALETTE=performative bash scripts/test-chat-ui.sh
```
