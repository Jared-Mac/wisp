# Publishing the Omarchy plugin

The public repository is `Jared-Mac/omarchy-wisp`. Export a committed revision with:

```sh
python3 scripts/export-omarchy-plugin.py /tmp/dev.wisp --ref HEAD
omarchy plugin validate /tmp/dev.wisp
```

The exporter permits only plugin source/assets, the license, and the public files
under `packaging/omarchy-plugin/`. It excludes development history and workflows,
local changes, native binaries and private operational material. Review the final
export before its first publication. The preview is generated from the offline
`BarLayout.qml` fixture; never use live conversations in public screenshots.

CI packages this export as `wisp-omarchy-plugin.tar.gz` with a SHA-256 checksum.
The public repository's hourly/manual workflow checks the checksum and requires
its source commit to match the published client's update metadata before import.
It updates only product files; changes to its own `.github` publisher must be
reviewed and applied in that repository separately. No cross-repository secret
is required. The rolling main channel is a pre-release channel.

Validate with `python3 tests/omarchy-plugin.py`, `bash scripts/test-omarchy-update.sh`,
`bash scripts/test-onboarding.sh`, and `bash scripts/test-bar-layout.sh`.
