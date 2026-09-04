#!/usr/bin/env bash
set -euo pipefail

repository=${WISP_UPDATE_REPOSITORY:-Jared-Mac/wisp}
release_tag=${WISP_UPDATE_TAG:-main}
bin_root=${XDG_BIN_HOME:-${HOME:?HOME is required}/.local/bin}
release_base=${WISP_UPDATE_BASE_URL:-https://github.com/$repository/releases/download/$release_tag}

if [[ "$(uname -s)" != Linux ]]; then
  echo "Wisp prebuilt updates currently support Linux x86_64 only." >&2
  exit 1
fi

case "$(uname -m)" in
  x86_64) architecture=x86_64 ;;
  *)
    echo "Wisp does not publish a prebuilt update for $(uname -m) yet." >&2
    exit 1
    ;;
esac

for command in curl sha256sum tar; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "missing required command: $command" >&2
    exit 1
  }
done

service_active=false
was_muted=false
was_deafened=false
if command -v systemctl >/dev/null 2>&1 \
  && systemctl --user is-active --quiet wisp.service; then
  service_active=true
  command -v jq >/dev/null 2>&1 || {
    echo "jq is required to safely update a running Wisp client" >&2
    exit 1
  }
  if ! snapshot=$("$bin_root/wispctl" status 2>/dev/null); then
    echo "Wisp is running but its status could not be read; update after stopping it." >&2
    exit 1
  fi
  if jq -e '
    .self.hangout_id != null
    or .self.sharing == true
    or (.self.media.screen_share.active // false) == true
    or (.self.media.camera.active // false) == true
  ' >/dev/null <<<"$snapshot"; then
    echo "Wisp is in a hangout or publishing video; leave and stop video before updating." >&2
    exit 1
  fi
  was_muted=$(jq -r '.self.muted == true' <<<"$snapshot")
  was_deafened=$(jq -r '.self.deafened == true' <<<"$snapshot")
fi

package_name="wisp-$release_tag-linux-$architecture"
archive="$package_name.tar.gz"
checksum="$archive.sha256"
temporary_dir=$(mktemp -d)

cleanup() {
  rm -rf -- "$temporary_dir"
}
trap cleanup EXIT INT TERM

curl --fail --location --silent --show-error "$release_base/$archive" --output "$temporary_dir/$archive"
curl --fail --location --silent --show-error "$release_base/$checksum" --output "$temporary_dir/$checksum"

read -r expected_hash expected_archive <"$temporary_dir/$checksum"
if [[ "$expected_archive" != "$archive" || ! "$expected_hash" =~ ^[[:xdigit:]]{64}$ ]]; then
  echo "release checksum has an unexpected format" >&2
  exit 1
fi
actual_hash=$(sha256sum "$temporary_dir/$archive" | cut -d' ' -f1)
if [[ "$actual_hash" != "$expected_hash" ]]; then
  echo "release checksum verification failed" >&2
  exit 1
fi

tar --extract --gzip --file "$temporary_dir/$archive" --directory "$temporary_dir" --no-same-owner
release_dir="$temporary_dir/$package_name"
if [[ ! -x "$release_dir/install.sh" ]]; then
  echo "release archive is missing install.sh" >&2
  exit 1
fi

"$release_dir/install.sh"

if [[ "$service_active" == true ]]; then
  systemctl --user restart wisp.service
  for _ in {1..10}; do
    if restarted_snapshot=$("$bin_root/wispctl" status 2>/dev/null); then
      if jq -e '.self.hangout_id != null' >/dev/null <<<"$restarted_snapshot"; then
        "$bin_root/wispctl" leave >/dev/null
      fi
      if jq -e '
        .self.sharing == true or (.self.media.screen_share.active // false) == true
      ' >/dev/null <<<"$restarted_snapshot"; then
        "$bin_root/wispctl" stop-share >/dev/null
      fi
      if jq -e '(.self.media.camera.active // false) == true' >/dev/null <<<"$restarted_snapshot"; then
        "$bin_root/wispctl" camera off >/dev/null
      fi
      if [[ "$was_deafened" == true ]]; then
        "$bin_root/wispctl" deafen >/dev/null
      elif [[ "$was_muted" == true ]]; then
        "$bin_root/wispctl" undeafen >/dev/null
        "$bin_root/wispctl" mute >/dev/null
      else
        "$bin_root/wispctl" undeafen >/dev/null
        "$bin_root/wispctl" unmute >/dev/null
      fi
      final_snapshot=$("$bin_root/wispctl" status)
      if ! jq -e \
        --argjson muted "$was_muted" \
        --argjson deafened "$was_deafened" '
          .self.hangout_id == null
          and .self.sharing == false
          and (.self.media.screen_share.active // false) == false
          and (.self.media.camera.active // false) == false
          and .self.muted == $muted
          and .self.deafened == $deafened
        ' >/dev/null <<<"$final_snapshot"; then
        echo "Wisp restarted, but its safe session state could not be restored." >&2
        exit 1
      fi
      echo "Updated and restarted Wisp."
      exit 0
    fi
    sleep 1
  done
  echo "Installed the update, but Wisp did not become ready after restart." >&2
  exit 1
fi

echo "Updated Wisp. Start it from the application menu or run: wisp"
