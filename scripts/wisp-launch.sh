#!/usr/bin/env bash
set -u

bin_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export PATH="$bin_dir:$PATH"

runtime_dir=${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}/wisp
mkdir -p "$runtime_dir"

exec 9>"$runtime_dir/launcher.lock"
if ! flock -n 9; then
  exit 0
fi

printf '%s Starting Wisp from the desktop launcher\n' "$(date --iso-8601=seconds)"

notified=0
while true; do
  "$bin_dir/wisp-friend"
  result=$?
  if [[ $result -eq 0 ]]; then
    exit 0
  fi
  if [[ $result -eq 2 ]]; then
    if command -v notify-send >/dev/null 2>&1; then
      notify-send --app-name=Wisp --icon=dev.wisp \
        "Wisp needs configuration" \
        "Run: wisp-friend-register <host> <Tyler|Jack|Charlie>"
    fi
    exit "$result"
  fi
  if [[ $notified -eq 0 ]] && command -v notify-send >/dev/null 2>&1; then
    notify-send --app-name=Wisp --icon=dev.wisp \
      "Waiting for the Wisp host" \
      "The saved host is offline. Wisp will connect automatically when it returns."
    notified=1
  fi
  sleep 10
done
