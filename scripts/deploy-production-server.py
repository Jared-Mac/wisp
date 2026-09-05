#!/usr/bin/env python3
"""Submit a verified CI server binary through the restricted deployment account."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time


def main():
    binary, commit, sequence = sys.argv[1:]
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9]{1,12}", sequence):
        raise ValueError("Invalid release identity")
    config = json.loads(os.environ.pop("WISP_DEPLOY_CONFIG"))
    if not re.fullmatch(r"[A-Za-z0-9.-]+", config["host"]) or config["user"] != "wisp-deploy":
        raise ValueError("Invalid deployment destination")
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix="wisp-deploy-") as directory:
        key = Path(directory) / "key"
        known = Path(directory) / "known_hosts"
        key.write_text(config.pop("private_key"))
        known.write_text(config["known_hosts"])
        ssh = ["ssh", "-F", "/dev/null", "-4", "-i", str(key), "-o", "IdentitiesOnly=yes",
               "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
               "-o", "UserKnownHostsFile=" + str(known), "-o", "ConnectTimeout=10",
               "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3",
               config["user"] + "@" + config["host"]]
        def request(command, source=None):
            result = subprocess.run([*ssh, command], stdin=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
            if result.returncode:
                try:
                    failure = json.loads(result.stdout)
                except (ValueError, UnicodeError):
                    failure = {}
                if failure.get("phase") == "failed":
                    raise RuntimeError("Server deployment failed: " + str(failure.get("error", "inspect VPS deployment status")))
                raise RuntimeError("Deployment SSH request failed; inspect the deployment service or key access")
            return json.loads(result.stdout)
        with open(binary, "rb") as source:
            checksum = hashlib.file_digest(source, "sha256").hexdigest()
            source.seek(0)
            status = request(f"stage {commit} {checksum} {sequence}", source)
        for attempt in range(9):
            if status.get("sequence", int(sequence)) > int(sequence) or status["phase"] == "superseded":
                print("A newer main release has superseded this deployment.")
                return
            if status["phase"] == "deployed":
                if status.get("commit") != commit or status.get("sha256") != checksum:
                    raise RuntimeError("Deployment verification did not match this release")
                print(f"Deployed {commit[:12]}; schema {status['schema']}, binary, integrity, services and public health verified.")
                return
            if status["phase"] == "failed":
                raise RuntimeError("Server deployment failed; recoverable backup retained on the VPS")
            if attempt < 8:
                time.sleep(15)
                status = request("status")
        print(f"Queued {commit[:12]}. The VPS will deploy automatically when voice rooms are empty and checks pass.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Never print the secret configuration, key, or SSH arguments.
        reason = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        sys.exit("Production deployment failed: " + reason)
