#!/usr/bin/env python3
"""Per-user Wisp release checks and coordinated client installation. No sudo."""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
import uuid

DEFAULTS = {"automatic": True, "check_on_launch": True, "background_checks": True, "interval_minutes": 5}
BUSY = {"downloading", "preparing", "installing"}
MAX_ARCHIVE = 512 * 1024 * 1024


def roots():
    user = Path.home()
    return (Path(os.environ.get("XDG_CONFIG_HOME", user / ".config")) / "wisp",
            Path(os.environ.get("XDG_STATE_HOME", user / ".local/state")) / "wisp",
            Path(os.environ.get("XDG_BIN_HOME", user / ".local/bin")),
            Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")) / "wisp/updates")


def read(path, fallback=None):
    try:
        value = json.loads(path.read_text())
        return value if isinstance(value, dict) else (fallback or {})
    except (OSError, ValueError):
        return {} if fallback is None else fallback


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd, temporary = tempfile.mkstemp(prefix=".update-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as target:
            json.dump(value, target)
            target.write("\n")
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


@contextlib.contextmanager
def lock(name):
    runtime = roots()[3]
    runtime.mkdir(parents=True, exist_ok=True, mode=0o700)
    with (runtime / name).open("a") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield


def process_start(pid):
    try:
        return Path(f"/proc/{int(pid)}/stat").read_text().rsplit(")", 1)[1].split()[19]
    except (OSError, ValueError, IndexError):
        return ""


def preferences():
    saved = read(roots()[0] / "updates.json")
    result = DEFAULTS.copy()
    for key, default in DEFAULTS.items():
        value = saved.get(key)
        if type(value) is type(default) and (key != "interval_minutes" or 1 <= value <= 1440):
            result[key] = value
    return result


def state(**changes):
    path = roots()[1] / "update-state.json"
    result = read(path, {"phase": "idle", "available": False})
    if changes:
        result.update(changes)
        write(path, result)
    return result


def status():
    result = state()
    if result.get("phase") in BUSY and result.get("worker_pid"):
        if process_start(result["worker_pid"]) != result.get("worker_start"):
            result = state(phase="error", error="The update stopped unexpectedly. Try again.", request="")
    return dict(result, preferences=preferences(), installed=read(roots()[0] / "installed-release.json"))


def repository():
    config = roots()[0] / "update-repository"
    value = os.environ.get("WISP_UPDATE_REPOSITORY") or (config.read_text().strip() if config.exists() else "Jared-Mac/wisp")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value):
        raise ValueError("Invalid update repository")
    return value


def release_base():
    return os.environ.get("WISP_UPDATE_BASE_URL", f"https://github.com/{repository()}/releases/download/main")


def fetch(url, limit, destination=None):
    request = urllib.request.Request(url, headers={"User-Agent": "Wisp-Updater/1", "Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=15) as response:
        length = 0
        parts = []
        while chunk := response.read(256 * 1024):
            length += len(chunk)
            if length > limit:
                raise ValueError("Update download exceeds its declared size")
            if destination:
                destination.write(chunk)
            else:
                parts.append(chunk)
        return length if destination else b"".join(parts)


def validate_manifest(value):
    if not isinstance(value, dict):
        raise ValueError("Invalid update manifest")
    if value.get("format") != 1 or value.get("platform") != "linux-x86_64":
        raise ValueError("Unsupported update package")
    if not re.fullmatch(r"[a-f0-9]{40}", str(value.get("commit", ""))):
        raise ValueError("Invalid release version")
    if not re.fullmatch(r"[a-f0-9]{64}", str(value.get("sha256", ""))):
        raise ValueError("Invalid release checksum")
    if value.get("archive") != "wisp-main-linux-x86_64.tar.gz":
        raise ValueError("Unexpected update archive")
    if not isinstance(value.get("size"), int) or not 0 < value["size"] <= MAX_ARCHIVE:
        raise ValueError("Invalid update size")
    if not isinstance(value.get("commit_time"), int) or value["commit_time"] <= 0:
        raise ValueError("Invalid release timestamp")
    return value


def is_newer(release, installed):
    if release["commit"] == installed.get("commit"):
        return False
    # Local source builds may be ahead of the published rolling release.
    timestamp = installed.get("commit_time", 0)
    return release["commit_time"] > (timestamp if type(timestamp) is int else 0)


def check(force=False):
    with lock("check.lock"), lock("install.lock"):
        previous = status()
        if previous.get("phase") in BUSY:
            return status()
        interval = preferences()["interval_minutes"] * 60
        if not force and time.time() - previous.get("last_checked", 0) < interval:
            return status()
        if platform.system() != "Linux" or platform.machine() != "x86_64":
            return state(phase="error", error="No update package is available for this platform.")
        try:
            manifest = validate_manifest(json.loads(fetch(release_base() + "/wisp-main-linux-x86_64.update.json", 65536)))
            available = is_newer(manifest, read(roots()[0] / "installed-release.json"))
            return state(phase="available" if available else "idle", available=available,
                         release=manifest, last_checked=time.time(), error="", request="")
        except (OSError, ValueError, urllib.error.URLError):
            return state(phase="error", last_checked=time.time(), error="Couldn't check for updates. Try again later.")


def run(args, *, timeout=30, check=True, environment=None):
    result = subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=timeout, env=environment)
    if check and result.returncode:
        raise RuntimeError("An update step failed. Your previous installation is backed up.")
    return result


def digest(path):
    with path.open("rb") as data:
        return hashlib.file_digest(data, "sha256").hexdigest()


def clients():
    result = []
    for path in (roots()[3] / "clients").glob("*.json"):
        client = read(path)
        if client.get("start") and process_start(client.get("pid", 0)) == client["start"]:
            result.append(client)
        else:
            path.unlink(missing_ok=True)
    return result


def idle_snapshot(snapshot):
    if not isinstance(snapshot, dict) or not isinstance(snapshot.get("self"), dict):
        return False
    me = snapshot.get("self", {})
    media = me.get("media", {})
    return not (me.get("hangout_id") or me.get("sharing") or media.get("livekit_connected") or media.get("screen_share", {}).get("active")
                or media.get("camera", {}).get("active"))


def snapshot():
    return json.loads(run([roots()[2] / "wispctl", "status"]).stdout)


def safe_clients(automatic, request=None):
    current = clients()
    # The UI must participate so its drafts and pending operations are protected.
    return bool(current) and all(c.get("safe") and (not automatic or not c.get("foreground"))
                                 and (request is None or c.get("prepared") == request) for c in current)


def extract(archive, destination):
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        if len(members) > 20000 or sum(m.size for m in members) > 2 * 1024**3:
            raise ValueError("Update archive is too large")
        for member in members:
            parts = Path(member.name).parts
            if not parts or parts[0] != "wisp-main-linux-x86_64" or ".." in parts or Path(member.name).is_absolute():
                raise ValueError("Invalid update archive path")
            if not (member.isfile() or member.isdir()):
                raise ValueError("Links and special files are not allowed in update packages")
            target = destination / member.name
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as data, target.open("wb") as out:
                    shutil.copyfileobj(data, out)
                target.chmod(0o755 if member.mode & 0o111 else 0o644)
    return destination / "wisp-main-linux-x86_64"


def backup_installation():
    config, saved, bins, _ = roots()
    (saved / "backups").mkdir(parents=True, exist_ok=True)
    backup = Path(tempfile.mkdtemp(prefix="client-update-", dir=saved / "backups"))
    paths = [(bins, "bin"), (config, "account"), (config.parent / "quickshell/wisp", "ui"),
             (config.parent / "quickshell/wisp-onboarding", "onboarding"),
             (config.parent / "omarchy/plugins/dev.wisp", "omarchy"),
             (config.parent / "systemd/user/wisp.service", "wisp.service")]
    for source, name in paths:
        if not source.exists():
            continue
        if name == "bin":
            (backup / name).mkdir()
            for binary in bins.glob("wisp*"):
                if binary.is_file():
                    shutil.copy2(binary, backup / name / binary.name)
        elif source.is_dir():
            shutil.copytree(source, backup / name, symlinks=True)
        else:
            shutil.copy2(source, backup / name)
    return backup


def restore_installation(backup):
    config, _, bins, _ = roots()
    for binary in (backup / "bin").iterdir():
        temporary = bins / ("." + binary.name + ".rollback")
        shutil.copy2(binary, temporary)
        temporary.replace(bins / binary.name)
    for name, target in [("ui", config.parent / "quickshell/wisp"), ("onboarding", config.parent / "quickshell/wisp-onboarding"),
                         ("omarchy", config.parent / "omarchy/plugins/dev.wisp")]:
        if (backup / name).is_dir():
            if target.exists():
                shutil.rmtree(target)
            shutil.copytree(backup / name, target, symlinks=True)
    if (backup / "wisp.service").exists():
        shutil.copy2(backup / "wisp.service", config.parent / "systemd/user/wisp.service")
    marker = backup / "account/installed-release.json"
    if marker.exists():
        shutil.copy2(marker, config / "installed-release.json")
    else:
        (config / "installed-release.json").unlink(missing_ok=True)
    run(["systemctl", "--user", "daemon-reload"])


def worker(automatic):
    with lock("install.lock"):
        config, saved, bins, _ = roots()
        release = validate_manifest(state().get("release", {}))
        if not is_newer(release, read(config / "installed-release.json")):
            return state(phase="idle", available=False)
        if automatic and (not preferences()["automatic"] or state().get("attempted_commit") == release["commit"]):
            return state(phase="available")
        if not safe_clients(automatic) or not idle_snapshot(snapshot()):
            return state(phase="available", waiting="Waiting until Wisp is idle")
        state(phase="downloading", worker_pid=os.getpid(), worker_start=process_start(os.getpid()),
              attempted_commit=release["commit"] if automatic else "", error="", waiting="")
        backup = None
        stopped = False
        try:
            saved.mkdir(parents=True, exist_ok=True)
            (saved / "backups").mkdir(exist_ok=True)
            with tempfile.TemporaryDirectory(prefix="client-download-", dir=saved) as folder:
                folder = Path(folder)
                archive = folder / release["archive"]
                with archive.open("wb") as output:
                    size = fetch(release_base() + "/" + release["archive"], release["size"], output)
                if size != release["size"] or digest(archive) != release["sha256"]:
                    raise ValueError("Update checksum mismatch")
                package = extract(archive, folder / "unpacked")
                if read(package / "release.json").get("commit") != release["commit"]:
                    raise ValueError("The release changed during download")
                for name in ["wispd", "wispctl", "wisp-account"]:
                    run([package / "bin" / name, "--help"])
                if automatic and not preferences()["automatic"]:
                    return state(phase="available", attempted_commit="")
                request = str(uuid.uuid4())
                state(phase="preparing", request=request, automatic_request=automatic)
                for _ in range(50):
                    if safe_clients(automatic, request):
                        break
                    time.sleep(.2)
                else:
                    return state(phase="available", request="", attempted_commit="", waiting="Waiting until Wisp is idle")
                before = snapshot()
                if not idle_snapshot(before):
                    return state(phase="available", request="", attempted_commit="", waiting="Waiting until voice is disconnected")
                desktop = run([bins / "wisp-ui", "app", "desktop"], check=False)
                was_visible = read_json_text(desktop.stdout).get("visible", False)
                backup = backup_installation()
                # Backups can take time. Recheck after them while each UI is frozen.
                if not safe_clients(automatic, request) or not idle_snapshot(snapshot()) or automatic and not preferences()["automatic"]:
                    return state(phase="available", request="", attempted_commit="", waiting="Waiting until Wisp is idle")
                state(phase="installing", backup=str(backup))
                run([bins / "wisp-ui", "quit"])
                run(["systemctl", "--user", "stop", "wisp.service"])
                stopped = True
                environment = dict(os.environ, WISP_CLIENT_ONLY="1", WISP_SKIP_WEB_RUNTIME="1")
                run([package / "install.sh"], timeout=180, environment=environment)
                run(["systemctl", "--user", "start", "wisp.service"])
                after = None
                for _ in range(100):
                    try:
                        after = snapshot()
                        if after:
                            break
                    except (RuntimeError, ValueError):
                        pass
                    time.sleep(.2)
                if not after or not idle_snapshot(after):
                    raise RuntimeError("Wisp did not restart safely")
                for key, on, off in [("deafened", "deafen", "undeafen"), ("muted", "mute", "unmute")]:
                    desired = bool(before["self"].get(key))
                    if bool(snapshot()["self"].get(key)) != desired:
                        run([bins / "wispctl", on if desired else off])
                for name in ["wispd", "wispctl", "wisp-account"]:
                    if digest(bins / name) != digest(package / "bin" / name):
                        raise RuntimeError("Installed client verification failed")
                verify_ui(package)
                verify_daemon()
                # Catch delayed recovery as well as an immediate rejoin.
                for _ in range(15):
                    if not idle_snapshot(snapshot()):
                        disconnect_media()
                        raise RuntimeError("Wisp did not remain disconnected")
                    time.sleep(.2)
                run([bins / "wisp-ui", "app", "open" if was_visible or not automatic else "hide"])
                return state(phase="updated", available=False, error="", request="", completed_at=time.time())
        except Exception as error:
            # Record only the exception class: command output can contain account data.
            write(saved / "last-update-error.json", {"time":time.time(), "step":state().get("phase"), "error_type":type(error).__name__})
            restored = not stopped
            try:
                if stopped and backup:
                    disconnect_media()
                    run(["systemctl", "--user", "stop", "wisp.service"], check=False)
                    restore_installation(backup)
                    run(["systemctl", "--user", "start", "wisp.service"])
                    restored = True
                    run([bins / "wisp-ui", "app", "open"], check=False)
            except Exception:
                restored = False
            return state(phase="error", error=("Couldn't install the update. Your previous client was preserved." if restored else
                         "Couldn't restore Wisp automatically. Your previous client and account are backed up."), request="")


def disconnect_media():
    for args in [["leave"], ["stop-share"], ["camera", "off"]]:
        try:
            run([roots()[2] / "wispctl", *args], check=False, timeout=5)
        except (OSError, subprocess.TimeoutExpired):
            pass


def verify_ui(package):
    for relative, installed in [("quickshell/app", "quickshell/wisp"), ("quickshell/onboarding", "quickshell/wisp-onboarding")]:
        source = package / relative
        if not (source / "shell.qml").is_file():
            raise RuntimeError("Missing release UI")
        for path in source.rglob("*"):
            if path.is_file() and digest(path) != digest(roots()[0].parent / installed / path.relative_to(source)):
                raise RuntimeError("Installed UI verification failed")


def verify_daemon():
    expected = roots()[2] / "wispd"
    matching = []
    for entry in Path("/proc").glob("[0-9]*/exe"):
        try:
            target = str(entry.readlink())
            if target.removesuffix(" (deleted)") == str(expected):
                matching.append(digest(entry) == digest(expected))
        except (OSError, PermissionError):
            pass
    if not matching or not all(matching):
        raise RuntimeError("Running client verification failed")


def read_json_text(value):
    try:
        return json.loads(value)
    except ValueError:
        return {}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["status", "check", "set", "register", "start", "worker"])
    parser.add_argument("values", nargs="*")
    args = parser.parse_args()
    if args.action == "status":
        result = status()
    elif args.action == "check":
        result = check("force" in args.values)
    elif args.action == "set":
        key, value = args.values
        if key not in DEFAULTS:
            raise ValueError("Unknown update setting")
        if key == "interval_minutes":
            value = int(value)
            if not 1 <= value <= 1440:
                raise ValueError("Choose an interval from 1 to 1440 minutes")
        else:
            if value not in ["true", "false"]:
                raise ValueError("Invalid update setting")
            value = value == "true"
        prefs = preferences()
        prefs[key] = value
        write(roots()[0] / "updates.json", prefs)
        result = status()
    elif args.action == "register":
        pid = int(args.values[0])
        client = args.values[1]
        if not re.fullmatch(r"[a-zA-Z0-9_-]+", client) or not process_start(pid):
            raise ValueError("Invalid UI registration")
        directory = roots()[3] / "clients"
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        result = dict(status(), lease_path=str(directory / f"{pid}-{client}.json"), process_start=process_start(pid))
    elif args.action == "start":
        automatic = "automatic" in args.values
        if state().get("phase") in BUSY:
            result = status()
        else:
            run(["systemd-run", "--user", "--collect", "--unit=wisp-client-update", "--property=Type=exec",
                 sys.executable, Path(__file__).resolve(), "worker", "automatic" if automatic else "manual"])
            result = status()
    else:
        result = worker("automatic" in args.values)
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except BlockingIOError:
        print(json.dumps(status()))
    except Exception:
        print(json.dumps({"phase": "error", "error": "The update helper is unavailable. Try again later.", "preferences": preferences()}))
        sys.exit(1)
