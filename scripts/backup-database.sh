#!/usr/bin/env bash
set -euo pipefail

command -v sqlite3 >/dev/null 2>&1 || {
  echo "sqlite3 is required for Wisp backups" >&2
  exit 1
}

data_root=${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}/wisp/server
database=${WISP_DATABASE_URL:-sqlite://$data_root/wisp.sqlite3}
database=${database#sqlite://}
backup_dir="$data_root/backups"
mkdir -p "$backup_dir"
chmod 0700 "$backup_dir"
output=${1:-$backup_dir/wisp-$(date -u +%Y%m%dT%H%M%SZ).sqlite3}

if [[ ! -f "$database" ]]; then
  echo "Wisp database does not exist: $database" >&2
  exit 1
fi
if [[ "$output" == *"'"* || "$output" == *$'\n'* ]]; then
  echo "backup path contains unsupported characters" >&2
  exit 2
fi

temporary="${output}.partial"
trap 'rm -f -- "$temporary"' EXIT
sqlite3 "$database" ".backup '$temporary'"
if [[ $(sqlite3 "$temporary" 'PRAGMA integrity_check;') != "ok" ]]; then
  echo "backup integrity check failed" >&2
  exit 1
fi
chmod 0600 "$temporary"
mv -f -- "$temporary" "$output"
trap - EXIT

# Keep the most recent fourteen automatic backups. Explicit output paths are
# never included in this cleanup.
if [[ "$output" == "$backup_dir"/* ]]; then
  mapfile -t expired < <(find "$backup_dir" -maxdepth 1 -type f -name 'wisp-*.sqlite3' -printf '%T@ %p\n' \
    | sort -nr | tail -n +15 | cut -d' ' -f2-)
  ((${#expired[@]} == 0)) || rm -f -- "${expired[@]}"
fi

echo "Created verified Wisp backup: $output"
