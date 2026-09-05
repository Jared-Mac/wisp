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

dev-server:
    cargo run -p wisp-server

dev-daemon profile="Owner":
    cargo run -p wispd -- --profile {{profile}}

sim profile *args:
    cargo run -p wisp-sim -- --profile {{profile}} {{args}}

client:
    ./scripts/wisp-client.sh

host-services:
    ./scripts/install-host-services.sh

plugin-sync:
    ./scripts/plugin-sync.sh

plugin-watch:
    ./scripts/plugin-watch.sh

app-sync:
    ./scripts/app-sync.sh

cli-install:
    cargo install --path apps/wispctl --locked --root "${HOME}/.local"

app:
    ./scripts/app-sync.sh
    wisp-ui app open

app-toggle:
    ./scripts/app-sync.sh
    wisp-ui app toggle

panel:
    ./scripts/app-sync.sh
    wisp-ui panel open

panel-toggle:
    ./scripts/app-sync.sh
    wisp-ui panel toggle

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

test-logo:
    ./scripts/test-logo.sh

test-private-alpha:
    ./scripts/test-private-alpha.sh

fmt:
    cargo fmt --all

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    just scan-secrets

scan-secrets:
    ./scripts/scan-secrets.sh
