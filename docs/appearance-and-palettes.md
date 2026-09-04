# Appearance, palettes, and color accents

Settings → Appearance has three independent groups, shared by the tray popup,
main window, and chat pop-outs. These are local presentation preferences; no
style enables or removes a feature.

- **Appearance:** Performative (default), Clean TUI, Herdr, Terminal Grid, Classic.
  This controls typography, spacing, frames, prompts, and control treatment.
- **Palette:** Ash & Olive (the former Performative colors), Solarized Japan
  (the former Herdr colors), Wisp blue, Graphite, Violet, Ember. Changing palettes
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
Unknown environments retain their old default. Embedded host-managed Omarchy
surfaces keep host ownership; standalone Omarchy windows have all choices.

Tests: `scripts/test-appearance.sh`, `scripts/test-performative.sh`, and
`scripts/test-chat-ui.sh`. The UI fixture accepts `WISP_TEST_COLOR_MODE=chat-only`,
`neutral`, or `all`, and `WISP_TEST_APPEARANCE_SETTINGS=1` for settings previews.
