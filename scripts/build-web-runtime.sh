#!/usr/bin/env bash
# Build a private Quickshell with the two WebEngine startup fixes. The system
# Quickshell remains untouched; only Wisp launches this runtime.
set -euo pipefail
[[ ${WISP_SKIP_WEB_RUNTIME:-0} != 1 ]] || exit 0
for command in cmake ninja c++ pkg-config curl tar sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Inline videos need the Wisp web runtime: install cmake, ninja, a C++ compiler, CLI11, wayland-protocols and Qt 6.8+ WebEngine, then run wisp-web-runtime." >&2
    exit 0
  fi
done
if ! pkg-config --atleast-version=6.8 Qt6WebEngineQuick || ! pkg-config --exists wayland-protocols CLI11; then
  echo "Inline videos need Qt 6.8+ WebEngine, CLI11 and wayland-protocols; install them and run wisp-web-runtime." >&2
  exit 0
fi
qt_version=$(pkg-config --modversion Qt6Core)
runtime_version="0.3.1-qt$qt_version-web1"
bin_root=${XDG_BIN_HOME:-$HOME/.local/bin}
state_root=${XDG_STATE_HOME:-$HOME/.local/state}/wisp
cache_root=${XDG_CACHE_HOME:-$HOME/.cache}/wisp/web-runtime/$runtime_version
mkdir -p "$bin_root" "$state_root" "$cache_root"
if [[ -x "$bin_root/wisp-quickshell" && -f "$state_root/web-runtime-version" \
    && $(cat "$state_root/web-runtime-version") == "$runtime_version" ]]; then exit 0; fi
if [[ ! -x "$cache_root/build/src/quickshell" ]]; then
  archive="$cache_root/source.tar.gz"
  curl --fail --location --silent --show-error \
    https://codeload.github.com/quickshell-mirror/quickshell/tar.gz/1a4716cde794a59928d9d9fc15f2afc7a95de360 --output "$archive"
  expected=51690baee56fe150deaf978c430b804243ad264654965b522c795dabe57cfe5b
  actual=$(sha256sum "$archive" | cut -d' ' -f1)
  [[ "$actual" == "$expected" ]] || { echo "Web runtime source checksum mismatch" >&2; exit 1; }
  mkdir -p "$cache_root/source"
  tar --extract --gzip --file "$archive" --directory "$cache_root/source" --strip-components=1 --no-same-owner
  # Qt WebEngine needs argv[0] and shared GL contexts. Upstream 0.3.1 passes
  # argc=0, which makes Chromium terminate the application during initialization.
  sed -i 's/auto qArgC = 0;/QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);\n\tauto qArgC = 1;/' "$cache_root/source/src/launch/launch.cpp"
  cmake -S "$cache_root/source" -B "$cache_root/build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DDISTRIBUTOR=Wisp -DCRASH_HANDLER=OFF \
    -DUSE_JEMALLOC=OFF -DSERVICE_PIPEWIRE=OFF -DSERVICE_PAM=OFF \
    -DSERVICE_POLKIT=OFF -DBLUETOOTH=OFF -DNETWORK=OFF -DSCREENCOPY=OFF
  cmake --build "$cache_root/build" --parallel "${WISP_BUILD_JOBS:-4}"
fi
if [[ -f "$bin_root/wisp-quickshell" ]]; then
  backup=$(mktemp -d "$state_root/web-runtime-backup.XXXXXX")
  cp -a "$bin_root/wisp-quickshell" "$backup/"
fi
candidate=$(mktemp "$bin_root/.wisp-quickshell.XXXXXX")
trap 'rm -f -- "$candidate"' EXIT
install -m 0755 "$cache_root/build/src/quickshell" "$candidate"
mv -f -- "$candidate" "$bin_root/wisp-quickshell"
printf '%s\n' "$runtime_version" > "$state_root/web-runtime-version"
echo "Wisp web runtime installed; restart Wisp to enable inline videos."
