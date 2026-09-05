# LiveKit WebRTC audio extensions

`libwebrtc` 0.3.46 and `webrtc-sys` 0.3.43 retain their upstream licenses.
Wisp's audio extensions are:

- `RtcAudioTrack::set_playout_volume` → native source `SetVolume` (0–2), keeping
  playback on the WebRTC mixer and its device routing.
- A render preprocessor on the singleton peer connection factory, plus
  `AudioProcessingModule::use_playout_reference`, connecting that mixer to the
  explicit APM used for Wisp's external microphone capture. Reference storage is
  fixed and bounded; the callback never modifies playback or blocks on a lock.
- `enable_high_noise_suppression`, allowing a lightweight fallback without
  replacing AEC history, and `playout_reference_frames` for transport validation.

See [the speech pipeline](../docs/audio-pipeline.md) for processing order,
reference lifetime, scope, and verification. NativeAudioSource bypasses the ADM
capture processor; source audio-option flags alone cannot apply AEC to it.

When upgrading LiveKit, reapply these extensions to matching locked binding
versions or replace them with equivalent upstream APIs. The render tap depends
on WebRTC's `CustomProcessing` contract (10 ms float samples in S16 units).
