#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
package_dir="$repo_dir/examples/plugins/shared-modpacks"
cargo build --manifest-path "$package_dir/Cargo.toml" --locked --release --target wasm32-unknown-unknown
mkdir -p "$repo_dir/target/plugins/games.valheim"
install -m 0644 "$package_dir/plugin.json" "$repo_dir/target/plugins/games.valheim/plugin.json"
install -m 0644 "$package_dir/target/wasm32-unknown-unknown/release/wisp_valheim.wasm" "$repo_dir/target/plugins/games.valheim/plugin.wasm"
printf 'Plugin package: %s\n' "$repo_dir/target/plugins/games.valheim"
