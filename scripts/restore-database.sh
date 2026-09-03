#!/usr/bin/env bash
set -euo pipefail

source_backup=${1:-}
if [[ -z "$source_backup" ]]; then
  echo "usage: $0 <backup.sqlite3>" >&2
  exit 2
fi
command -v sqlite3 >/dev/null 2>&1 || {
  echo "sqlite3 is required for Wisp restore" >&2
  exit 1
}
if [[ ! -f "$source_backup" ]]; then
  echo "backup does not exist: $source_backup" >&2
  exit 1
fi
if [[ $(sqlite3 "$source_backup" 'PRAGMA integrity_check;') != "ok" ]]; then
  echo "refusing to restore a database that failed integrity_check" >&2
  exit 1
fi
if [[ ${WISP_RESTORE_OFFLINE_CONFIRMED:-0} != 1 ]] \
  && command -v systemctl >/dev/null 2>&1 \
  && systemctl --user is-active --quiet wisp-server.service; then
  echo "stop wisp-server.service before restoring" >&2
  exit 1
fi
if [[ ${WISP_RESTORE_OFFLINE_CONFIRMED:-0} != 1 ]] \
  && pgrep -u "$(id -u)" -x wisp-server >/dev/null 2>&1; then
  echo "stop wisp-server before restoring" >&2
  exit 1
fi

data_root=${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}/wisp/server
database=${WISP_DATABASE_URL:-sqlite://$data_root/wisp.sqlite3}
database=${database#sqlite://}
mkdir -p "$(dirname -- "$database")"
previous="${database}.pre-restore-$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -f "$database" ]]; then
  mv -- "$database" "$previous"
  echo "Previous database preserved at: $previous"
fi
rm -f -- "${database}-wal" "${database}-shm"
install -m 0600 "$source_backup" "$database"

if [[ $(sqlite3 "$database" 'PRAGMA integrity_check;') != "ok" ]]; then
  echo "restored database failed integrity_check" >&2
  exit 1
fi
echo "Restored Wisp database from: $source_backup"
