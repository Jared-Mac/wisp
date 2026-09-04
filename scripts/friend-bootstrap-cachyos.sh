#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

if ! command -v pacman >/dev/null 2>&1; then
  echo "This bootstrap is for CachyOS and other Arch-based systems." >&2
  exit 1
fi

echo "Installing Wisp client build and runtime dependencies..."
portal_backend=xdg-desktop-portal-gtk
case "${XDG_CURRENT_DESKTOP:-}" in
  *Hyprland*) portal_backend=xdg-desktop-portal-hyprland ;;
  *KDE*) portal_backend=xdg-desktop-portal-kde ;;
  *GNOME*) portal_backend=xdg-desktop-portal-gnome ;;
esac

sudo pacman -S --needed \
  base-devel rustup clang pkgconf qt6-base qt6-declarative openssl alsa-lib libpulse pipewire \
  gst-plugin-pipewire gst-plugins-base gst-plugins-good gst-libav \
  xdg-desktop-portal "$portal_backend" \
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

echo "Wisp client installed. Ask the host for its Tailscale name, a one-use invite,"
echo "your profile, and the private media key. Then run:"
echo "  just friend-register <host>.ts.net <Tyler|Jack|Charlie>"
echo "Future starts only need: just friend"
