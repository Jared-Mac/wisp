#!/usr/bin/env python3
"""Import only a plugin archive paired with the published Wisp client release."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
from urllib.error import HTTPError
from urllib.request import Request, urlopen

BASE = "https://github.com/Jared-Mac/wisp/releases/download/main/"
ARCHIVE = "wisp-omarchy-plugin.tar.gz"
ROOT = Path(__file__).resolve().parents[1]


def download(name, limit=32*1024*1024):
    with urlopen(Request(BASE+name, headers={"User-Agent": "wisp-omarchy-plugin"}), timeout=60) as response:
        data = response.read(limit+1)
    if len(data) > limit:
        raise ValueError("Release asset exceeds size limit")
    return data


def unpack(data, directory):
    count = 0
    total = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != "dev.wisp":
                raise ValueError("Unsafe archive path")
            relative = PurePosixPath(*path.parts[1:])
            if member.isdir():
                continue
            if not member.isfile() or relative == PurePosixPath("."):
                raise ValueError("Only regular plugin files are accepted")
            if relative.parts[0] not in {"app", ".github"} and str(relative) not in {"Panel.qml", "manifest.json", "release.json", "LICENSE", "README.md", "preview.png"}:
                raise ValueError("Unexpected plugin file")
            if ".git" in relative.parts or "__pycache__" in relative.parts:
                raise ValueError("Unexpected private directory")
            count += 1
            total += member.size
            if count > 1000 or total > 64*1024*1024:
                raise ValueError("Expanded plugin exceeds size limit")
            target = directory.joinpath(*relative.parts)
            if target.exists():
                raise ValueError("Duplicate archive entry")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.extractfile(member).read())
            target.chmod(0o644)
    for required in ["manifest.json", "release.json", "Panel.qml", "app/WispBridge.qml", "README.md", "LICENSE"]:
        if not (directory/required).is_file():
            raise ValueError("Incomplete plugin release")
    if json.loads((directory/"manifest.json").read_text()).get("id") != "dev.wisp":
        raise ValueError("Wrong plugin identity")


def main():
    try:
        client = json.loads(download("wisp-main-linux-x86_64.update.json", 1024*1024))
        expected = download(ARCHIVE+".sha256", 4096).decode().split()[0]
        data = download(ARCHIVE)
    except HTTPError as error:
        if error.code == 404:
            print("Waiting for the first Wisp release with a plugin archive.")
            return
        raise
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError("Plugin checksum mismatch; retry after release upload finishes")
    with tempfile.TemporaryDirectory() as temporary:
        stage = Path(temporary)
        unpack(data, stage)
        release = json.loads((stage/"release.json").read_text())
        if release.get("commit") != client.get("commit") or not release.get("commit"):
            print("Client and plugin release are still being published; leaving current plugin intact.")
            return
        # Source-release exports cannot rewrite this publisher or its permissions.
        # Changes to .github are reviewed and applied separately by the maintainer.
        files = {str(p.relative_to(stage)):p for p in stage.rglob("*") if p.is_file() and ".github" not in p.relative_to(stage).parts}
        tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
        for name in tracked:
            if name and not name.startswith(".github/") and name not in files:
                (ROOT/name).unlink()
        for name, source in files.items():
            destination = ROOT/name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        print("Imported Wisp plugin " + release["version"])


if __name__ == "__main__":
    main()
