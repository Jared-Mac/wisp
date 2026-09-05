# Automatic main server releases

Pushing `main` runs CI, including its tests and release build. The production job
downloads that run's artifact, verifies its checksum, and submits only the server
binary to the configured owner-managed VPS. The GitHub repository secret
`WISP_PRODUCTION_DEPLOY` contains the deployment destination, a dedicated SSH key,
and pinned SSH host keys. Never put that secret or private keys in the repository.

The VPS account `wisp-deploy` accepts only `stage COMMIT SHA256 SEQUENCE` and
`status` through a forced SSH command. It has no interactive SSH shell or
forwarding, and its only sudo command is the root-owned deployment helper. CI
cannot replace that helper or use the SSH endpoint for file reads or arbitrary
root commands. Uploaded server code runs as the existing `wisp` service user and
receives its service environment, so repository write access is deployment
authority.

A root-owned timer checks the queue every 30 seconds. An identical installed
binary is verified without a restart. For a changed server build, the helper:

1. Checks both Wisp membership and LiveKit occupancy; active voice defers the release.
2. Runs the candidate's migrations against a separate database copy as `wisp`.
3. Checks occupancy again, stops only `wisp-server`, and backs up its database,
   executable, and environment under `/var/backups/wisp/github-*`.
4. Installs and starts the candidate, then verifies schema/checksums, database
   integrity, foreign keys, running executable, service state, and public health.

GitHub waits briefly for completion. If people remain in voice, its log reports
that the release is queued; the VPS continues trying after the job ends. This
does not join anyone to voice or restart LiveKit/Caddy. Newer release sequences
supersede queued older releases; stale jobs cannot overwrite a newer release.

Inspect `/var/lib/wisp-deploy/status.json`, `systemctl status wisp-deploy.timer`,
and `journalctl -u wisp-deploy.service` on the VPS. A `deployed` state records the
commit, executable checksum, schema and backup path. A `failed` state needs
attention; rerun the GitHub workflow after fixing the cause. A rerun has a higher
attempt sequence. Failures retain backups. The helper never restores a database
automatically, because it could contain new messages. It only restores the old
executable when the database migration state is unchanged.
Migration probe logs remain private at `/var/lib/wisp-deploy/last-probe.log`.

The root-owned configuration `/etc/wisp/deploy.json` names the installed binary,
database, environment file, backup directory, internal LiveKit API, and local and
public health URLs. Deployment code lives in `infra/deploy/`; changes to these
privileged helpers require installation by the VPS owner or a sudo administrator.
The pipeline intentionally cannot update its own privileged access controls.
