#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

if ! command -v pacman >/dev/null 2>&1; then
  echo "This bootstrap is for CachyOS and other Arch-based systems." >&2
  exit 1
fi

echo "Installing Wisp client build and runtime dependencies..."
sudo pacman -S --needed \
  base-devel rustup clang cmake pkgconf openssl alsa-lib libpulse pipewire \
  quickshell tailscale just jq curl git

sudo systemctl enable --now tailscaled
if ! tailscale status >/dev/null 2>&1; then
  echo "Tailscale needs authentication. Follow the URL from the next command."
  sudo tailscale up
fi

rustup toolchain install 1.97.0 --profile minimal
rustup override set 1.97.0
cargo build --locked --release -p wispd -p wispctl

bin_root=${XDG_BIN_HOME:-$HOME/.local/bin}
mkdir -p "$bin_root"
install -m 0755 target/release/wispd "$bin_root/wispd"
install -m 0755 target/release/wispctl "$bin_root/wispctl"
./scripts/app-sync.sh

echo "Wisp client installed. Ask the host for its Tailscale DNS name and your profile."
echo "Then run: just friend-config <host>.ts.net <Tyler|Jack|Charlie>"
echo "Future starts only need: just friend"
