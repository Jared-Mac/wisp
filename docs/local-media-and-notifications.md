# Local media, attention and sound controls

- Saved rooms stay in one list, with **#name /count** above a compact participant row. Clicking
  a room opens chat; **[join]** on its row or **Join voice** in the chat header
  starts voice explicitly.
  **[+]** beside Rooms opens creation. Temporary calls appear beside Friends.
- The current-call area below the room list shows the voice server and room, with screen share,
  camera, **Invite**, and **[d/c]**. Its label and actions keep the voice server’s context while browsing
  another server. Room access invitations remain in Room settings.
- Tray chat headings use the conversation's assigned color and name. Unread
  shortcuts open the indicated chat; the return shortcut restores the previous chat.
- Friend/room context menus adjust **your** playback gain, 0–200%. Preferences
  are keyed by account and stable user ID in `~/.config/wisp/participant-volumes.json`.
  They apply to current tracks and newly subscribed/reconnected tracks. They do
  not change anyone's microphone or another listener's audio.
- Speaking indicators combine LiveKit activity with measured audio levels and a
  700 ms release hold. Muting and leaving take precedence over that hold.
- Watched screens/cameras join the main split tree when the main window is open.
  Otherwise they open separately. Pop-out, anchor, drag and resize are local UI
  operations. Closing a pop-out anchors it; the × inside the stream stops watching.
  Closing the main window pops its watched streams out rather than hiding them.
  **Settings → Audio / Video** can make new streams always open separately.
  Streams are never persisted as auto-watch subscriptions.
- **Settings → Notifications & Chat** offers other-chat (default), app-unfocused,
  and all-message sound policies. Each conversation can be muted from its chat
  options or Settings. Muting is local, persists, and follows that conversation
  across tiles/pop-outs; unread messages/badges are retained.
- Four room cues distinguish others joining/leaving from your own joins/leaves.
  Each has a local custom-file picker, test and restore-default action. Global
  mute/volume and separate own/other room-event toggles apply. Initial snapshots
  and reconnect snapshots don't replay room or message sounds. Sound preferences
  are in `~/.config/wisp/notifications.json`; selected custom files must remain
  available at their chosen paths.
- Settings tabs: Profile, Audio / Video, Appearance, Notifications & Chat, Privacy,
  Devices, and Server (for owners/admins).

## Native viewer/build

`cargo build --release -p wisp-video` builds the Rust-owned video receiver and
small Qt Quick scene-graph adapter as a QML plugin. Cargo invokes Qt6 `moc` and
the C++ compiler through `cxx-build`; **CMake is not used**. Source builds require
Qt6 development packages, pkg-config, and a C++ compiler. The helper
`bash scripts/build-video-ui.sh` stages Cargo's output for `app-sync.sh`, which
installs it under the installed UI's `native/WispVideo`. `wisp-ui` sets the
physical QML import and sound-asset paths (Quickshell virtual URLs cannot load
native libraries or serve as external audio-player filenames). Release packaging
includes the built module; CI installs Qt6 development packages and builds it.
People installing that precompiled package do not need the compiler, moc, Cargo,
or CMake, but still need compatible Qt/Quickshell runtime libraries.

Rust owns the Unix socket, bounded frame validation/storage, cancellation on
source change/destruction, viewport requests, and aspect-fit geometry. The C++
adapter is limited to Qt properties, a timer that polls Rust, texture upload, and
plugin registration. The socket worker never touches Qt objects or invokes GUI
callbacks; it exchanges at most one pending frame with the UI thread. Old-source
workers have separate state and cannot overwrite a new stream. The only
handwritten unsafe Rust is two Qt plugin-loader ABI forwarding functions;
receiver code forbids unsafe, and the existing workspace policy is unchanged.

The daemon's private mode-0600 `wispd.video` Unix socket transports decoded RGBA,
with one requested frame in flight and latest-frame replacement. No media files,
recompression or network listener are used for UI rendering. Closing a viewer
connection releases its watching subscription. Legacy native viewers remain
available to older clients. The Omarchy adapter delegates watch actions to the
standalone desktop host without changing its host-provided styling.

The two pinned WebRTC binding patches in `third_party/` expose native per-track
output gain so playback stays on WebRTC's mixer/AEC/device path. No server API,
migration or remote deployment is required.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `node scripts/test-chat-logic.cjs`
- `bash scripts/test-chat-ui.sh`
- `bash scripts/test-settings-access.sh`
- `bash scripts/test-local-controls.sh` (set `NODE` if Node isn't on PATH)
- `QT_SCALE_FACTOR=1.25 WISP_TEST_WIDTH=840 bash scripts/test-local-controls.sh`

Local controls tests use synthetic RGBA frames, fake sound playback and isolated
preferences. They never join a room, publish media, write the system clipboard,
or contact a real Wisp account. Actual call quality, per-person loudness, GPU
performance under simultaneous high-resolution streams, and a real Omarchy
host still need user testing.

Participant names open a menu with local volume, local mute, and a direct message
in a new tile. Local mute preserves the volume slider value and is scoped to the
server/account/person; self deafen remains an all-audio control. Server mute and
server deafen require server administration and are enforced with LiveKit
participant permissions and subsequent join tokens. Server mute disables audio
publication; server deafen also suspends receiving media. Camera and screen video
publishing remain available. Removing a restriction preserves manual mute/deafen
choices and restores an already-authorized microphone without joining a room.
Clients predating server moderation must update to restore microphone publishing
after server unmute; older clients need to reconnect. Participant indicators use
shield badges for server mute/deafen and a separate crossed-out speaker for local
mute, with explicit hover text. Both remain visible when restrictions overlap.
The disconnect control is labeled **[d/c]** beside the single-line room and
connection status, with matching typography and an accessible descriptive name.
Share, camera, and invite remain underneath.

Channel and saved-room clicks open another chat tile by default. This can be
changed in Settings → Notifications & Chat. With the setting off, navigation
reuses the active channel/room-chat tile or another channel tile; it never
replaces a DM, private group chat, or video tile. If none is available, a new tile
is opened. Existing copies of the selected chat are focused without duplicates.
The explicit + action always requests a tile. At the eight-tile limit, Wisp
explains that a tile must be closed instead of replacing an unrelated chat.
The Omarchy panel forwards the requested behavior to the desktop workspace.

The local camera preview decodes Qt byte-array camera IDs byte by byte to match
V4L2 device paths. It still requires an explicit preview and confirmation before
publishing, and releases local capture on cancel or target/device changes.
