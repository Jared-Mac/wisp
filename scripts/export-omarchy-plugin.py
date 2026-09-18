#!/usr/bin/env python3
"""Export a committed, public-only Omarchy plugin without development history."""
import argparse
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[1]
PUBLIC = {
    "README.md", "preview.png", ".github/update.py", ".github/workflows/update.yml",
}
PRIVATE = re.compile(
    rb"/home/jaredm|TLT26-churn|wisp-vps|WISP_DISCORD|android-handoff|development-workflows"
    rb"|-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----"
    rb"|github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}"
)


def git(*args):
    return subprocess.check_output(["git", "-C", str(REPO), *args])


def public_name(name):
    if name in {"quickshell/manifest.json", "quickshell/Panel.qml"}:
        return name.removeprefix("quickshell/")
    if name.startswith("quickshell/app/"):
        path = PurePosixPath(name)
        if any(part.startswith(".") or part in {"native", "__pycache__"} for part in path.parts):
            raise ValueError(f"Unexpected plugin source path: {name}")
        if path.suffix not in {".qml", ".js", ".json", ".svg", ".png", ".wav"} and name != "quickshell/app/assets/PRESENCE-ICONS-LICENSE.txt":
            raise ValueError(f"Unexpected plugin source type: {name}")
        return name.removeprefix("quickshell/")
    if name == "LICENSE":
        return name
    prefix = "packaging/omarchy-plugin/"
    if name.startswith(prefix) and name[len(prefix):] in PUBLIC:
        return name[len(prefix):]
    return None


def export(ref, destination):
    commit = git("rev-parse", "--verify", ref + "^{commit}").decode().strip()
    destination = Path(destination).resolve()
    if destination.exists():
        raise ValueError("Export destination must not exist")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".wisp-plugin-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "plugin"
        stage.mkdir()
        for entry in git("ls-tree", "-rz", commit).split(b"\0"):
            if not entry:
                continue
            metadata, raw_name = entry.split(b"\t", 1)
            mode, kind, oid = metadata.split()
            name = raw_name.decode()
            target = public_name(name)
            if target is None:
                continue
            if mode not in {b"100644", b"100755"} or kind != b"blob":
                raise ValueError(f"Only regular files may be exported: {name}")
            data = git("cat-file", "blob", oid.decode())
            if PRIVATE.search(data):
                raise ValueError(f"Private material detected in {name}")
            output = stage / target
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
            output.chmod(0o755 if mode == b"100755" else 0o644)
        for required in ["manifest.json", "Panel.qml", "app/WispBridge.qml", "README.md", "LICENSE", ".github/update.py", ".github/workflows/update.yml"]:
            if not (stage / required).is_file():
                raise ValueError(f"Missing public export file: {required}")
        manifest = json.loads((stage / "manifest.json").read_text())
        manifest["version"] = manifest["version"].split("+")[0] + "+" + commit[:12]
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        (stage / "release.json").write_text(json.dumps({
            "format": 1, "source_repository": "Jared-Mac/wisp", "commit": commit,
            "version": manifest["version"], "client_release": "main",
        }, indent=2) + "\n")
        stage.rename(destination)
    return commit


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--ref", default="HEAD")
    args = parser.parse_args()
    print("Exported plugin from " + export(args.ref, args.destination))
