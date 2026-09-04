# Terminal-theme trial — historical record

**Adopted after user review.** Terminal is now the default for non-Omarchy desktop
launches, with the original **Classic** theme available in **Settings → Appearance**.
Both themes expose the same features and preserve chat/media state. The record
below describes the earlier, isolated trial; its no-merge/no-push status and
experimental naming are historical, not the current release policy. For the old
appearance, use Settings rather than the historical full-UI rollback script,
which would also remove subsequently added UI features such as the selector.

**Later policy update:** the embedded Omarchy bar popup remains host-managed,
but the standalone resizable Wisp window is now Wisp-owned and offers the same
appearance choices and Performative default as other known desktop launches.

## Starting point and scope

The task began on `codex/wisp-edit` at `a2281d3`, with the completed room,
attachment-retention, and history-clearing work uncommitted. Those changes were
committed and pushed to `origin/main` as
`5f052d93824a4223a30db203c56c27c295bd1040`, as requested. There were no unrelated
uncommitted files left. The dedicated `codex/experimental-terminal-theme` branch
was then created from that clean commit; it did not previously exist.

Only this experiment's UI, appearance-launch metadata, tests, and this document
have changed since that starting point. No backend, protocol, database, server
deployment, release, remote installation, or experiment push is included. The
theme has not been adopted or merged into main. Review with:

```sh
git diff 5f052d93824a4223a30db203c56c27c295bd1040 -- quickshell scripts tests docs/terminal-theme-trial.md
```

## Design and launch paths

`WispTheme.qml` retains the exact legacy defaults and adds a separate
`terminal-experimental` profile: Hack (resolved against `Qt.fontFamilies()`, then
available fixed-width alternatives), 13px body, 12px captions, 16px headings,
4px corners, and subtle existing-palette outlines. Presence dots and media
indicators, all semantic colors, and the waveform asset are unchanged. Spacing
scale remains 1.0. Small edited/retention annotations retain their subtle size.

`shell.qml` owns one appearance selector and one theme shared by the tray popup,
full app, and preview window. The default dimensions, two-column activity/chat
layout, anchoring, scrolling, dismissal, editor shortcuts, and media behavior
are retained. Trial-only width constraints prevent long labels colliding with
actions. Media buttons can wrap in the narrow activity column; the legacy row's
positions and dimensions are preserved.

Missing font-family declarations in editors, menus, dialogs, attachment labels,
and Qt Quick Controls use conditional `Binding` overrides with restoration.
`TrialControlStyle.qml` supplies conditional native-control palettes; existing
controls keep their focus/hover/pressed/disabled behavior. Shared ChatButton
adds trial-only outlines and pressed feedback. Existing settings and preview
components inherit the same theme rather than receiving separate skins.

Launch paths:

- Desktop launcher, daemon startup, system-tray actions, and **Open app** route
  through `wisp-ui`, which launches the shared standalone `shell.qml`.
- `wisp-ui` marks recognized CachyOS launches, but Omarchy integration metadata,
  `OMARCHY_PATH`, Omarchy installation/configuration directories, or Omarchy
  executables take precedence. Unknown environments remain legacy. No user
  names, accounts, or hostnames are used.
- The optional `quickshell/Panel.qml` explicitly keeps `profile: "legacy"` and
  all host palette/font/size/spacing overrides. Its **Open app** launcher passes
  Omarchy integration metadata, so the standalone app also remains legacy.
- Direct `qs` launches without recognized metadata remain legacy. An existing
  standalone process retains its launch environment until restarted.

## Local activation and rollback

Per the follow-up request, there is no opt-in prompt or settings flow: the trial
was enabled directly on this local CachyOS installation. Its internal local
selector is `/home/tlt26/.config/wisp/appearance.json`:

```json
{"profile":"terminal-experimental"}
```

The file is not committed or copied by app-sync. Other installations are not
enabled. Charlie's machine and Jared's machine were not accessed or modified.
Changing the value to `legacy` and restarting only the UI restores the legacy
profile in both windows. To re-enable locally, use `terminal-experimental` and
restart the UI. Use absolute commands to avoid accidentally invoking a different
checkout's launcher:

```sh
/home/tlt26/.local/bin/wisp-ui quit
/home/tlt26/.local/bin/wisp-ui app open
```

The pre-trial installed UI, launcher, daemon, CLI, and appearance-state marker
are saved in this durable directory:

`/home/tlt26/.local/share/wisp/theme-trials/terminal-20260903.j76geE/`

There was no appearance.json before the trial. The `appearance-was-absent`
marker records that. For a complete installed-UI rollback, run:

```sh
bash /home/tlt26/.local/share/wisp/theme-trials/terminal-20260903.j76geE/rollback-appearance.sh
```

That script first saves the current UI/configuration into a new rollback
snapshot, restores the previous installed UI and launcher, restores the prior
absence of the appearance selector, and reopens the app. It does not change the
Git branch, discard work, touch chat storage or credentials, replace the daemon,
join rooms, or activate media. Prefer the profile-only switch if you have made
later UI updates you want to retain. Do not use a Git checkout as a substitute
for restoring installed UI/configuration.

## Verification

- Release workspace build, isolated UI IPC lifecycle test, appearance-policy
  tests, chat logic tests, and chat UI fixtures passed.
- Appearance-policy matrix: CachyOS, Omarchy, unknown; trial, legacy, invalid,
  and absent settings. Local standalone launches also verified both surfaces
  in legacy, forced-Omarchy, and restored-CachyOS trial modes.
- 28 before/after image comparisons had **zero differing pixels**: legacy and
  simulated Omarchy host overrides, each across app, image, popup, settings,
  editing, files, room/DM clear dialogs, room settings, new room, app/popup media,
  preview, and empty states. Baseline sources came from `git archive 5f052d9`,
  rendered with the same fixtures and environment as the experiment.
- Trial previews inspected at 1180×900 / 460×800, constrained 840×700 / 400×700,
  and Qt 125% scaling. Long names, populated/empty chat, room/admin dialogs,
  settings, menus, synthetic media/previews, transfer progress, and focus states
  were exercised without contacting real conversations or activating hardware.
- Running binary and installed UI were compared to the checkout. The local
  profile reports `terminal-experimental`, environment `cachyos`, font `Hack`
  for both presentations. No room joined; mic publishing/camera/share off.
- The complete rollback script was executed successfully, confirming the old
  installed UI and absent appearance configuration were restored. The trial
  was then reinstalled/re-enabled and its profile verified again.

Screenshots are in the durable backup directory: `terminal-final-app.png`,
`terminal-final-panel.png`, `fractional-*.png`, and `verified-{before,after}-*.png`.
The `live-app.png` and `live-panel.png` files, if present, are local desktop
captures and may contain actual conversation content; use fixtures for sharing.

Not verified: a live Omarchy shell on Jared's hardware, Charlie's installation,
all possible font inventories/display scales, or compositor-owned titlebar
styling. The host adapter was tested through representative local overrides,
and Omarchy standalone launch selection was simulated locally. No remote
deployment or broader rollout has occurred. Adoption requires a later decision.
