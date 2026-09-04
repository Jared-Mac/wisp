#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/bin"
install -m 0755 "$repo_dir/tests/fakes/cmake" "$test_dir/bin/cmake"
# A fresh target directory ensures Cargo actually rebuilds, not just a cache hit.
PATH="$test_dir/bin:$PATH" CMAKE="$test_dir/bin/cmake" CARGO_TARGET_DIR="$test_dir/target" \
  cargo build --manifest-path "$repo_dir/Cargo.toml" --locked --release -p wisp-video
nm -D --defined-only "$test_dir/target/release/libwispvideo.so" >"$test_dir/symbols"
rg -q ' T qt_plugin_instance$' "$test_dir/symbols"
rg -q ' T qt_plugin_query_metadata_v2$' "$test_dir/symbols"
echo 'Clean Cargo video-plugin build and Qt loader exports passed with CMake disabled'
