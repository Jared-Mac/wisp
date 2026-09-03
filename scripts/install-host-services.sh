#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
bin_root=${XDG_BIN_HOME:-${HOME:?HOME is required}/.local/bin}
config_root=${XDG_CONFIG_HOME:-$HOME/.config}
unit_root="$config_root/systemd/user"
wisp_config="$config_root/wisp"

for binary in wisp-server; do
  [[ -x "$bin_root/$binary" ]] || {
    echo "$bin_root/$binary is missing; install a Wisp release first" >&2
    exit 1
  }
done
mkdir -p "$unit_root" "$wisp_config" "$bin_root"
install -m 0755 "$repo_dir/scripts/backup-database.sh" "$bin_root/wisp-backup"
install -m 0755 "$repo_dir/scripts/restore-database.sh" "$bin_root/wisp-restore"
for unit in wisp-server.service wisp-backup.service wisp-backup.timer; do
  install -m 0644 "$repo_dir/infra/local/$unit" "$unit_root/$unit"
done
if [[ ! -f "$wisp_config/server.env" ]]; then
  install -m 0600 "$repo_dir/infra/local/server.env.example" "$wisp_config/server.env"
  echo "Created $wisp_config/server.env; replace its example addresses and secrets."
fi
systemctl --user daemon-reload
systemctl --user enable --now wisp-backup.timer

echo "Installed Wisp host services."
echo "After configuring LiveKit and server.env, run:"
echo "  systemctl --user enable --now wisp-server.service"
