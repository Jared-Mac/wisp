# Cargo-built video tile plugin

Build: `cargo build --locked --release -p wisp-video` from the workspace.
Output: `target/release/libwispvideo.so` (or `$CARGO_TARGET_DIR/release`).
Stage for the existing UI/release tools: `bash scripts/build-video-ui.sh`.

- `src/receiver.rs`: safe Rust socket I/O, bounded latest-frame storage,
  viewport negotiation, validation, and cancellation. No Qt callbacks from workers.
- `src/lib.rs`: Rust aspect-fit geometry and `cxx` bridge; two narrow exported
  entry points forward Qt's plugin ABI. Metadata layout is compile-time checked
  against Qt in the adapter and smoke-tested in Rust and Quickshell.
- `src/qt.cpp`: Qt-only property/texture/plugin glue. This file contains no
  transport, frame parsing, buffer queue, or worker lifecycle implementation.
- `build.rs`: finds Qt6 via pkg-config, runs Qt6 moc, and compiles the bridge
  through Cargo's cxx-build dependency. Set `QT_MOC` for an explicit moc path.

No CMake is used. Qt remains C++, so source builds still need its development
headers/tools and a C++ compiler. Precompiled packages contain this plugin and
only need the compatible Qt and Quickshell runtimes, not build tools. The plugin
must be built against a Qt version compatible with the installed Quickshell.

Tests: `cargo test -p wisp-video`, `cargo clippy -p wisp-video --all-targets -- -D warnings`,
`bash scripts/test-video-cargo.sh` (fresh build with CMake disabled), and
`NODE=node bash scripts/test-local-controls.sh`. The integration fixture
loads the actual dynamic plugin and supplies synthetic ultrawide frames over a
private local socket. It does not join rooms or activate capture.
