#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=${1:-}
target_dir=${CARGO_TARGET_DIR:-$repo_dir/target}

if [[ ! "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
  echo "usage: $0 <version-or-commit>" >&2
  exit 2
fi

case "$(uname -m)" in
  x86_64) architecture=x86_64 ;;
  aarch64|arm64) architecture=aarch64 ;;
  *)
    echo "unsupported release architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

for binary in wispd wispctl wisp-server; do
  if [[ ! -x "$target_dir/release/$binary" ]]; then
    echo "missing release binary: $target_dir/release/$binary" >&2
    echo "run: cargo build --workspace --release --locked" >&2
    exit 1
  fi
done

package_name="wisp-$version-linux-$architecture"
dist_dir="$repo_dir/dist"
staging_dir=$(mktemp -d)
package_dir="$staging_dir/$package_name"

cleanup() {
  rm -rf -- "$staging_dir"
}
trap cleanup EXIT INT TERM

mkdir -p \
  "$package_dir/bin" \
  "$package_dir/infra/local" \
  "$package_dir/scripts"

for binary in wispd wispctl wisp-server; do
  install -m 0755 "$target_dir/release/$binary" "$package_dir/bin/$binary"
done

cp -a "$repo_dir/quickshell" "$package_dir/quickshell"
for script in \
  app-sync.sh \
  backup-database.sh \
  configure-friend.sh \
  friend-tailscale.sh \
  install-host-services.sh \
  register-friend-device.sh \
  restore-database.sh \
  plugin-sync.sh \
  wisp-launch.sh \
  wisp-ui.sh \
  wisp.sh; do
  install -m 0755 "$repo_dir/scripts/$script" "$package_dir/scripts/$script"
done
for asset in \
  dev.wisp.desktop \
  dev.wisp.service \
  dev.wisp.surface.desktop \
  wisp-backup.service \
  wisp-backup.timer \
  wisp-server.service; do
  install -m 0644 "$repo_dir/infra/local/$asset" "$package_dir/infra/local/$asset"
done
install -m 0600 "$repo_dir/infra/local/server.env.example" "$package_dir/infra/local/server.env.example"
install -m 0755 "$repo_dir/scripts/install-release.sh" "$package_dir/install.sh"
install -m 0644 "$repo_dir/LICENSE" "$repo_dir/README.md" "$package_dir/"

mkdir -p "$dist_dir"
archive="$dist_dir/$package_name.tar.gz"
tar -czf "$archive" -C "$staging_dir" "$package_name"
(
  cd "$dist_dir"
  sha256sum "$(basename -- "$archive")" >"$(basename -- "$archive").sha256"
)

echo "Created $archive"
echo "Created $archive.sha256"
