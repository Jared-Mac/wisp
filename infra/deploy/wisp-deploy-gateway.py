#!/usr/bin/python3
"""Forced SSH command: no shell, forwarding, or arbitrary sudo commands."""
import os
import re
import shlex
import subprocess
import sys


def command(value):
    args = shlex.split(value)
    if args == ["status"]:
        return args
    if (len(args) == 4 and args[0] == "stage"
            and re.fullmatch(r"[0-9a-f]{40}", args[1])
            and re.fullmatch(r"[0-9a-f]{64}", args[2])
            and re.fullmatch(r"[0-9]{1,12}", args[3])):
        return args
    raise ValueError("Only deployment submission and status are available")


if __name__ == "__main__":
    try:
        args = command(os.environ.get("SSH_ORIGINAL_COMMAND", ""))
    except ValueError:
        sys.exit("Unsupported deployment command")
    sys.exit(subprocess.call(["/usr/bin/sudo", "-n", "/usr/local/sbin/wisp-deploy-server", *args]))
