#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cmake -S "$repo_dir/native/video" -B "$repo_dir/target/video-ui" -DCMAKE_BUILD_TYPE=Release
cmake --build "$repo_dir/target/video-ui" --parallel 2
