# Desktop IPC input limits

The desktop and Omarchy adapter use `quickshell/app/ipc-transport.py` between
the daemon's Unix socket and QML. Python 3 is required. The helper runs with
`python3 -I` and uses only the standard library; it does not invoke a shell.

Quickshell's `SplitParser` buffers an entire newline-delimited frame before
calling QML. Checking its `onRead` input alone cannot bound an unterminated
socket stream. The relay reads at most 16 KiB at a time and rejects a frame
as soon as it exceeds 8 MiB, before decoding UTF-8 or parsing JSON. It closes
the socket without forwarding the offending frame or logging its contents.

Before forwarding, the relay validates the envelope and snapshot structure,
limits nesting to 32 levels and the total object tree to 100,000 values,
and bounds every array, object, key and string. Limits include:

| Data | Maximum |
| --- | --- |
| Servers / server states | 16 each |
| Messages / reactions per collection | 2,048 each |
| Conversations / friends / members per collection | 512 each |
| Hangouts / spots per collection | 256 each |
| Devices / knocks / room invitations per collection | 128 each |
| Other arrays / object properties | 512 each |
| IDs / display names / content types | 256 characters |
| Names / labels | 512 characters |
| Other strings | 16 Ki characters |
| Text payloads / captions | 64 Ki characters |
| Image data URLs (avatars, emojis, invitation QR images) | 1 Mi characters |

The 8 MiB encoded frame limit applies in addition to all per-field limits,
including nested snapshots. Chat attachments use local files, so their file
size is independent of this transport budget. UTF-8 is decoded only after
the complete bounded frame arrives; arbitrary byte splits preserve Unicode.

The relay waits for QML to acknowledge each frame before reading more socket
data. This applies backpressure and bounds data waiting in the shell's pipe.
Commands continue to flow while a response awaits acknowledgement. Command
lines have a separate 256 KiB ceiling. EOF discards incomplete frames; retries
start with empty buffers. Invalid input leaves the UI disconnected until it
is restarted, avoiding repeated automatic processing of a hostile response.

The daemon/server protocol is unchanged and existing clients remain compatible.
No Android or server migration is required. Normal connection failures still
retry. The relay never sends a join, mute, camera, or screen-share command.

Transfer progress also has a lifetime limit in QML: at most 128 upload/download
entries are retained, with the least recently updated evicted first. Entries
expire after five minutes without an update, or five seconds after completion
(including empty transfers). A one-second timer removes expired entries even
when no new events arrive, and disconnecting clears the map. Repeated completion
events do not extend an existing completion deadline. Only nonnegative safe
integer byte counters and a local expiry time are retained; IDs are limited to
256 characters, directions to upload/download, and extra payload fields are
discarded. These rules bound persistent state across any number of valid frames.

Validation: `python3 -m unittest discover -s tests -p 'test_ipc_transport.py'`.
The tests exercise a real Unix socket, an exact-limit frame, overflow without
a newline, fragmented UTF-8, coalesced frames, acknowledgement backpressure,
malformed/oversized objects, EOF, command limits, and the QML transport when
Quickshell is available. CI always runs the Python checks.
`node scripts/test-transfer-progress.cjs` covers repeated active/completed events,
eviction, expiry, field projection and invalid counters in CI.
`bash scripts/test-transfer-progress.sh` exercises repeated events through the
real QML bridge, progress labels, disconnect cleanup and timer-based expiry.
