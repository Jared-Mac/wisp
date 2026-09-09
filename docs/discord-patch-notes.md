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
