# Wisp

Wisp is a Quickshell-native social layer for a small group of friends. This
repository implements presence, ephemeral hangouts, SQLite persistence, pushed
events, persistent desktop state, LiveKit voice/screen/camera media, a
CLI/simulator, detachable native video surfaces, a standalone desktop window,
and an optional Omarchy bar integration. Milestones M0–M4 are implemented.

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
Diagnostic output is available with `journalctl --user -u wisp.service`; Wisp
does not maintain an unbounded private log file.

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

The default **Clear** audio preset publishes the microphone through the
DeepFilterNet3 neural denoiser (48 kHz, 10 ms frames, 30 ms algorithmic
latency). **Natural** keeps WebRTC's lighter speech cleanup, while **Studio**
leaves the signal unprocessed. If DeepFilterNet cannot initialize, Wisp falls
back to RNNoise and reports that backend under **Settings → Audio** and in
`wispctl status`.

During a hangout, **Share screen** opens the standard XDG desktop portal picker
for a monitor or individual window, while **Camera on** publishes the selected
camera as an independent track. Screen and camera can run together, and each
remote track gets its own **Watch** control and detachable Hyprland-managed
surface. Receiving video never opens a window or starts decoding until the
viewer asks to watch it. Closing a surface unmaps its native window and pauses
that track without interrupting voice; resizing selects an appropriate simulcast
layer.

Publishing defaults to H.264 at the High profile (screen up to 1080p60 at
8 Mbps; camera up to 720p30 at 2.5 Mbps). Settings also expose Balanced and
Ultra profiles plus VP8 and AV1. Wisp asks LiveKit for a detected hardware
encoder when one exists and reports the selected backend instead of assuming
GPU support. The tray and Omarchy center-bar icon show matching, independent
screen/camera badges. The Omarchy icon also keeps unread-message and
remote-video indicators visible while a call is active; stopping a share
revokes its portal session and unpublishes the track.

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
enroll each computer with its own one-use invite:

```bash
just friend-register <host>.ts.net Tyler
just friend
```

The host creates the invite with `wispctl invite Tyler`, then sends its code and
the private media key to that friend separately from GitHub. `friend-register`
prompts without echoing either secret and stores the host, assigned identity,
device credential, and media key in
`~/.config/wisp/friend.env` with user-only permissions. The setting is local to
that computer and is never committed to the repository. Every computer is a
separately revocable device with a short-lived server session. The panel header
shows
the active profile name and live Open, Knock first, Closed, or Away presence so
identity and availability are immediately visible. During an active voice
connection the header shows Connected instead. Re-run `friend-register` with a
new invite to change the saved host, profile, or device. `wispd` itself also
requires a profile argument or `WISP_PROFILE`, so an unconfigured client can no
longer silently become Jared.

The Tailscale address and unique Tyler/Jack/Charlie assignments are private test
information and must not be committed. See
[`docs/tailscale-friend-test.md`](docs/tailscale-friend-test.md) for complete
host setup, CachyOS instructions, ports, test steps, and security limitations.

## Messages, Porch, and devices

The app includes direct messages, the small-circle timeline, persistent Spot
chats, and the current ad-hoc room timeline. Messages are committed to SQLite
before acknowledgement, missed messages synchronize after reconnect, and
unread cursors are per user. Ad-hoc room messages expire after 24 hours.
**Porch** keeps one durable chat while its voice/video room exists only when
occupied and is recreated for a later visit.

Friends can open a direct conversation from a friend row. Equivalent CLI
operations are useful for diagnostics:

```bash
wispctl dm Tyler "Are you free?"
wispctl porch
wispctl devices
wispctl revoke-device <device-id>
```

LiveKit audio and video use its built-in AES-GCM end-to-end encryption whenever
Wisp runs with device authentication. The media key is shared privately and is
never sent to `wisp-server`; the Settings device page reports whether E2EE is
active. Coordination data and messages remain server-readable in this private
alpha, so this is not yet an end-to-end-encrypted messenger.

For an always-on host installed from a release, copy and edit the generated
`~/.config/wisp/server.env`, configure LiveKit, then install the user units:

```bash
./scripts/install-host-services.sh
systemctl --user enable --now wisp-server.service
systemctl --user status wisp-server.service wisp-backup.timer
```

The daily backup timer keeps 14 verified SQLite backups under
`~/.local/share/wisp/server/backups`. Restore only while the server is stopped:

```bash
systemctl --user stop wisp-server.service
wisp-restore ~/.local/share/wisp/server/backups/<backup>.sqlite3
systemctl --user start wisp-server.service
```

To test voice and remote video without microphone feedback, start Tyler with
generated media:

```bash
just sim Tyler --publish-tone --publish-video --publish-camera
cargo run -p wispctl -- join Tyler
cargo run -p wispctl -- status
```

The status output reports the selected microphone/speaker and a growing
`received_audio_frames` counter. Synthetic screen and camera tracks appear as
separate available media. The UI's per-track **Watch** button, or
`wispctl watch Tyler screen_share` / `wispctl watch Tyler camera`, opens a
GPU-rendered native XWayland window with app ID `dev.wisp.surface`. Unwatching
either track hides its window and unsubscribes without leaving the LiveKit
room. `wispctl mute`, `unmute`, `deafen`, `undeafen`, and `leave` operate on the
LiveKit session.

Camera, codec, and publishing quality can also be controlled from the CLI:

```bash
cargo run -p wispctl -- video devices
cargo run -p wispctl -- video camera <device-id>
cargo run -p wispctl -- video quality high
cargo run -p wispctl -- video codec h264
cargo run -p wispctl -- camera on
cargo run -p wispctl -- camera off
```

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
just test-private-alpha
just lint
```

`just test-ui` launches an isolated standalone Quickshell instance and checks
the full app and compact panel lifecycles, daemon status, anchoring, and clean
shutdown. `just bootstrap` installs
the pinned, checksum-verified LiveKit development
binary into `.tools/`, and `just dev` starts it on loopback. `just test-media`
tests one publisher with synthetic screen and camera tracks plus two viewers,
on-demand subscriptions, two simultaneous native surfaces, audio continuity,
surface close/reopen independence, missing-camera recovery, and SFU
reconnection. The same media gate runs headlessly in CI. `just test-reliability` runs
the shorter Voice MVP gate: four
users, repeated leave/rejoin, real Quickshell IPC process restarts, SFU failure
and recovery, clear connection errors, and bounded RSS growth. The cycle count
and memory allowance can be adjusted with `WISP_RELIABILITY_CYCLES` and
`WISP_RSS_GROWTH_LIMIT_KIB`; no one-hour soak is required. See
`docs/architecture.md`. `just test-knock` covers request deduplication, Later,
expiry, acceptance, both offline-user cases, and simulator auto-response.
`just test-private-alpha` covers one-use enrollment, short sessions, scoped
conversation access, offline delivery, unread cursors, Porch lifecycle,
retention, restart, online backup/restore, revocation, protocol rejection, and
the no-message-content logging rule.

## Automated builds and releases

GitHub Actions builds Wisp on every push to `main` and every pull request. The
CI workflow checks formatting and Clippy, runs the Rust and headless integration
tests, scans for obvious secrets, and produces a release-mode Linux x86_64
archive. Archives from ordinary CI runs are available as workflow artifacts for
14 days.

Pushing a version tag creates a GitHub release with the archive and its SHA-256
checksum:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The archive contains `wispd`, `wispctl`, `wisp-server`, the standalone
Quickshell app, the Omarchy adapter, and the runtime launch scripts. After
installing the CachyOS runtime dependencies listed in
[`docs/tailscale-friend-test.md`](docs/tailscale-friend-test.md), extract it and
run:

```bash
./install.sh
wisp-friend-register <host>.ts.net Tyler
wisp
```

The release job uses GitHub's short-lived repository token. It does not require
or receive Tailscale credentials, LiveKit secrets, or a personal access token.
