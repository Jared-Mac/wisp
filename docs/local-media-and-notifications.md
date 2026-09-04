# Local media, attention and sound controls

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
- Settings tabs: Audio / Video, Appearance, Notifications & Chat, Devices & Privacy.

## Native viewer/build

`bash scripts/build-video-ui.sh` builds the small Qt Quick scene-graph renderer
with CMake and Qt6 Quick/Network/QML development packages. `app-sync.sh` runs this
and installs it under the installed UI's `native/WispVideo`. `wisp-ui` sets the
physical QML import and sound-asset paths (Quickshell virtual URLs cannot load
native libraries or serve as external audio-player filenames). Release packaging
includes the built module; CI installs Qt6 development packages and builds it.

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
- `bash scripts/test-local-controls.sh` (set `NODE` if Node isn't on PATH)
- `QT_SCALE_FACTOR=1.25 WISP_TEST_WIDTH=840 bash scripts/test-local-controls.sh`

Local controls tests use synthetic RGBA frames, fake sound playback and isolated
preferences. They never join a room, publish media, write the system clipboard,
or contact a real Wisp account. Actual call quality, per-person loudness, GPU
performance under simultaneous high-resolution streams, and a real Omarchy
host still need user testing.
