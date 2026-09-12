# Discord patch notes

Every push or merge to `main` runs the Discord patch-notes workflow. Its repository
secret is `WISP_DISCORD_PATCH_NOTES`; keep the webhook URL out of tracked files,
logs and release notes. The workflow never runs on pull requests or tags.

Add a new, succinct Markdown file under `docs/patch-notes/` for each release or
merge. Describe changes for users and omit personal names, account details and
server addresses. Newly added notes in the pushed commit range become one Discord
message. When there is no notes file, commit subjects are used as a fallback.

Messages explicitly announce a merge, not a successful deployment. GitHub CI and
the production deployment service report release status separately. Mentions are
disabled. Discord confirms delivery before the workflow succeeds; only explicit
rate-limit responses are retried. Check the channel before manually rerunning a
failed job, since a lost response can leave delivery uncertain.

## Coordinating Android notes

The Android client lives in the private `TLT26-churn/wisp-android` repository.
During the normal push, pull, and server-update workflow, inspect its published
commits/releases since the last covered revision and ask its task for the tested
release summary when needed. Add relevant user-facing Android changes to the
same new `docs/patch-notes/*.md` entry, with a short Android label so readers can
tell which client changed. Distinguish a test APK from a generally available
release. Do not announce uncommitted or unfinished work as released, and do not
publish private repository contents, credentials, host details, or invite codes.

`docs/release-tracking/android.json` records the repository, last confirmed
Discord-covered Android commit, and any pending note/commit. Before pushing notes,
set pending fields to the exact Android commit and note path. After the existing
Discord workflow reports successful delivery, promote that pending commit to
`last_announced_commit` and clear pending fields. A pending entry with successful
workflow delivery already counts as covered: verify the workflow before drafting
another announcement, so retries or an interrupted task do not repeat notes.
The tracking file stays outside the patch-notes directory and is not itself posted.

Pulling a release whose notes were already delivered does not send them again.
For a server update without a main push, prepare the combined note for the next
normal authorized main push; the existing repository-secret workflow delivers it.
No cross-repository token or additional Discord webhook is placed in source.
The initial Android repository has no published source commit yet, so the initial
tracking revision is null and no Android release is claimed.
