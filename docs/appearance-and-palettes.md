# Appearance, palettes, and color accents

Settings → Appearance has three independent groups, shared by the tray popup,
main window, and chat pop-outs. These are local presentation preferences; no
style enables or removes a feature.

- **Appearance:** Classic (default), TUI, Clean TUI, Herdr, Terminal Grid.
  This controls typography, spacing, frames, prompts, and control treatment.
- **Palette:** Ash & Olive (the former Performative colors), Solarized Japan
  (the former Herdr colors), Wisp blue, Graphite, Violet, Ember, Astra. Changing palettes
  does not change fonts, geometry, or which interface style is selected.
- **Color accents:** independently enable distinct chat borders/rules, distinct
  chat headings, room section accents, friends section accents, online friend
  names, and message sender names. Focus, unread, presence, and safety indicators
  remain distinguishable even when decorative colors are disabled.

For Clean TUI with Performative colors and chat-only accents: select Clean TUI,
select Ash & Olive, enable both chat controls, and disable room/friends controls.
Clean TUI retains its quiet rules instead of becoming a boxed terminal layout.
Chat identity colors remain stable across tiles and restarts in chat-colors.json;
turning them off does not erase their assignments.

Preferences live in `~/.config/wisp/appearance.json`. Older palette-driven
Performative/Herdr configurations resolve to the corresponding appearance plus
palette, preserving Clean TUI when it was explicitly selected. On the next
settings change, version 2 saves those axes and color options explicitly. Existing
Clean TUI preferences default to neutral decorative sections. Changing either
appearance or palette thereafter retains the independently chosen color options.
New installations default to Classic with Wisp blue. TUI retains the saved
`performative` identifier, so existing explicit style and palette choices survive. Embedded host-managed Omarchy
surfaces keep host ownership; standalone Omarchy windows have all choices.

Tests: `scripts/test-appearance.sh`, `scripts/test-performative.sh`, and
`scripts/test-chat-ui.sh`. The UI fixture accepts `WISP_TEST_COLOR_MODE=chat-only`,
`neutral`, or `all`, and `WISP_TEST_APPEARANCE_SETTINGS=1` for settings previews.

## Main-window readability

Classic main windows and chat pop-outs use the redesigned reading layout with
16px messages, 14px controls, and 18px conversation titles. The compact popup and host-managed Omarchy adapter keep their existing
density and typography. TUI restores compact monospace text, square labeled frames,
bracketed buttons, a chat prompt, and a block caret; Clean TUI retains quieter rules.
All styles share the same chats, media controls, soundboard, and settings. Solarized
Japan uses brighter secondary and safety text in Classic main windows.

Presence choices live in one descriptive menu. Mute/deafen remain available in
chat and settings; narrow windows move workspace layout into the account menu.
The sidebar has a distinct surface, and wide layouts pin the active call controls
below the friend list. Stacked layouts keep those controls in the room list.
Message actions appear on hover or keyboard focus, with existing reactions always
visible. The composer retains drafts, attachments, emoji, and its editing keys.
Below 720px of workspace width, Rooms & friends opens a full-height drawer;
Escape or selecting another conversation returns to chat. Active call controls
remain reachable below chat while the drawer is closed. Multiple chat panes
automatically become tabs when their minimum widths exceed the available space;
widening restores the saved tile arrangement without changing it.

Settings opens on Audio, with microphone, speaker, cleanup, and Test my voice
together. Video has a separate tab, with codec and hardware details under Advanced
video. Search opens the matching tab and expands advanced controls when needed.

Validation: `scripts/test-chat-ui.sh` includes a `makeover` interaction fixture.
Run `WISP_TEST_READING=1 WISP_TEST_THEME=legacy bash scripts/test-room-flow.sh`
to exercise room navigation, invitations, and leaving voice with the reading layout.

Astra matches Omarchy’s Astra palette: ink-blue `#0c1224`, raised surfaces
`#1c2843`, periwinkle `#a397ec`, cool silver-blue text `#bac7df`, and violet,
teal, gold, and rose status accents. Choose **Color palette → Astra** with any
interface style. It retains saved layout, conversation colors, and color options;
the Omarchy adapter continues to follow its host theme. The generated WISP
wordmark keeps its original artwork colors.

## Terminal layout refinement

TUI and Herdr use Herdr's JetBrains Mono font preference, falling back to the
installed monospace family. Inactive frames are subdued; the active chat retains
a crisp single-cell rule. A narrow activity column and flat chat canvas give the
transcript priority. Presence uses one bracketed menu, timestamps use HH:mm, and
message actions appear on hover or keyboard focus alongside the sender. Existing
reactions remain visible. The composer labels its destination as `message /room`.
The neutral footer separates connection/media state from shortcut hints; narrow
windows prioritize status and expose the full text on hover. Chat tiles fall back
to tabs and the sidebar becomes a drawer at narrow widths, preserving saved layout.
Classic, Clean TUI, and host-managed Omarchy styling remain independent.

Design references: [Herdr](https://herdr.dev/),
[Lazygit](https://github.com/jesseduffield/lazygit), and [Charm](https://charm.land/).
