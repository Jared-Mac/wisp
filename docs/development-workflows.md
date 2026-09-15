# Private development workflows and public source release

These workflows support the current private development process. They are not
Wisp features and must not become requirements for running a public source build.

- [Android change queue](android-handoff.md): collect desktop/server changes for
  a later, explicitly requested Android batch. Update the queue with relevant
  commits; do not send routine task updates or wake the Android task to process it.
- [Discord patch notes](discord-patch-notes.md): the current main-push announcement
  workflow remains enabled during private development. It uses repository secrets;
  no webhook belongs in source. Announce completed releases, never queued Android work.
- [Owner-managed deployment](automatic-server-deployment.md): continue the current
  release checks during private development. Keep host configuration and credentials
  outside source.

## Before publishing the actual source release

Do this as an explicit release-preparation change, not during ordinary feature work:

1. Remove `docs/android-handoff.md`, private release-tracking state, and internal
   task/coordination instructions from the public export, including their links.
2. Remove the private Discord workflow, posting script and its workflow-specific
   tests/documentation; retain useful user-facing release notes if wanted.
3. Remove or replace owner-specific deployment workflows, private task/repository
   references and operational instructions with general self-hosting documentation.
4. Review `AGENTS.md`, `README.md`, CI, packaging and docs for dangling references.
   Ensure public builds/tests do not require private secrets, services or tasks.
5. Preserve product features and their documentation: client updates, server
   moderation, Android compatibility, and general build/self-hosting support.
6. Scan the final export for credentials, private identifiers and internal artifacts,
   and build/test that export before publication.

Do not remove these active development workflows prematurely, delete useful Git
history, or change production release automation as a side effect of this checklist.
