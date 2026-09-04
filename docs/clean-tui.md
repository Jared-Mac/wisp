# Clean TUI interface

**Clean TUI** is an optional Wisp interface profile inspired by the restraint of
Herdr without copying its layout or requiring its palette. Select it under
**Settings → Appearance → Clean TUI · focused**. Terminal Grid and Classic remain
available unchanged.

The profile treats terminal character as rhythm rather than decoration:

- the Rooms/Friends activity rail defaults to a narrow 220–360 px range;
- section frames become quiet labeled rules instead of colorful boxes;
- inactive controls and panes use neutral, low-contrast chrome;
- chat content receives more breathing room than navigation;
- transcript names and timestamps drop extra brackets and seconds;
- selections use a restrained accent wash; and
- the prompt composer, numbered sections, keyboard actions, monospace type, and
  live status line preserve the TUI identity.

Clean TUI changes presentation only. Existing tiles, drafts, conversations,
permissions, calls, and media state are retained when switching to or from it.
Color palettes remain independent: Herdr, Performative, Wisp blue, Graphite,
Violet, and Ember can all be paired with Clean TUI.

The embedded Omarchy panel remains host-managed. It uses its compact adapter
treatment and Omarchy's colors and metrics rather than inheriting the standalone
window's selected interface profile.

Verification:

```sh
bash scripts/test-appearance.sh
WISP_TEST_MODES=cleantui bash scripts/test-chat-ui.sh
bash scripts/test-ui.sh
```
