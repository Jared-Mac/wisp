#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=1.13.6

case "$(uname -m)" in
  x86_64)
    archive="livekit_${version}_linux_amd64.tar.gz"
    expected="2b61abef2b9ba14b4b8ca38b37de9a37ffc682b9931d5fc03ceca2f0b77d3e33"
    ;;
  aarch64|arm64)
    archive="livekit_${version}_linux_arm64.tar.gz"
    expected="5c75f09173199f3f8fe0c3c0d5a41171f9b306ffce6843d752c276d11e77d19b"
    ;;
  *)
    echo "unsupported architecture for the LiveKit development binary: $(uname -m)" >&2
    exit 1
    ;;
esac

install_dir="$repo_dir/.tools/livekit-$version"
binary="$install_dir/livekit-server"
if [[ -x "$binary" ]]; then
  echo "$binary"
  exit 0
fi

temp_dir=$(mktemp -d)
cleanup() {
  rm -rf -- "$temp_dir"
}
trap cleanup EXIT

url="https://github.com/livekit/livekit/releases/download/v${version}/${archive}"
curl --fail --location --silent --show-error "$url" --output "$temp_dir/$archive"
actual=$(sha256sum "$temp_dir/$archive" | cut -d' ' -f1)
if [[ "$actual" != "$expected" ]]; then
  echo "LiveKit checksum mismatch: expected $expected, got $actual" >&2
  exit 1
fi

mkdir -p "$install_dir"
tar -xzf "$temp_dir/$archive" -C "$install_dir" livekit-server
chmod 0755 "$binary"
ln -sfn "livekit-$version" "$repo_dir/.tools/livekit"
echo "$binary"
