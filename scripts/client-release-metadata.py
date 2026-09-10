#!/usr/bin/env python3
"""Record a source build, or finish a bounded release manifest after packaging."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

source, destination = map(Path, sys.argv[1:3])
if (source / "release.json").exists():
    release = json.loads((source / "release.json").read_text())
else:
    def git(*args):
        return subprocess.check_output(["git", "-C", str(source), *args], text=True).strip()
    release = {"format": 1, "commit": git("rev-parse", "HEAD"), "commit_time": int(git("show", "-s", "--format=%ct", "HEAD"))}
if len(sys.argv) > 3:
    archive = Path(sys.argv[3])
    with archive.open("rb") as data:
        digest = hashlib.file_digest(data, "sha256").hexdigest()
    release.update(platform="linux-" + sys.argv[4], archive=archive.name, size=archive.stat().st_size, sha256=digest)
destination.parent.mkdir(parents=True, exist_ok=True)
temporary = destination.with_name(destination.name + ".tmp")
temporary.write_text(json.dumps(release) + "\n")
temporary.replace(destination)
