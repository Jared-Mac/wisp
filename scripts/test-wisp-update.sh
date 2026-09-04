#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)

cleanup() {
  rm -rf -- "$test_dir"
}
trap cleanup EXIT INT TERM

mock_bin="$test_dir/mock-bin"
state_dir="$test_dir/state"
release_dir="$test_dir/releases"
package_name=wisp-test-linux-x86_64
package_dir="$test_dir/package/$package_name"
mkdir -p "$mock_bin" "$state_dir" "$release_dir" "$package_dir"

cat >"$mock_bin/systemctl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == "--user is-active --quiet wisp.service" ]]; then
  exit 0
fi
if [[ "$*" == "--user restart wisp.service" ]]; then
  touch "$WISP_TEST_STATE_DIR/restarted"
  exit 0
fi
exit 1
EOF

cat >"$mock_bin/wispctl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
command=${1:-}
case "$command" in
  status)
    if [[ ! -e "$WISP_TEST_STATE_DIR/installed" ]]; then
      printf '%s\n' '{"self":{"hangout_id":null,"sharing":false,"muted":true,"deafened":false,"media":{"screen_share":{"active":false},"camera":{"active":false}}}}'
      exit 0
    fi
    status_count_file="$WISP_TEST_STATE_DIR/status-count"
    status_count=0
    [[ -e "$status_count_file" ]] && read -r status_count <"$status_count_file"
    ((status_count += 1))
    printf '%s\n' "$status_count" >"$status_count_file"
    if (( status_count <= 2 )); then
      hangout=null
      sharing=false
      camera=false
    else
      [[ -e "$WISP_TEST_STATE_DIR/left" ]] && hangout=null || hangout='"rejoined"'
      [[ -e "$WISP_TEST_STATE_DIR/share-stopped" ]] && sharing=false || sharing=true
      [[ -e "$WISP_TEST_STATE_DIR/camera-stopped" ]] && camera=false || camera=true
    fi
    [[ -e "$WISP_TEST_STATE_DIR/muted" ]] && muted=true || muted=false
    printf '{"self":{"hangout_id":%s,"sharing":%s,"muted":%s,"deafened":false,"media":{"screen_share":{"active":%s},"camera":{"active":%s}}}}\n' \
      "$hangout" "$sharing" "$muted" "$sharing" "$camera"
    ;;
  leave) touch "$WISP_TEST_STATE_DIR/left" ;;
  stop-share) touch "$WISP_TEST_STATE_DIR/share-stopped" ;;
  camera)
    [[ "${2:-}" == off ]]
    touch "$WISP_TEST_STATE_DIR/camera-stopped"
    ;;
  mute) touch "$WISP_TEST_STATE_DIR/muted" ;;
  unmute) rm -f -- "$WISP_TEST_STATE_DIR/muted" ;;
  deafen) touch "$WISP_TEST_STATE_DIR/deafened" "$WISP_TEST_STATE_DIR/muted" ;;
  undeafen) rm -f -- "$WISP_TEST_STATE_DIR/deafened" ;;
  *) exit 2 ;;
esac
EOF

cat >"$package_dir/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
touch "$WISP_TEST_STATE_DIR/installed"
EOF

chmod 0755 "$mock_bin/systemctl" "$mock_bin/wispctl" "$package_dir/install.sh"
archive="$release_dir/$package_name.tar.gz"
tar -czf "$archive" -C "$test_dir/package" "$package_name"
(
  cd "$release_dir"
  sha256sum "$(basename -- "$archive")" >"$(basename -- "$archive").sha256"
)

PATH="$mock_bin:$PATH" \
XDG_BIN_HOME="$mock_bin" \
WISP_TEST_STATE_DIR="$state_dir" \
WISP_UPDATE_TAG=test \
WISP_UPDATE_BASE_URL="file://$release_dir" \
  bash "$repo_dir/scripts/wisp-update.sh" >/dev/null

for marker in installed restarted left share-stopped camera-stopped muted; do
  [[ -e "$state_dir/$marker" ]] || {
    echo "updater test did not create expected marker: $marker" >&2
    exit 1
  }
done
[[ ! -e "$state_dir/deafened" ]]

echo "Wisp updater test passed"
