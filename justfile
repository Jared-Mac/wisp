set dotenv-load := true

export WISP_SERVER_URL := env_var_or_default("WISP_SERVER_URL", "http://127.0.0.1:8787")

default:
    @just --list

bootstrap:
    ./scripts/bootstrap.sh

livekit-install:
    ./scripts/livekit-install.sh

dev:
    ./scripts/dev.sh

dev-tailscale:
    ./scripts/dev-tailscale.sh

tailscale-info:
    ./scripts/tailscale-info.sh

dev-server:
    cargo run -p wisp-server

dev-daemon profile="Jared":
    cargo run -p wispd -- --profile {{profile}}

sim profile *args:
    cargo run -p wisp-sim -- --profile {{profile}} {{args}}

friend host profile:
    ./scripts/friend-tailscale.sh "{{host}}" "{{profile}}"

friend-bootstrap:
    ./scripts/friend-bootstrap-cachyos.sh

plugin-sync:
    ./scripts/plugin-sync.sh

plugin-watch:
    ./scripts/plugin-watch.sh

app-sync:
    ./scripts/app-sync.sh

cli-install:
    cargo install --path apps/wispctl --locked --root "${HOME}/.local"

app:
    WISP_QUICKSHELL_PATH="{{justfile_directory()}}/quickshell/app" ./scripts/wisp-ui.sh open

app-toggle:
    WISP_QUICKSHELL_PATH="{{justfile_directory()}}/quickshell/app" ./scripts/wisp-ui.sh toggle

test:
    cargo test --workspace

test-integration:
    ./scripts/test-integration.sh

test-media:
    ./scripts/test-media.sh

test-reliability:
    ./scripts/test-voice-reliability.sh

test-knock:
    ./scripts/test-knock.sh

test-ui:
    ./scripts/test-ui.sh

fmt:
    cargo fmt --all

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    just scan-secrets

scan-secrets:
    ./scripts/scan-secrets.sh
