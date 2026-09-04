# Herdr appearance option

**Herdr** is an optional Wisp palette and TUI treatment. It does not replace
Classic, Terminal, Wisp blue, Graphite, Violet, Ember, or Performative. Choose it
under **Settings → Appearance → Color palette → Herdr**; switching away
restores the selected base style and palette normally.

The option mirrors Jared's current Herdr `terminal` theme over the Solarized
Japan terminal palette:

- canvas and panels: `#001419`
- foreground: `#adb7b7`
- muted text: `#637981`
- cyan accent and focus: `#29a298`
- active-row and selection fill: `#002c38`
- green/yellow/red/magenta semantic accents: `#849900`, `#b28500`, `#db302d`,
  and `#d23681`
- JetBrainsMono Nerd Font when installed, with Wisp's normal monospace fallback

Like Performative, Herdr enables compact numbered frames, bracketed controls,
the prompt-style composer, square corners, and the live status line. It changes
presentation only: conversations, drafts, room membership, permissions, and
media state are untouched. The embedded Omarchy bar adapter shares the compact
TUI structure but remains host-managed for colors and metrics, and ignores
standalone palette choices.

Verification:

```sh
bash scripts/test-appearance.sh
WISP_TEST_THEME=terminal WISP_TEST_PALETTE=herdr bash scripts/test-chat-ui.sh
bash scripts/test-ui.sh
```
