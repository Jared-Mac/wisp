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
snapshot/event stream. Clear defaults to an embedded DeepFilterNet3 model on a
dedicated bounded worker and exposes DeepFilterNet's attenuation limit as a
live 0–100 strength setting. Clear continuously publishes the model output with
no secondary detector or post-denoise gate, so a classifier cannot clip quiet
words or consonants. RNNoise remains only as an automatic denoiser fallback.
Inference state stays off the async networking/UI executor. The daemon also
reports per-window peak processing time, cumulative 10 ms deadline misses, and
capture-queue depth.

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

`wisp-server` owns accounts, friendships, server and room roles, channel
categories, encrypted-conversation routing, ephemeral hangouts, messages,
saved rooms, device identities, presence events, and short-lived LiveKit grants.
It binds to loopback by default. A production database has no compiled users or
rooms: one explicit bootstrap creates the immutable server owner, and all other
accounts arrive through expiring friend or room invitations. Stable fixture
accounts exist only when development sessions are explicitly enabled.

`wispd` may hold several independent server sessions. It keeps each server's
snapshot, encryption vault, device credential, and event stream separate, then
exposes a scoped aggregate to QML. The active server controls navigation while
open chat tiles may span servers. Voice has one explicit server context at a
time; switching it disables outgoing video before leaving the prior server.
This is client aggregation, not server federation.

Passwords are Argon2id hashes. Login or invited registration returns one opaque,
revocable credential for that physical installation. Only credential hashes are
stored in SQLite. A device exchanges that credential for a 12-hour session,
and `wispd` renews it when needed. Revoking one device invalidates both its
credential and outstanding sessions without affecting another device owned by
the same person. Login failures are throttled per trusted peer/account and
password verification runs behind a bounded work semaphore.

All authenticated requests require protocol version 1. WebSocket events carry
only change notifications, never message bodies; clients respond by fetching a
fresh authorized snapshot. `wispd` reconnects with bounded exponential delay
and deterministic jitter, then restores its desired hangout/media state.

Conversation transport remains deliberately small: direct, circle, and
hangout. Dedicated server text channels reuse encrypted circle transport and
carry separate category metadata; they never create or join voice. The
server verifies membership on every read/write, inserts a message and advances
the sender's read cursor in one transaction, and acknowledges only after that
transaction commits. Per-user cursors produce unread counts, while a full
snapshot supplies missed messages after reconnect. Direct, circle,
dedicated-channel, and persistent-room history is durable; ad-hoc hangout
history is deleted after 24 hours. TestRoom is a development-only fixture;
production rooms are account-created records with one persistent conversation,
while each media room exists only while occupied.

Device-authenticated LiveKit connections require a client-held media key. The
key configures LiveKit's built-in AES-GCM E2EE manager in each client and never
crosses the coordination API. Chat content uses separate client-held age and
Ed25519 keys with signed membership chains; the server stores ciphertext but can
still observe coordination metadata. The current shared room media key remains
a replaceable boundary for future per-device distribution.

The media path publishes a DeepFilterNet-processed platform microphone, plays
remote audio through the selected platform speaker, and independently publishes
screen and camera video. Portal-authorized PipeWire screen capture and
GStreamer camera capture are converted to I420 and sent as separate LiveKit
tracks. Quality profiles choose capture dimensions, frame rate, and bitrate;
the selected H.264, VP8, or AV1 codec is passed explicitly to LiveKit. Encoder
backend discovery prefers hardware only when the installed LiveKit/WebRTC build
reports it as available.

Remote screen and camera publications are availability metadata until the user
clicks the track's **Watch** control. `wispd` then subscribes to that publication
and uploads the newest decoded RGBA frame to a `wgpu` texture. Each track has a
separate `dev.wisp.surface` XWayland window on a persistent winit event-loop
thread, so Hyprland can float, tile, fullscreen, or pin it normally. Closing a
surface unsubscribes only that video track; voice and other video tracks remain
active. Occlusion pauses delivery and resize events request low, medium, or high
simulcast layers. Desired subscriptions survive an SFU reconnect without
opening any window the user did not request.

Closing a native surface unmaps its X11 window and unsubscribes the corresponding
video track; reopening maps the cached surface again. Wisp uses XWayland because
winit cannot hide Wayland windows, while the proprietary NVIDIA Vulkan driver
faults when Wisp destroys a live GPU device. At process shutdown, cached
renderers are left for the operating system to reclaim, avoiding the driver's
faulty `vkDestroyDevice` path when no application state needs to survive.

Video frames use bounded/latest-frame queues on both sides of the RGBA handoff,
so rendering delay does not create an unbounded backlog. Surface state and
receive/render counters are exposed through the existing IPC snapshot. The
simulator can publish deterministic 640×360 VP8 screen and camera sources with
`--publish-video --publish-camera`.

Remote audio and video subscription names are also part of the media snapshot.
This makes multi-user reliability and Watch availability observable without
inferring membership from a single last-received frame. Media startup failures
carry a stable `error_code` plus a
human-readable message; the panel displays the message while tests and future
clients can branch on the code.

The primary desktop UI is the named `wisp` Quickshell configuration in
`quickshell/app`. It uses only upstream Quickshell and QtQuick APIs and hosts two
presentations over one bridge: a compositor-managed `FloatingWindow` app and a
compact layer-shell `PanelWindow` for the generic system tray. The shared
content switches between one and two columns as the application window changes
size. Wisp-owned windows persist an interface profile independently from their
color palette: Classic, Terminal Grid, Performative, Herdr, or the restrained,
narrow-rail Clean TUI. Palette selection never changes style metrics; independent
color-accent preferences control chat rules/headings, activity sections and names.
All profiles consume the same bridge state and expose the same actions.
`quickshell/Panel.qml` is a thin optional Omarchy adapter that embeds the
same compact presentation in Omarchy's native anchored popup. It opts into the
shared TUI structure while overriding visual tokens with Omarchy's host colors,
font, scale, corners, and geometry. Each active
frontend has its own pushed IPC connection; none owns call state.

The Voice MVP reliability gate is intentionally short and deterministic rather
than a one-hour soak. It connects Owner plus MemberA, MemberB, and MemberC; cycles the
real media session through leave/rejoin; starts two independent Quickshell IPC
probe processes; restarts LiveKit; verifies an offline-LiveKit join produces a
clear error and then recovers; and enforces a configurable RSS-growth ceiling.

The M3 media gate adds one publisher and two viewers. It verifies simultaneous
screen and camera tracks, explicit per-track watching, no decoded frames while
hidden, two native surfaces on a graphical host, headless receiving for the
second viewer, uninterrupted outgoing voice across video close/reopen, camera
failure as recoverable state, and resubscription after a LiveKit restart.

The M4 private-alpha gate starts the server with development authentication
disabled. It validates bootstrap of the first administrator device, one-use
friend invites, replay rejection, short sessions, device revocation, strict
protocol compatibility, conversation authorization, durable/offline direct
messages, read cursors, TestRoom lifecycle, transient hangout retention, restart,
SQLite online backup/restore, and absence of message content in normal logs.
Host and client systemd user services use restart-on-failure and journald; the
daily timer retains 14 integrity-checked backups.

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
