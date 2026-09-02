# Wisp

Wisp is a Quickshell-native social layer for a small group of friends. This
repository implements presence, ephemeral hangouts, SQLite persistence, pushed
events, persistent desktop state, a LiveKit voice path, a CLI/simulator, a
standalone desktop window, and an optional Omarchy bar integration.

## Run it

```bash
./scripts/bootstrap.sh
just dev
```

In another terminal:

```bash
just app
just sim Tyler
cargo run -p wispctl -- status
cargo run -p wispctl -- join Tyler
```

`just app` syncs and opens the portable, resizable Quickshell application.
`just panel` syncs and opens its compact anchored presentation.
To install it as the named `wisp` Quickshell configuration, application
launcher, and `wisp-ui` command:

```bash
just app-sync
wisp
wisp-ui app open
wisp-ui app toggle
wisp-ui panel toggle
```

The app uses a single column at narrow sizes and a two-column layout when more
space is available. The standalone process stays connected to `wispd` when
both surfaces are hidden, so reopening is immediate and voice state remains
owned by the daemon. The legacy `wisp-ui open` and `wisp-ui toggle` forms remain
aliases for the full app.

Use `wisp` for a normal terminal launch. It starts the complete saved client or
reveals the already-running Wisp app. The searchable **Wisp** application menu
entry calls the same launcher, so both routes enforce one daemon, one tray icon,
and one UI process. The client runs as the per-user `wisp.service`, so the
terminal command returns after opening the app; `systemctl --user status
wisp.service` shows its state.

Launching **Wisp** from the desktop application menu starts the saved friend
client when necessary, or reopens the existing app when the daemon is already
running. After **Exit Wisp**, the same launcher starts the complete client again
rather than opening a disconnected UI. If the remote host is temporarily
offline, the daemon and tray remain available and retry until the host returns.
Diagnostic output is stored in
`~/.local/state/wisp/launcher.log`.

`wispd` also publishes a Wisp system-tray icon on desktops that support
StatusNotifierItem. Left-click the icon to toggle the Wisp panel beside the
tray. Right-click for panel Show/Hide, **Open Wisp app**, mute, deafen,
panel-anchor, and **Exit Wisp** actions. The panel also has an **Open app**
button. Muted and deafened states update the tray icon, tooltip, checked menu
items, and compact icons in the panel. Microphone mute is orange; deafen is red
and always forces microphone mute. Clicking the headset again leaves the
microphone muted; clicking the microphone while deafened clears both states.
The lower in-call controls contain only video, sharing, and leave actions;
small mute/deafen icons appear beside the local member name in a hangout.
The panel always opens on the operating system's primary display and remembers
its corner choice under **Settings → Desktop position**. Auto uses the tray
click's edge when the tray is on the primary display and otherwise falls back
to the primary display's bottom-right corner.

**Exit Wisp** closes the Quickshell UI and the tray-owning daemon. When Wisp
was started with `just dev`, that daemon exit also makes the development
supervisor stop its local server and LiveKit children, leaving no Wisp
background processes behind.

## Optional Omarchy integration

Omarchy users can additionally install the center-bar adapter:

```bash
just plugin-sync
omarchy-shell shell toggle dev.wisp
```

The Omarchy adapter and generic tray popup use the same compact `WispContent`
presentation; the adapter simply supplies Omarchy's native anchored popup host.
Both offer **Open app** for the separate resizable layout. The adapter and the
standalone configuration have independent pushed connections to `wispd`; call
state remains daemon-owned. The adapter is not required on CachyOS or other
desktops.

## Trusted Tailscale friend test

For a short multi-PC test, the host can run a separate tailnet-only mode:

```bash
just dev-tailscale
just tailscale-info  # run in another terminal
```

Friends on CachyOS clone this repository, run `just friend-bootstrap`, and then
start their assigned development profile with:

```bash
just friend-config <host>.ts.net Tyler
just friend
```

`friend-config` stores the host and assigned identity in
`~/.config/wisp/friend.env` with user-only permissions. The setting is local to
that computer and is never committed to the repository. The panel header shows
the active profile name and live Open, Knock first, Closed, or Away presence so
identity and availability are immediately visible. During an active voice
connection the header shows Connected instead. Explicit arguments such as
`just friend <host>.ts.net Tyler` override
and update the saved values. `wispd` itself also requires a profile argument or
`WISP_PROFILE`, so an unconfigured client can no longer silently become Jared.

The Tailscale address and unique Tyler/Jack/Charlie assignments are private test
information and must not be committed. See
[`docs/tailscale-friend-test.md`](docs/tailscale-friend-test.md) for complete
host setup, CachyOS instructions, ports, test steps, and security limitations.

To test voice and remote video without microphone feedback, start Tyler with
generated media:

```bash
just sim Tyler --publish-tone --publish-video
cargo run -p wispctl -- join Tyler
cargo run -p wispctl -- status
```

The status output reports the selected microphone/speaker and a growing
`received_audio_frames` counter. Synthetic video opens a GPU-rendered native
Wayland window with app ID `dev.wisp.surface`; `wispctl surface close` and
`wispctl surface open` destroy and recreate that window without leaving the
LiveKit room. `wispctl mute`, `unmute`, `deafen`, `undeafen`, and `leave` operate
on the LiveKit session.

Audio devices and processing are available in both Quickshell frontends and
from the CLI:

```bash
cargo run -p wispctl -- audio devices
cargo run -p wispctl -- audio input <device-id>
cargo run -p wispctl -- audio output <device-id>
cargo run -p wispctl -- audio preset natural  # or clear / studio
```

Device selection is applied without leaving an active room. If a preferred
headset disappears, Wisp uses an available fallback and restores the headset
when it returns.

Push-to-talk can be enabled under Wisp **Settings → Audio** or from the CLI:

```bash
cargo run -p wispctl -- ptt enable
cargo run -p wispctl -- ptt press
cargo run -p wispctl -- ptt release
cargo run -p wispctl -- ptt disable
```

The daemon automatically closes a press whose release is lost. On
Omarchy/Hyprland, install the CLI once with `just cli-install`, then click
**Set shortcut** in Wisp Settings and press the desired chord. Wisp writes a
small generated `~/.config/hypr/wisp.lua`, imports it from `bindings.lua`, and
reloads and validates Hyprland. **Clear** removes Wisp's binding. Other
compositors can bind the same press/release CLI commands manually.

Friends whose presence is `knock` receive expiring join requests. The recipient
gets Join/Later controls in the Wisp panel; the same flow is available from the
CLI:

```bash
cargo run -p wispctl -- join Tyler
cargo run -p wispctl -- knock <knock-id> join
# or: cargo run -p wispctl -- knock <knock-id> later
```

For unattended development, a simulator can respond automatically with
`just sim Tyler --presence knock --auto-respond-knocks join`.

To bind `Super+H`, copy the single reviewed line from
`infra/local/wisp-bindings.lua` into `~/.config/hypr/bindings.lua`, then run
`hyprctl reload` and `hyprctl configerrors`. The repository does not overwrite
personal Hyprland configuration automatically.

## Verification

```bash
just test
just test-integration
just test-media
just test-reliability
just test-knock
just test-ui
just lint
```

`just test-ui` launches an isolated standalone Quickshell instance and checks
the full app and compact panel lifecycles, daemon status, anchoring, and clean
shutdown. `just bootstrap` installs
the pinned, checksum-verified LiveKit development
binary into `.tools/`, and `just dev` starts it on loopback. `just test-media`
tests real local LiveKit audio/video, surface close/reopen independence, and SFU
reconnection. `just test-reliability` runs the shorter Voice MVP gate: four
users, repeated leave/rejoin, real Quickshell IPC process restarts, SFU failure
and recovery, clear connection errors, and bounded RSS growth. The cycle count
and memory allowance can be adjusted with `WISP_RELIABILITY_CYCLES` and
`WISP_RSS_GROWTH_LIMIT_KIB`; no one-hour soak is required. See
`docs/architecture.md`. `just test-knock` covers request deduplication, Later,
expiry, acceptance, both offline-user cases, and simulator auto-response.
