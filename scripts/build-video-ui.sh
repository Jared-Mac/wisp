#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
cargo build --manifest-path "$repo_dir/Cargo.toml" --locked --release -p wisp-video
# Stable staging location used by local UI sync, fixtures, and release packaging.
# The plugin itself is a Cargo cdylib, including its small compiled Qt adapter.
mkdir -p "$repo_dir/target/video-ui"
install -m 0755 "${CARGO_TARGET_DIR:-$repo_dir/target}/release/libwispvideo.so" "$repo_dir/target/video-ui/libwispvideo.so"
install -m 0644 "$repo_dir/native/video/qmldir" "$repo_dir/target/video-ui/qmldir"
