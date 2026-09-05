#!/usr/bin/python3
"""Root-owned deployment queue. Only stage/status are exposed through SSH."""
import base64
import contextlib
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path
import pwd
import re
import shlex
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request

STATE = Path("/var/lib/wisp-deploy")
CONFIG = Path("/etc/wisp/deploy.json")
LIMIT = 128 * 1024 * 1024


def atomic_json(path, value):
    temporary = path.with_suffix(".next")
    temporary.write_text(json.dumps(value, sort_keys=True) + "\n")
    temporary.replace(path)


def read_state():
    path = STATE / "status.json"
    return json.loads(path.read_text()) if path.exists() else {"phase": "idle", "sequence": -1}


@contextlib.contextmanager
def locked():
    STATE.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (STATE / "lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def stage(commit, checksum, sequence, source):
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9a-f]{64}", checksum):
        raise ValueError("Invalid release identity")
    if not re.fullmatch(r"[0-9]{1,12}", sequence):
        raise ValueError("Invalid release sequence")
    STATE.mkdir(mode=0o700, parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix="upload-", dir=STATE)
    temporary = Path(temporary)
    try:
        size = 0
        with os.fdopen(fd, "wb") as target:
            while chunk := source.read(1024 * 1024):
                size += len(chunk)
                if size > LIMIT:
                    raise ValueError("Server build exceeds upload limit")
                target.write(chunk)
            target.flush()
            os.fsync(target.fileno())
        with temporary.open("rb") as binary:
            if binary.read(4) != b"\x7fELF" or digest(temporary) != checksum:
                raise ValueError("Server build checksum or format is invalid")
        with locked():
            previous = read_state()
            if int(sequence) < previous["sequence"]:
                return {"phase": "superseded", "commit": commit}
            if int(sequence) == previous["sequence"]:
                if previous.get("commit") != commit or previous.get("sha256") != checksum:
                    raise ValueError("Release sequence was already used")
                return previous
            temporary.chmod(0o700)
            temporary.replace(STATE / "pending-server")
            result = {"phase": "queued", "commit": commit, "sha256": checksum,
                      "sequence": int(sequence), "queued_at": int(time.time())}
            atomic_json(STATE / "status.json", result)
            return result
    finally:
        temporary.unlink(missing_ok=True)


def environment(path):
    values = {"PATH": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"}
    for line in Path(path).read_text().splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        key, value = line.split("=", 1)
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", key):
            raise ValueError("Invalid server environment key")
        parsed = shlex.split(value)
        if len(parsed) > 1:
            raise ValueError("Quote environment values containing spaces")
        values[key] = parsed[0] if parsed else ""
    return values


def database(path):
    return sqlite3.connect("file:" + str(path) + "?mode=ro", uri=True, timeout=15)


def inspect_database(path):
    with database(path) as db:
        if db.execute("PRAGMA integrity_check").fetchone() != ("ok",):
            raise RuntimeError("Database integrity check failed")
        if db.execute("PRAGMA foreign_key_check").fetchone():
            raise RuntimeError("Database foreign-key check failed")
        rows = db.execute("SELECT version, success, hex(checksum) FROM _sqlx_migrations ORDER BY version").fetchall()
        if not rows or any(not row[1] for row in rows):
            raise RuntimeError("Database has unsuccessful migrations")
        version = db.execute("SELECT MAX(version) FROM schema_migrations").fetchone()[0]
        return {"schema": version, "migrations": rows}


def backup_database(source, destination):
    with database(source) as original, sqlite3.connect(destination) as backup:
        original.backup(backup)


def json_request(url, data=None, headers=None):
    request = urllib.request.Request(url, data=data, headers=headers or {})
    with urllib.request.urlopen(request, timeout=8) as response:
        return json.load(response)


def rooms_idle(config, env):
    with database(config["database"]) as db:
        count = db.execute("SELECT COUNT(*) FROM hangout_members hm JOIN hangouts h ON hm.hangout_id=h.id WHERE hm.left_at IS NULL AND h.ended_at IS NULL").fetchone()[0]
    if count:
        return False
    def encode(value):
        return base64.urlsafe_b64encode(json.dumps(value, separators=(",", ":")).encode()).rstrip(b"=")
    now = int(time.time())
    token = encode({"alg": "HS256", "typ": "JWT"}) + b"." + encode({
        "iss": env["WISP_LIVEKIT_API_KEY"], "sub": "wisp-deployment",
        "nbf": now - 5, "exp": now + 60, "video": {"roomList": True}})
    signature = base64.urlsafe_b64encode(hmac.new(env["WISP_LIVEKIT_API_SECRET"].encode(), token, hashlib.sha256).digest()).rstrip(b"=")
    response = json_request(config["livekit_http_url"].rstrip("/") + "/twirp/livekit.RoomService/ListRooms", b"{}", {
        "Content-Type": "application/json", "Authorization": "Bearer " + (token + b"." + signature).decode()})
    return all(int(room.get("numParticipants", room.get("num_participants", 0))) == 0 for room in response.get("rooms", []))


def health(url, public=False):
    if public:
        # The host's public AAAA route is not reliable; verify its actual IPv4 path.
        data = subprocess.check_output(["/usr/bin/curl", "--ipv4", "--fail", "--silent", "--max-time", "10", url], stderr=subprocess.DEVNULL)
        value = json.loads(data)
    else:
        value = json_request(url)
    if not value.get("ok") or not value.get("database"):
        raise RuntimeError("Server health check failed")


def service(action):
    subprocess.run(["/usr/bin/systemctl", action, "wisp-server"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=30)


def verify_services():
    result = subprocess.check_output(["/usr/bin/systemctl", "is-active", "wisp-server", "livekit-server", "caddy"], stderr=subprocess.PIPE, timeout=10).decode().splitlines()
    if result != ["active", "active", "active"]:
        raise RuntimeError("A production service is not active")


def verify_running(binary):
    pid = subprocess.check_output(["/usr/bin/systemctl", "show", "wisp-server", "--property=MainPID", "--value"], timeout=10).decode().strip()
    if not pid.isdigit() or pid == "0" or digest(Path("/proc") / pid / "exe") != digest(binary):
        raise RuntimeError("Running server does not match installed build")


def migration_probe(config, env, binary):
    account = pwd.getpwnam("wisp")
    with tempfile.TemporaryDirectory(prefix="wisp-migration-") as directory:
        folder = Path(directory)
        candidate = folder / "wisp-server"
        db_path = folder / "probe.sqlite3"
        shutil.copy2(binary, candidate)
        candidate.chmod(0o755)
        backup_database(config["database"], db_path)
        os.chown(folder, account.pw_uid, account.pw_gid)
        os.chown(db_path, account.pw_uid, account.pw_gid)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        probe_env = dict(env, WISP_DATABASE_URL="sqlite://" + str(db_path), WISP_SERVER_ADDR=f"127.0.0.1:{port}")
        with (folder / "probe.log").open("wb") as log:
            process = subprocess.Popen([str(candidate)], env=probe_env, cwd=folder, user=account.pw_uid, group=account.pw_gid, extra_groups=[], stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if process.poll() is not None:
                        raise RuntimeError("Candidate failed its migration probe")
                    try:
                        health(f"http://127.0.0.1:{port}/healthz")
                        return inspect_database(db_path)
                    except (OSError, ValueError, RuntimeError):
                        time.sleep(0.1)
                raise RuntimeError("Candidate migration probe timed out")
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                shutil.copy2(folder / "probe.log", STATE / "last-probe.log")


def install(source, destination):
    temporary = destination.with_name(destination.name + ".deploy-next")
    shutil.copy2(source, temporary)
    temporary.chmod(0o755)
    temporary.replace(destination)


def apply():
    with locked():
        state = read_state()
        if state["phase"] != "queued":
            return state
        config = json.loads(CONFIG.read_text())
        env = environment(config["environment"])
        binary = Path(config["binary"])
        candidate = STATE / "pending-server"
        previous_schema = None
        backup = None
        stopped = False
        try:
            if digest(candidate) != state["sha256"]:
                raise RuntimeError("Queued build checksum changed")
            if digest(binary) == state["sha256"]:
                verified = inspect_database(config["database"])
            else:
                try:
                    idle = rooms_idle(config, env)
                except (OSError, ValueError):
                    return dict(state, waiting_for="voice service health")
                if not idle:
                    return dict(state, waiting_for="voice rooms to empty")
                expected = migration_probe(config, env, candidate)
                previous_schema = inspect_database(config["database"])
                backup = Path(tempfile.mkdtemp(prefix="github-" + state["commit"][:12] + "-", dir=config["backups"]))
                shutil.copy2(binary, backup / "wisp-server")
                shutil.copy2(config["environment"], backup / "server.env")
                # Recheck both control membership and LiveKit immediately before stopping.
                try:
                    idle = rooms_idle(config, env)
                except (OSError, ValueError):
                    return dict(state, waiting_for="voice service health")
                if not idle:
                    return dict(state, waiting_for="voice rooms to empty")
                service("stop")
                stopped = True
                backup_database(config["database"], backup / "before.sqlite3")
                install(candidate, binary)
                service("start")
                for attempt in range(100):
                    try:
                        health(config["local_health_url"])
                        break
                    except (OSError, ValueError, RuntimeError):
                        if attempt == 99:
                            raise
                        time.sleep(0.2)
                verified = inspect_database(config["database"])
                if verified != expected:
                    raise RuntimeError("Deployed schema differs from the migration probe")
            verify_services()
            verify_running(binary)
            health(config["public_health_url"], public=True)
            state.update(phase="deployed", deployed_at=int(time.time()), schema=verified["schema"])
            if backup:
                state["backup"] = str(backup)
            atomic_json(STATE / "status.json", state)
            atomic_json(STATE / "deployed.json", state)
            candidate.unlink(missing_ok=True)
            return state
        except Exception as error:
            reason = str(error) if isinstance(error, RuntimeError) else type(error).__name__
            state.update(phase="failed", error=reason, failed_at=int(time.time()))
            # Never automatically restore a database: it may contain new user writes.
            if stopped and backup:
                state["backup"] = str(backup)
                try:
                    if inspect_database(config["database"]) == previous_schema:
                        service("stop")
                        install(backup / "wisp-server", binary)
                        service("start")
                        state["binary_rolled_back"] = True
                except Exception:
                    state["binary_rolled_back"] = False
            atomic_json(STATE / "status.json", state)
            return state


if __name__ == "__main__":
    os.umask(0o077)
    if os.geteuid() != 0:
        sys.exit("Deployment helper requires root")
    try:
        if sys.argv[1:] == ["status"]:
            with locked():
                result = read_state()
        elif sys.argv[1:] == ["apply"]:
            result = apply()
        elif len(sys.argv) == 5 and sys.argv[1] == "stage":
            result = stage(*sys.argv[2:], sys.stdin.buffer)
        else:
            raise ValueError("Unsupported deployment command")
        print(json.dumps(result))
        sys.exit(1 if result.get("phase") == "failed" else 0)
    except Exception as error:
        # Do not expose environment values, LiveKit tokens, or database content.
        sys.exit("Deployment request failed: " + type(error).__name__)
