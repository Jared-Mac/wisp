# Wisp protocol v1

Local IPC is newline-delimited JSON on `$XDG_RUNTIME_DIR/wisp/wispd.sock`.
The socket is mode `0600`. Every client command has a version and request ID:

```json
{"v":1,"id":"a1","type":"command","name":"set_muted","args":{"muted":true}}
```

Results are request-scoped:

```json
{"type":"result","v":1,"id":"a1","ok":true,"value":{"muted":true}}
```

Snapshots are complete and events are ordered by `seq`:

```json
{"type":"event","v":1,"seq":42,"name":"self_state_changed","payload":{"snapshot":{}}}
```

Supported commands are `hello`, `status`, `set_presence`, `join_friend`,
`join_hangout`, `leave`, `set_muted`, `toggle_muted`, `set_deafened`,
`toggle_deafened`, `respond_knock`, `share`, `camera`, `watch_video`,
`refresh_audio_devices`, `set_input_device`, `set_output_device`,
`set_audio_preset`, `refresh_video_devices`, `set_camera_device`,
`set_video_quality`, `set_video_codec`, `set_push_to_talk`,
`set_push_to_talk_shortcut`, `push_to_talk_press`, and
`push_to_talk_release`. The legacy `open_surface` and `close_surface` commands
target the first available video publication. Device commands take a stable
device `id`; audio presets are `natural`, `clear`, or `studio`. Repeating
`push_to_talk_press` renews the daemon-owned lease without emitting another
state event. Invalid versions, JSON, or commands return a structured error and
do not terminate the IPC connection.

`respond_knock` takes a `knock_id` and a response of `accept` or `later`.
Pending incoming knocks are included in the snapshot's `knocks` array with the
sender and expiration time. Joining a friend in Knock presence returns a
`knock_sent` result instead of treating the request as an error.

The snapshot's media state includes `remote_audio_participants`,
`remote_video_participants`, per-track `remote_videos`, receive/render counters,
native-surface state, and optional `error_code`/`error` fields. A remote video
publication makes Watch available but remains unsubscribed until requested.
Each remote screen/camera entry reports its target, MIME type, simulcast
capability, subscription, requested layer, dimensions, and frame counters. The
nested `camera` object contains capture devices, selection, publication state,
dimensions, and recoverable errors. The nested `video` object contains the
publishing quality, codec, discovered encoder backends, and whether hardware
acceleration is actually active. Error codes are stable machine-readable
categories; `error` remains suitable for direct display. The nested `audio`
object contains input/output inventories, selected device IDs, the processing
preset, and an integer `input_level` from 0–100. While a call is active, `wispd`
detects device changes, switches to an available fallback, and restores the
preferred device if it returns.

The self state contains `push_to_talk.enabled`, `push_to_talk.active`, the
optional global `shortcut`, its `shortcut_backend`, and any descriptions in
`shortcut_replaced`. `set_push_to_talk_shortcut` accepts a normalized shortcut
string or `null` to clear it. Omarchy/Hyprland configuration is generated and
validated by `wispd`, never by the presentation layer.
Manual `muted` state always takes precedence. A PTT press expires after 30
seconds unless renewed and is also released on room changes or media reconnect.
`media.active_speakers` contains sorted participant display names from LiveKit.

The server control API uses short-lived development sessions during local work.
`GET /v1/events?token=...` is a WebSocket. Other `/v1` routes use a Bearer token.
Development credentials never enter Quickshell.
