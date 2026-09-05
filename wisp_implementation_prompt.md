# Wisp — Phased Implementation Prompt

## Mission

Implement **Wisp**, a lightweight, Quickshell-native, voice-first communication system for a small group of friends.

Wisp should feel like a **desktop social layer**, not a conventional Discord clone. Omarchy should have first-class bar integration, while the primary on-demand Quickshell window must also run on desktops such as CachyOS without Omarchy's shell modules. The initial implementation should prioritize:

- exceptionally low idle overhead;
- excellent voice quality;
- fast join/leave behavior;
- first-class Quickshell integration;
- native Hyprland behavior;
- screen sharing and camera as detachable media surfaces;
- a small, understandable Rust backend;
- clean separation between the local desktop daemon and the self-hosted coordination/media server;
- easy local development.

For now, **self-host everything on the developer's local Omarchy PC**. The architecture must make it straightforward to move the server-side processes to another Linux machine or RK1 later without redesigning the client.

Do **not** implement a TUI in the initial plan.

---

# Product Model

Wisp is organized around **people and live activity**, not servers and channel trees.

The primary questions the UI should answer are:

1. Who is around?
2. Who is hanging out?
3. Can I join?

The ordinary user-facing concepts are:

- **Friend** — another user.
- **Presence** — whether friends may join:
  - `Open`
  - `Knock`
  - `Closed`
  - `Away`
- **Hangout** — an ephemeral live session containing people.
- **Spot** — an optional saved hangout preset such as `TestRoom`.
- **Circle** — a durable trust/membership group; keep this mostly invisible when only one exists.
- **Media track** — voice, camera, or screen-share media associated with a participant.

The central UX principle is:

> Discord makes users enter a place to find their friends. Wisp shows users their friends and creates a place around whatever they are doing.

---

# Initial User Experience

Most of the time, Wisp should have **no visible application window**. A standalone Quickshell process may remain connected invisibly for immediate recall.

The optional Omarchy bar adapter may show compact state such as:

```text
󰍬 MemberA MemberB
```

When connected:

```text
🎙 MemberA MemberB      🖥 MemberB
```

A hotkey such as `Super+H` should summon the standalone Wisp window on any supported desktop. On Omarchy, the same content may also appear as an anchored bar panel:

```text
╭─ Wisp ───────────────────────────╮
│ NOW                              │
│                                  │
│ MemberA + MemberB                     │
│ Factorio                  JOIN   │
│                                  │
│ FRIENDS                          │
│ MemberC                  ● open  │
├──────────────────────────────────┤
│ Mute       Share       Leave     │
╰──────────────────────────────────╯
```

The panel should disappear when no longer needed while voice continues uninterrupted.

Screen shares and cameras should be separate Hyprland-managed surfaces rather than forcing the user into a permanent video-call grid.

---

# Architecture

Use this separation from the beginning:

```text
┌────────────────────────── Local Linux PC ──────────────────────────┐
│                                                                    │
│  standalone Quickshell ─┐                                         │
│  Omarchy dev.wisp ──────┼──── Unix socket ────┐                    │
│  wispctl ───────────────┘                     ▼                    │
│       wispd                    desktop daemon                       │
│       ├── mic / speaker state                                      │
│       ├── LiveKit client                                           │
│       ├── audio processing                                         │
│       ├── screen / camera publication                              │
│       ├── remote video surfaces                                    │
│       └── persistent local call state                              │
│          │                                                         │
│          ├────────────▶ wisp-server                                │
│          │              presence                                   │
│          │              users / circles                            │
│          │              hangouts                                   │
│          │              messages                                   │
│          │              LiveKit token issuance                     │
│          │              SQLite                                     │
│          │                                                         │
│          └────────────▶ livekit-server                             │
│                         voice / video SFU                           │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

All three server/client processes run on the same PC during initial development.

Later migration should require moving only:

- `wisp-server`
- `livekit-server`
- the server database
- server secrets

The following remain on every user's desktop:

- `wispd`
- Quickshell frontend
- local audio processing
- local capture
- local video rendering

---

# Repository Layout

Use a single monorepo.

Keep the initial crate count deliberately small.

```text
wisp/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── justfile
│
├── crates/
│   └── wisp-protocol/
│
├── apps/
│   ├── wispd/
│   ├── wisp-server/
│   └── wispctl/
│
├── tools/
│   └── wisp-sim/
│
├── quickshell/
│   ├── manifest.json
│   ├── BarWidget.qml
│   ├── Panel.qml
│   └── app/
│       ├── shell.qml
│       ├── WispBridge.qml
│       ├── WispWindow.qml
│       ├── WispContent.qml
│       ├── components/
│       └── views/
│
├── migrations/
├── infra/
│   └── local/
├── scripts/
└── docs/
    ├── architecture.md
    └── protocol.md
```

Do not create separate crates for media, storage, an SDK, plugins, or alternate frontends until the codebase genuinely needs those boundaries.

---

# Architectural Rules

Maintain these boundaries aggressively.

## `wispd`

Owns desktop-local functionality:

- LiveKit client connection;
- microphone capture;
- speaker playback;
- audio device state;
- audio processing;
- screen capture;
- camera capture;
- video publication/subscription;
- remote video rendering;
- connection state;
- local settings;
- Quickshell IPC.

`wispd` must continue a call even if Quickshell restarts.

## `wisp-server`

Owns server-side coordination:

- users;
- device identities later;
- circle membership;
- friend relationships;
- presence;
- hangout lifecycle;
- access rules;
- knock requests;
- persistent messages;
- SQLite persistence;
- LiveKit room/token issuance.

## `livekit-server`

Owns real-time media routing:

- audio;
- screen-share video;
- camera video;
- WebRTC transport.

Do not write a custom SFU.

## Quickshell frontends

Owns presentation only:

- standalone on-demand window;
- optional Omarchy bar indicator and anchored panel;
- notifications;
- animations;
- controls;
- shell integration.

Quickshell must not contain:

- LiveKit credentials;
- database logic;
- WebRTC;
- audio processing;
- media encoding;
- application secrets.

## `wispctl`

Keep a small CLI for development and automation.

Examples:

```bash
wispctl status
wispctl presence open
wispctl join MemberA
wispctl mute
wispctl deafen
wispctl leave
wispctl share window
```

This is not a TUI.

---

# Development Profiles

Seed four development users:

```text
Owner
MemberA
MemberB
MemberC
```

Use stable UUIDs internally. Display names must never be primary keys.

Provide:

```bash
wispd --profile Owner
wisp-sim --profile MemberA
wisp-sim --profile MemberB
wisp-sim --profile MemberC
```

`wisp-sim` should be capable of simulating presence, room membership, messages, synthetic audio, and eventually synthetic video so multi-user behavior can be tested from one PC.

---

# Phase 0 — Technical Risk Spikes

Before implementing product features, retire the highest-risk integrations.

## 0A. Rust + Local LiveKit Audio

Run LiveKit locally.

Prove:

```text
Rust publisher
      │
 synthetic tone
      ▼
 local LiveKit
      │
      ▼
Rust receiver
```

Then replace the synthetic source with a real microphone.

Prove:

- token creation;
- room connection;
- audio publication;
- audio reception;
- speaker playback;
- mute/unmute;
- clean leave;
- reconnect behavior.

Do not build Wisp social semantics yet.

## 0B. Quickshell ↔ `wispd` IPC

Implement:

```text
$XDG_RUNTIME_DIR/wisp/wispd.sock
```

Use a simple newline-delimited JSON protocol initially.

Example:

```json
{"v":1,"type":"snapshot","self":{"muted":false},"friends":[]}
```

Quickshell must:

- connect;
- receive a complete snapshot;
- update visible UI;
- send a command;
- handle daemon restart;
- reconnect;
- request a fresh snapshot.

Do not poll.

## 0C. Remote Video Rendering

Do this before full screen sharing.

Prove that `wispd` can:

1. receive synthetic video frames;
2. upload them to a GPU texture;
3. render them in a native Wayland window;
4. close/reopen that window;
5. keep the LiveKit session alive independently.

Preferred initial model:

```text
LiveKit video frames
        │
        ▼
      wispd
        │
       wgpu
        │
        ▼
Hyprland-managed window
```

Give Wisp media windows a stable app ID such as:

```text
dev.wisp.surface
```

### Phase 0 Exit Gate

Do not continue until all three are proven:

- local Rust LiveKit audio;
- Quickshell IPC;
- viable remote video surface.

---

# Phase 1 — Development Foundation

Create a repeatable one-command development environment.

## Local Processes

```text
livekit-server   127.0.0.1:7880
wisp-server      127.0.0.1:8787
wispd            Unix domain socket
wisp-ui          standalone Quickshell process
omarchy-shell    optional existing Quickshell process
wisp-sim         configurable fake clients
```

Keep services bound to loopback during development.

## Commands

Provide a `justfile` with at least:

```bash
just bootstrap
just dev
just dev-server
just dev-daemon
just sim MemberA
just sim MemberB
just sim MemberC
just plugin-sync
just plugin-watch
just app
just app-sync
just test-ui
just test
just test-integration
just fmt
just lint
```

`just dev` should start the complete local stack and cleanly terminate child processes on exit.

## Paths

Use XDG paths:

```text
$XDG_CONFIG_HOME/wisp/
$XDG_DATA_HOME/wisp/
$XDG_STATE_HOME/wisp/
$XDG_RUNTIME_DIR/wisp/
```

For example:

```text
$XDG_DATA_HOME/wisp/server/wisp.sqlite3
$XDG_RUNTIME_DIR/wisp/wispd.sock
```

## Tooling

Set up immediately:

- `cargo fmt`;
- `clippy`;
- unit tests;
- integration tests;
- structured `tracing`;
- SQLite migrations;
- dependency lockfile;
- secret scanning;
- debug/release profiles.

---

# Phase 2 — Portable Quickshell UI + Omarchy Adapter

Build the actual Wisp interaction model using fake state first.

Suggested structure:

```text
quickshell/
├── manifest.json
├── BarWidget.qml
├── Panel.qml
└── app/
    ├── shell.qml
    ├── WispBridge.qml
    ├── WispWindow.qml
    ├── WispContent.qml
    ├── components/
    │   ├── HangoutCard.qml
    │   ├── FriendRow.qml
    │   ├── PresenceDot.qml
    │   └── MediaControls.qml
    └── views/
        ├── NowView.qml
        ├── FriendsView.qml
        └── SettingsView.qml
```

The standalone app must import only upstream Quickshell and QtQuick modules.
The Omarchy plugin is a thin adapter around shared content and may import
Omarchy-specific modules only at that boundary. Each frontend process should
have one long-lived bridge to `wispd`.

Lazy-load larger views when not visible.

## Required States

Bar:

```text
󰍬
```

Friends around:

```text
󰍬 MemberA MemberB
```

Connected:

```text
🎙 MemberA MemberB      🖥 MemberB
```

Disconnected/error states must be visually distinct.

## IPC v1

Use versioned command/result/event envelopes.

Examples:

```json
{"v":1,"id":"a1","type":"command","name":"set_muted","args":{"muted":true}}
```

```json
{"v":1,"id":"a1","type":"result","ok":true}
```

```json
{"v":1,"type":"event","seq":42,"name":"self_state_changed","payload":{"muted":true}}
```

On connection:

```text
connect
  ↓
hello
  ↓
complete snapshot
  ↓
ordered incremental events
```

On reconnect, request another complete snapshot. Do not implement event replay yet.

### Phase 2 Exit Gate

- bar reflects daemon state;
- `Super+H` opens Wisp;
- standalone `FloatingWindow` works without Omarchy modules;
- Omarchy adapter reuses the portable content;
- UI actions generate real daemon commands;
- daemon restart reconnects automatically;
- Quickshell restart does not restart `wispd`;
- no polling;
- no permanent animation loop when idle.

---

# Phase 3 — `wisp-server` + SQLite

Implement:

```text
Axum
SQLite
WebSocket events
LiveKit token generation
in-memory connected-client registry
```

## Initial Durable Schema

Start small:

```text
users
circles
circle_members
hangouts
hangout_members
messages
schema_migrations
```

Presence may remain primarily in memory, with `last_seen_at` persisted.

Add Spots and device identities later.

## Initial API

A simple control API is sufficient:

```text
POST /v1/dev/session
GET  /v1/snapshot
GET  /v1/events
POST /v1/hangouts/join-friend
POST /v1/hangouts/leave
POST /v1/livekit/token
```

Use HTTP for commands and a WebSocket for pushed state/events.

## Hangout Mapping

Every Wisp hangout gets:

- opaque `HangoutId`;
- LiveKit room identifier;
- participant membership;
- access state.

Do not expose LiveKit room naming as a user-facing concept.

### Phase 3 Exit Gate

Using Owner's Quickshell UI plus simulators:

- all users show correct online/offline state;
- presence propagates;
- a hangout can be created;
- membership remains consistent;
- hangout ends when empty;
- database survives restart;
- stale sessions are cleaned up.

---

# Phase 4 — Voice MVP

This is the first milestone that should become genuinely usable.

Implement in this order:

1. join one fixed room;
2. publish microphone;
3. play remote audio;
4. mute;
5. deafen;
6. leave;
7. reconnect;
8. join by Wisp hangout;
9. join by friend.

## Explicit Client State

Use a real state machine:

```text
Offline
   ↓
ConnectingToServer
   ↓
Available
   ↓
Joining
   ↓
Connected
   ↓
Reconnecting
   ↓
Connected / Failed
```

Expose these states to Quickshell.

Do not reduce connection state to one boolean.

## Testing

Avoid microphone feedback during local multi-client testing.

`wisp-sim` should support:

```bash
wisp-sim --profile MemberA --publish-tone
wisp-sim --profile MemberB --publish-file test-voice.wav
wisp-sim --profile MemberC --silent
```

### Phase 4 Exit Gate

- one-click voice join;
- reliable two-user voice;
- reliable four-user voice;
- immediate mute/deafen;
- stable leave/rejoin;
- one-hour voice soak test;
- no continuously growing memory use;
- Quickshell restart does not interrupt voice;
- clear device and connection errors.

At this point, start using Wisp daily.

---

# Phase 5 — Wisp Social Model

Implement the UX that differentiates Wisp from Discord.

## Presence

```text
● Open
◉ Knock
○ Closed
◐ Away
```

Semantics:

- `Open`: friends may join directly.
- `Knock`: joining requires approval.
- `Closed`: unavailable for unsolicited joins.
- `Away`: informational inactive state.

## Join Friend

The ordinary action is:

```text
Join MemberA
```

not:

```text
Join room XYZ
```

Behavior:

```text
MemberA already has a hangout
        ↓
join it
```

Otherwise:

```text
MemberA is available and alone
        ↓
create ephemeral hangout
```

## Knock

A knock is an expiring control event, not a persistent text message.

Example:

```text
MemberA wants to hang out

[ Join ]    [ Later ]
```

## `NOW`

Aggregate active people by hangout:

```text
NOW

MemberA + MemberB        Factorio       JOIN
MemberC             ● open
```

Do not show MemberA and MemberB as unrelated active entries.

### Phase 5 Exit Gate

A user should understand the current social state of the whole friend group immediately without navigating channels, rooms, or servers.

---

# Phase 6 — Audio Quality

Start with a clean baseline before adding heavy neural cleanup.

## Audio Pipeline

Target:

```text
PipeWire / platform microphone
       ↓
echo cancellation
       ↓
high-pass filtering
       ↓
selected denoiser
       ↓
automatic gain control
       ↓
voice activity / level meter
       ↓
Opus 48 kHz
       ↓
LiveKit
```

## Initial Presets

### Natural

- echo cancellation;
- light suppression;
- gentle gain control.

### Clear

- echo cancellation;
- normal suppression;
- automatic gain control.

### Studio

- optional echo cancellation;
- suppression off;
- AGC off;
- higher-quality Opus;
- stereo where appropriate.

Only after baseline testing, consider:

### Aggressive

- stronger cleanup using DeepFilterNet or another appropriate real-time denoiser.

Do not stack multiple neural denoisers.

## Features

Implement:

- microphone selection;
- output selection;
- device hot-plug;
- push-to-talk;
- global mute/deafen;
- input level meter;
- active speaker state;
- per-user output volume;
- microphone test;
- processed/unprocessed comparison;
- advanced network/audio statistics hidden by default.

### Phase 6 Exit Gate

Manually validate:

- headset in quiet room;
- speakers with echo;
- mechanical keyboard;
- fan/air conditioner;
- USB microphone disconnect;
- Bluetooth device change;
- game running under load.

---

# Phase 7 — Screen Sharing and Camera

Do not build a Discord-style call grid.

## Surface Model

Quickshell sends commands such as:

```text
OpenShare(MemberB)
CloseShare(MemberB)
FullscreenShare(MemberB)
```

`wispd` owns:

- video track subscription;
- decoded frames;
- GPU textures;
- native media surfaces.

Voice must not depend on any media window.

## Implementation Order

1. publish synthetic video;
2. receive synthetic video;
3. render remote surface;
4. capture full monitor;
5. capture selected window;
6. stop/reselect capture;
7. region capture;
8. hardware encoding;
9. quality profiles;
10. camera.

Start conservatively:

```text
H.264
720p30
```

Then add:

```text
H.264 1080p60
AV1 1080p60 when hardware available
AV1 1440p60 after profiling
```

Make codec capability negotiation explicit.

## Subscription Behavior

Starting a share should only show:

```text
MemberB  🖥 sharing
```

Do not automatically open or decode it.

Only subscribe to the required quality when the user views the surface.

Expected policy:

```text
surface closed
    → unsubscribe / pause

surface hidden
    → pause

small PiP
    → lower-quality layer

fullscreen
    → highest useful layer
```

This is important for bandwidth, CPU, GPU usage, and the future RK1-hosted deployment.

## Camera

Camera is another optional track/surface.

Do not introduce a mandatory participant grid.

### Phase 7 Exit Gate

- one publisher and at least two viewers;
- hidden viewers do not decode full-resolution video;
- opening/closing video never interrupts voice;
- Hyprland can float/tile/fullscreen/pin the surface;
- screen and camera tracks coexist;
- capture failure is recoverable;
- hardware acceleration is detected rather than assumed.

---

# Phase 8 — Messages and Spots

Keep messaging intentionally small.

## Conversation Types

Support only:

```text
direct message
circle timeline
current hangout timeline
```

Do not add:

- channel trees;
- threads;
- forums;
- rich bot ecosystems;
- file hosting;
- message reactions initially.

## Persistence

Persist messages before acknowledging them.

Example shape:

```text
id
conversation_id
sender_id
created_at
content_type
payload
encryption_version
```

Offline clients should retrieve missed messages after reconnecting.

## Spots

Introduce one initial persistent Spot:

```text
TestRoom
```

A Spot is a saved hangout configuration, not an always-running room.

```text
TestRoom empty
    → not shown in NOW

TestRoom occupied
    → shown in NOW
```

### Phase 8 Exit Gate

- messages survive restart;
- offline message delivery works;
- hangout chat can use short retention;
- TestRoom behaves as a preset rather than a permanent voice channel;
- no channel hierarchy has appeared.

---

# Phase 9 — Private Alpha Hardening

Do not expose the development service to the Internet before this phase.

## Authentication

Use invite-driven device registration.

Suggested flow:

```text
admin creates one-time invite
        ↓
friend registers device
        ↓
device receives identity credential
        ↓
server issues short-lived sessions
```

Model every device separately so individual devices can be revoked.

## Media Privacy

Enable LiveKit end-to-end encryption before claiming that media is private from the server.

Do not invent a custom media cipher.

Initial key distribution may be simple for the private alpha, but keep the design ready for per-device encrypted key distribution later.

## Reliability

Add:

- systemd user service for `wispd`;
- server services;
- restart-on-failure;
- health checks;
- bounded exponential reconnect;
- database backup;
- restore test;
- structured error codes;
- log rotation;
- no message content in normal logs;
- protocol compatibility checks;
- migration safety.

## Soak Tests

Test:

- two-hour voice call;
- one-hour screen share;
- repeated join/leave;
- `wisp-server` restart;
- LiveKit restart;
- Quickshell restart;
- network off/on;
- microphone removal;
- monitor disconnect during sharing;
- SQLite backup while messages arrive.

---

# Phase 10 — LAN and Future RK1 Migration

This phase is not part of initial development, but the architecture must make it simple.

First test another machine on the LAN.

Only after real authentication exists should the local services bind beyond loopback.

Later move:

```text
wisp-server
livekit-server
SQLite
server secrets
```

to the RK1.

Keep:

```text
wispd
Quickshell
audio processing
capture
video rendering
```

on each desktop.

The migration should require changing configuration, not application architecture.

---

# Performance Targets

Treat these as targets to measure, not assumptions.

| Area | Target |
|---|---:|
| Hidden Quickshell window / closed plugin | no polling / continuous animation |
| Wisp panel opening | feels instant; aim `<100 ms` |
| Local voice join | aim `<2 s` |
| Mute command → audio path | aim `<100 ms` |
| `wispd` voice-only CPU | `<5%` of one modern desktop core |
| `wispd` voice-only RSS | `<200 MB`, stretch `<120 MB` |
| `wisp-server` idle CPU | effectively zero |
| Local message propagation | `<50 ms` |
| Hidden video surface | no full-resolution decode |
| Quickshell restart | zero voice interruption |

Record benchmark results throughout development.

---

# Testing Strategy

## Unit Tests

Cover:

- presence state;
- Open/Knock/Closed permissions;
- hangout state transitions;
- message ordering;
- migrations;
- protocol serialization;
- reconnect logic;
- invalid command handling.

## Integration Tests

Programmatically launch a temporary stack using random ports and a temporary database:

```text
LiveKit
wisp-server
Owner wispd
MemberA simulator
```

Test:

```text
connect
presence update
create hangout
issue token
join
publish synthetic audio
mute
message
leave
restart server
resync
```

## Manual Hardware Tests

Reserve manual testing for:

- PipeWire devices;
- echo cancellation;
- screen portal;
- camera;
- GPU encode/decode;
- multiple monitors;
- Hyprland surface behavior.

---

# Release Milestones

## M0 — Shell Prototype

Standalone Quickshell window plus optional Omarchy bar/panel driven by a fake `wispd`.

## M1 — Local Voice

Two local profiles communicate through local LiveKit.

## M2 — Wisp Social Alpha

`Open / Knock / Closed`, `NOW`, join-a-friend, ephemeral hangouts.

## M3 — Media Alpha

High-quality audio, screen sharing, camera, detachable native surfaces.

## M4 — Private Alpha

Messages, TestRoom, device invites, E2EE, reconnect hardening, backups.

---

# Scope Guardrails

Do **not** build the following during the initial implementation:

```text
TUI
non-Quickshell cross-platform GUI
Windows/macOS client
public communities
Discord-style server hierarchy
channel hierarchy
federation
P2P/SFU automatic switching
mobile apps
AI participants
file hosting
plugin marketplace
AV2
neural video codecs
custom cryptography
Redis
Kubernetes
multi-node LiveKit
complex role systems
```

Do not prematurely optimize control-plane serialization. JSON over localhost is insignificant compared with audio/video traffic and is easier to debug.

Do not introduce infrastructure merely because it may become useful at large scale.

Optimize for four friends and excellent usability first.

---

# First Complete Vertical Slice

The implementation should ultimately make this flow feel effortless:

```text
log into Omarchy
      ↓
Wisp is already available in the shell
      ↓
see friends in the bar
      ↓
press Super+H
      ↓
see MemberA + MemberB hanging out
      ↓
join them
      ↓
talk with excellent audio
      ↓
MemberB starts sharing
      ↓
Wisp shows "MemberB 🖥 sharing"
      ↓
open MemberB's stream as a native Hyprland surface
      ↓
close the Wisp panel
      ↓
continue hanging out normally
```

Protect this experience aggressively.

The goal is not to build a smaller Discord.

The goal is to make communication with close friends feel like a **native desktop primitive**.
