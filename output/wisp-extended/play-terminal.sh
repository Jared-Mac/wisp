#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
ttfx --frame-rate 30 --seed 42 --canvas-width 336 --canvas-height 36 --anchor-text nw --ignore-terminal-dimensions colorshift --gradient-stops ef70bd 35dfdf a68bfa --gradient-steps 24 --gradient-frames 2 --cycles 2 --no-travel --skip-final-gradient < wisp.ansi
