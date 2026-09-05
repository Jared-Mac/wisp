# Speech pipeline

Wisp owns microphone processing explicitly in `apps/wispd/src/audio.rs`.
GStreamer captures one selected device, converts/resamples to 48 kHz mono PCM,
and assembles exact 480-sample frames. A single dedicated worker processes the
frames; LiveKit transports them with Opus (64 kb/s ceiling, DTX and RED enabled).
Playback stays on WebRTC's native mixer, including per-person volume and deafen.

## Processing modes

- **Clear voice** (`clear`, default): WebRTC AEC3 with the speaker mix as its
  reference, high-pass rumble removal, embedded DeepFilterNet3, then peak
  protection. Both trained neural stages run on every frame; the runtime SNR
  shortcuts that skip reconstruction or force silence are disabled. DeepFilterNet adds 30 ms of algorithmic lookahead.
- **Light cleanup** (`natural`): AEC3, high-pass filtering, WebRTC's moderate noise
  suppression, and peak protection. No neural inference.
- **Unprocessed** (`studio`): bit-exact PCM bypass after device conversion. No AEC,
  filtering, noise suppression, or limiter. Use headphones in a quiet space.

There is no additional speech detector/gate or automatic gain boost. The peak
limiter only attenuates peaks above 0.95 full scale, recovering over approximately
100 ms. It cannot repair distortion already clipped in the microphone hardware.

If DeepFilterNet fails to load/process, or exceeds its 10 ms compute budget for
eight consecutive frames after a 20-frame warmup, Clear switches to WebRTC's
high noise-suppression setting for the rest of the call. This reuses the AEC
history and adds no neural lookahead. It is reported as `denoiser: "webrtc"`.
A failed neural frame emits silence instead of a burst of untreated microphone
noise. A new explicit join retries full-quality processing. RNNoise and its
exclusive dependencies have been removed; the lightweight path uses the same
WebRTC processor as Light cleanup.

## Why the speaker reference is explicit

LiveKit's `NativeAudioSource` delivers samples directly to encoder sinks and
bypasses the ADM capture path. Enabling processing flags on that source does
not run those samples through the factory APM. Wisp disables source flags and
runs APM before neural processing itself.

The vendored WebRTC binding installs a render preprocessor on the singleton RTC
factory. It copies the native speaker mix, downmixed to mono, into fixed-size
reference storage for each subscribed external APM. It never modifies playback.
The render callback uses try-locks and drops reference frames on contention;
it performs neither neural work nor per-frame heap allocation. References are
capped at ten frames and expire after 100 ms. The speech worker drains them into
APM's reverse stream before processing microphone input. AEC3 estimates acoustic
and device delay from that reference. This reference covers Wisp playback;
unrelated applications' audio is not included.

## Continuous speech correction

DeepFilterNet's Rust runtime defaults are compute-saving heuristics: below
-10 dB estimated local SNR they force a zero mask; above 20 dB they retain the
attenuating mask but skip deep-filter reconstruction; above 30 dB they bypass
both stages. Crossings were causing abrupt cuts during continuous speech.
Wisp now always runs both trained stages. Neural masks still suppress noise;
there is no dry-signal mix or automatic gain boost added by this correction.

An offline comparison of the same local speech capture isolated the drops to
the neural stage. In the affected 1.5-second region, original/APM/old/new levels
were approximately -13.0/-13.3/-18.2/-13.1 dBFS. These are sample-specific energy
measurements, not a listening-quality score. The fixed pipeline's deterministic
stationary-noise test measured 43.3 dB attenuation while retaining its existing
30 ms lookahead and realtime-overload fallback. Media integration accepts either
backend with its matching latency; CPU contention during a build can legitimately
activate the fallback.

## Bounded lifetime and latency

The capture queue holds at most 60 ms across complete and partial frames.
Overflow drops the oldest complete audio. Frames older than 60 ms are rejected
both before processing and after the worker await. This bounds Wisp's pending
capture audio, not total microphone-to-listener latency, which also includes
device buffers, neural lookahead, codec packetization, network and jitter buffer.

Microphone switches activate a new source ID only after the replacement starts;
callbacks from the old device are ignored. The queue epoch invalidates partial,
queued and in-flight speech on device changes, mute transitions, or preset
changes. Worker state is rebuilt from a pristine model template across those
boundaries and capture discontinuities. Muted audio is replaced with silence
without neural processing. Leaving releases the echo reference and DSP history.
A three-second capture watchdog reports a stopped microphone as a visible media
failure requiring rejoin. It does not automatically join or reopen a room.

## Hear yourself

Audio settings includes an explicit local microphone test outside voice rooms.
Record up to eight seconds, finish early if desired, then replay the original
capture or the selected processing mode through the selected speaker. The test
uses the same capture conversion and dedicated speech worker as calls. It closes
capture before playback, avoiding a live monitoring feedback loop. Playback
queues individually timestamped 10 ms buffers; large recording-sized buffers
can be silently dropped by PipeWire sinks. This previews
microphone processing, not Opus/network effects or a room's acoustic echo.

PCM stays in bounded memory; no recording file, upload, room, or publication is
created. Recording stops automatically at eight seconds, with a three-second
no-input watchdog and twelve-second overall timeout. Stop cancels playback.
Closing the settings view, changing device/mode, or joining a room stops the test
and discards its sample. Call mute/deafen and push-to-talk preferences are not
changed. Clicking Record explicitly opens the test microphone even if call mute
is enabled.

The local IPC command `audio_test` accepts `action`: `status`, `record`, `stop`,
`play_original`, `play_processed`, or `clear`. It returns only phase, duration,
meter, preset, playback selection, and error metadata. PCM is never returned.
`wispctl audio test <action>` provides the same interface. Run
`scripts/test-audio-test.sh` for generated capture and silent virtual-speaker
playback checks, including measured nonzero PCM at the isolated sink monitor
for both playback buttons, and `scripts/test-audio-settings.sh` for the UI controls.

## Verification

`cargo test -p wispd --bin wispd audio::tests -- --nocapture` checks arbitrary
capture chunking, overload/expiry, old-device callbacks, mute-era invalidation,
bit-exact bypass, peak protection, recurrent-state reset, sustained overload
fallback, stationary-noise suppression, and synthetic delayed-echo cancellation.

`bash scripts/test-voice-reliability.sh` uses generated microphone and remote
tones on isolated local servers. It verifies actual speaker-reference delivery,
bounded capture depth, participant mixing, leave/rejoin, and LiveKit reconnects.
`bash scripts/test-media.sh` also checks input/output and processing-mode changes.
The local application is never joined to a room for these tests.

These are deterministic signal and transport checks, not a speech-quality score
or a promise of silent output for every noise source. Real microphone/headset
listening remains necessary for assessing consonants, double-talk, keyboard
noise, reverberation, and acoustic echo under different speaker volumes.

## Validation of this rework

- Audio engine: 12 tests passed; synthetic AEC reduced a 40 ms delayed speaker
  signal by 50.5 dB, and the lightweight fallback reduced stationary white noise
  by 20.3 dB. DeepFilterNet reduced that noise fixture below PCM quantization.
  These figures describe only the generated fixtures, not real-world speech.
- Workspace tests passed (124 tests before the additional audio-failure case);
  the final daemon suite passed 62 tests, including failure latching.
- Four-user voice test passed three leave/rejoin cycles and relay recovery;
  observed resident-memory growth was about 5 MiB.
- Full media integration passed, including actual reference delivery, device
  switches, mode changes, mute/PTT, and generated-tone transmission in Studio.
- Audio settings interaction/render checks and the settings-access suite passed.
- The existing broad `test-local-controls.sh` suite still fails its chat/video
  fixture checks. The same failures were reproduced with the pre-rework audio
  view and bridge copy; they are outside this audio change.

Production verification found schema 21, matching migration checksums, database
integrity and foreign keys, active Wisp/LiveKit/Caddy services, a matching running
server executable, and healthy public/local endpoints. This rework changes only
client processing and additive local audio telemetry; server application code,
migrations, and its dependency graph require no deployment delta.
