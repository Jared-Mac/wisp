# Wisp architecture

Wisp deliberately keeps desktop state and coordination state separate.

```text
standalone Quickshell ─┐
Omarchy dev.wisp ──────┼─ newline JSON over Unix socket ── wispd
wispctl ───────────────┘                                  │
                                                     HTTP + WebSocket
                                                          │
                                                     wisp-server ── SQLite
                                                          │
                                                    LiveKit tokens
                                                          │
                                                   livekit-server
```

`wispd` owns state that must survive UI restarts: the connection state machine,
mute/deafen/share controls, microphone capture, speaker playback, the LiveKit
client, and native video surfaces. Quickshell is presentation only.

Audio preferences also live in `wispd`. The daemon keeps stable device IDs,
distinguishes the user's preferred device from the currently active fallback,
and uses LiveKit's live switch operations so a headset unplug does not leave the
room. Active calls refresh device inventory every two seconds; opening either
frontend refreshes it immediately. The daemon exposes Natural, Clear, and
Studio processing presets plus a throttled local input level through the same
snapshot/event stream.

Push-to-talk is a daemon-owned microphone gate rather than a UI-only button.
Manual mute always wins. Presses carry a renewable 30-second lease, and the
event-driven lease task closes the gate after a lost release without polling
while idle. Room changes and media reconnects also clear an active press.
LiveKit active-speaker changes are normalized into sorted display names in the
media snapshot for every frontend.

Global PTT shortcut configuration also stays behind the daemon IPC boundary.
The Quickshell settings view only captures a chord. On Omarchy/Hyprland,
`wispd` validates it, writes a dedicated generated `hypr/wisp.lua` module,
adds one managed import to the user's bindings file, reloads Hyprland, and
rolls both files back if config validation fails.

`wisp-server` owns stable users, circle membership, ephemeral hangouts, messages,
presence events, and short-lived LiveKit grants. It binds to loopback by default.
The four development users have stable UUIDs and are seeded by the first migration.

The initial code is a complete control-plane vertical slice plus the Phase 0
LiveKit media path. `wispd` publishes a processed platform microphone, plays
remote tracks through the platform speaker, subscribes to remote video, and
uploads the newest decoded RGBA frame to a `wgpu` texture. A persistent winit
event-loop thread owns the Wayland connection while the `dev.wisp.surface`
window and GPU renderer can be destroyed and recreated independently. Closing
the surface does not close the LiveKit room or interrupt audio.

Video frames use bounded/latest-frame queues on both sides of the RGBA handoff,
so rendering delay does not create an unbounded backlog. Surface state and
receive/render counters are exposed through the existing IPC snapshot. The
simulator can publish a deterministic 640×360 VP8 test source with
`--publish-video`.

Remote audio subscription names are also part of the media snapshot. This makes
multi-user reliability observable without inferring membership from a single
last-received frame. Media startup failures carry a stable `error_code` plus a
human-readable message; the panel displays the message while tests and future
clients can branch on the code.

The primary desktop UI is the named `wisp` Quickshell configuration in
`quickshell/app`. It uses only upstream Quickshell and QtQuick APIs and hosts two
presentations over one bridge: a compositor-managed `FloatingWindow` app and a
compact layer-shell `PanelWindow` for the generic system tray. The shared
content switches between one and two columns as the application window changes
size. `quickshell/Panel.qml` is a thin optional Omarchy adapter that embeds the
same compact presentation in Omarchy's native anchored popup. Each active
frontend has its own pushed IPC connection; none owns call state.

The Voice MVP reliability gate is intentionally short and deterministic rather
than a one-hour soak. It connects Jared plus Tyler, Jack, and Charlie; cycles the
real media session through leave/rejoin; starts two independent Quickshell IPC
probe processes; restarts LiveKit; verifies an offline-LiveKit join produces a
clear error and then recovers; and enforces a configurable RSS-growth ceiling.

Knocks are ephemeral in-memory control events rather than messages. A request is
deduplicated by sender/recipient pair and expires after 30 seconds by default.
Creation, dismissal, acceptance, cancellation, and expiry all emit pushed server
events; each daemon refreshes its complete snapshot, so Quickshell never polls.
Only the recipient sees the pending request. Accepting joins the recipient to
the requester's current hangout, or creates an ephemeral hangout containing
both users when neither has one. A disappearing requester cancels acceptance
with a clear error and immediately removes the stale prompt.

## State flow

1. A client opens `$XDG_RUNTIME_DIR/wisp/wispd.sock` and sends `hello`.
2. `wispd` replies with a hello result and a complete snapshot.
3. Ordered events carry fresh snapshots after local or server state changes.
4. After reconnect, clients send `hello` again; event replay is not required.
5. The daemon subscribes to `/v1/events` and refreshes authoritative state from
   `/v1/snapshot` after every server event.

## Migration boundary

Moving to another Linux host requires changing `WISP_SERVER_URL`,
`WISP_LIVEKIT_URL`, the database URL, and secrets. Desktop IPC and media code do
not move.
