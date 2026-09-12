# Desktop native account backup integration

Desktop/server implementation is complete. Release activation and Android
publication are coordinated so the backend is ready before Android distribution.

## Implemented and checked

- Portable atomic local identity/trust storage; unchanged trust does not rewrite.
- Native secure signup/sign-in, original-password migration, password changes,
  exact-operation reauthentication, trusted-device rewrap, native email reset.
- Fixed-target signed history catch-up, detached typed trust merges, bounded sync.
  Unchanged background checks read only small status metadata, not the payload.
- Durable interrupted-operation journals and termination of abandoned credentials.
  Classic migration recovery stages replacements and retains prepared credentials.
- Credential installation waits for authenticated restore/locked-state validation.
  Config retries preserve other services, selection and real previous-file backups.
- Default secure CLI/onboarding, explicitly guarded classic login, native reset
  entry through a token-free deep link, and grouped backup/recovery settings.
- Background sync defaults on with a 60-second interval and a manual Sync action.
  Separate operation leases keep ordinary encrypted chat trust updates available.
- Launcher reconciles confirmed credential installations before reading config;
  native helpers and the daemon never install an incomplete recovery credential.
- Scoped media keys join the portable bundle without rewriting unchanged data.
  Locked secure accounts cannot enroll replacement identities or publish media.
- Desktop synthetic HTTP server integration covers remote same-key restoration,
  two-device trust merge, lost signup/password/reset responses, concurrent local
  trust during migration, repeated revoked sign-ins and classic recovery devices,
  trusted-device repair after reset, and interrupted installation.

## Remaining release gates

- Final release build and coordinated cross-platform packaging. No real-account
  migration or private password entry through tools.
- Completed verification includes strict workspace Clippy, 103 server tests,
  98 desktop tests, native CLI remote restore
  including media keys, invitation/join integration, onboarding, backup settings
  in four themes, and the existing dismissible-error/recovery-email UI tests.
- Finish coordinated desktop/Android versioning, client installation, main release,
  owner-server migration/health verification and Discord release-note delivery.
