# LiveKit WebRTC gain extension

`libwebrtc` 0.3.46 and `webrtc-sys` 0.3.43 are copied from their locked
crates.io sources, retaining upstream licenses. The only functional patch is
`RtcAudioTrack::set_playout_volume` → `AudioTrack::set_playout_volume` →
WebRTC `AudioSourceInterface::SetVolume` (0–2). This leaves audio on the native
WebRTC mixer and preserves its echo-cancellation reference and device routing.
No capture gain, transmitted audio, or other listener is changed.

When upgrading LiveKit, replace these vendored sources with the matching locked
versions and reapply the small gain API, or remove the patches if upstream adds it.
