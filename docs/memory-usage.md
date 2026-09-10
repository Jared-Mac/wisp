# Client memory usage

The client keeps speech processing and settings allocation tied to use:

- The speech worker starts without a DeepFilterNet model. Joining voice or
  recording a local test warms it before capture starts. Disconnecting, failed
  joins, test completion, and test cancellation release the model, its pristine
  reset template, and echo-cancellation state. No model unloading occurs during
  an active call, including while muted. Audio thresholds, processing algorithms,
  sampling rate, and quality settings are unchanged.
- Closing a speech session rejects late frames until another session explicitly
  starts. Reopening constructs clean state so earlier speech cannot replay.
- Settings are created on first opening; individual sections are created on first
  visit. Visited sections stay instantiated to preserve local form drafts and
  existing tab behavior. Search creates its destination before scrolling to it.
- Chat image previews decode at approximately their displayed physical pixel
  size. Widths are rounded up in 128-pixel buckets to avoid decoding again for
  every pixel of window resizing. Original dimensions determine layout, and the
  separate viewer and clipboard still use the original image. The viewer disables
  the decoded-image cache so closing it does not leave a full-size cached copy.
  See Qt's [Image sourceSize documentation](https://doc.qt.io/qt-6/qml-qtquick-image.html#sourceSize-prop).

These changes release object ownership; the system allocator can retain freed
pages for reuse. Resident memory need not immediately return to its cold-start
value, and shared libraries, GPU buffers, and the Omarchy shell contribute to
process totals independently.

## Validation and measurement

```sh
cargo test -p wispd --release
cargo test -p wispd --release audio::tests::speech_worker_memory_probe -- --ignored --nocapture
WISP_TEST_BUILD_PROFILE=release bash scripts/test-audio-test.sh
QT_QPA_PLATFORMTHEME=basic WISP_TEST_MODES='memory imagegeometry panelimagegeometry' bash scripts/test-chat-ui.sh
QT_QPA_PLATFORMTHEME=basic bash scripts/test-streams-and-search.sh
```

The ignored memory probe runs in a fresh test process with synthetic audio. It
reports RSS, PSS, private dirty pages, swap, and model warmup time before startup,
while idle, during processing, and after cleanup. It never accesses a microphone,
opens a playback device, or joins a room. Compare the same release profile on the
same machine; don't sum RSS across processes because it double-counts shared pages.
The recording/playback integration test uses a generated tone and its own virtual
sink, rather than the user's microphone or speakers.

A local before/after comparison on 2026-09-07 measured an idle speech-worker RSS
reduction from 52,312 to 13,360 KiB. Model preparation on first use took 89 ms in
that run, before capture. Identical isolated 933×900 Performative/Astra UI fixtures
measured 279,244 → 267,072 KiB for ordinary chat startup and 389,512 → 247,592 KiB
with one 7680×4320 image preview. These are workload-specific measurements, not
fixed budgets or predictions for an established live session.
