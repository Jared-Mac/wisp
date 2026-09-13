#!/usr/bin/env python3
"""Post a main update. Never print the webhook, response, or commit authors."""
import json
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

NOTIFY_HERE = "<!-- wisp-notify: here -->"


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def notes(before, after):
    if not re.fullmatch(r"[0-9a-f]{40}", after):
        raise ValueError("Invalid release commit")
    if not re.fullmatch(r"[0-9a-f]{40}", before) or before == "0" * 40:
        before = git("rev-parse", f"{after}^")
    paths = git("diff", "--name-only", "--diff-filter=A", before, after, "--", "docs/patch-notes/").splitlines()
    documents = [git("show", f"{after}:{p}") for p in paths if p.endswith(".md")]
    if documents:
        # Only explicit metadata on a new release note opts into a notification.
        # Text in commit subjects or the note body cannot enable mentions.
        notify_here = any(document.splitlines()[0] == NOTIFY_HERE for document in documents if document)
        documents = [document.removeprefix(NOTIFY_HERE).strip() for document in documents]
        return "\n\n".join(documents), notify_here
    subjects = git("log", "--no-merges", "--format=%s", f"{before}..{after}").splitlines()
    # Squash, fast-forward and direct pushes all get a useful fallback.
    return "\n".join(f"- {s}" for s in dict.fromkeys(subjects)) or "Maintenance update.", False


def payload(body, repo, after, *, notify_here=False):
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("Invalid repository")
    body = body.strip()
    if len(body) > 3700:
        body = body[:3650].rsplit("\n", 1)[0] + "\n\nSee GitHub for the full changes."
    message = {"allowed_mentions": {"parse": []}, "embeds": [{
        "title": "Wisp · merged to main", "description": body,
        "url": f"https://github.com/{repo}/commit/{after}", "color": 0x8EC5EE,
        "footer": {"text": f"{after[:8]} · Build and deployment status are tracked on GitHub"},
    }]}
    if notify_here:
        # Mentions in embeds do not notify. Keep the only mention-enabled
        # message content fixed so no user, role or @everyone ping is added.
        message["content"] = "@here"
        message["allowed_mentions"] = {"parse": ["everyone"]}
    return message


def post(url, message):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc != "discord.com" or not re.fullmatch(r"/api/webhooks/\d+/[A-Za-z0-9_-]+", parsed.path):
        raise ValueError("Invalid Discord webhook secret")
    target = urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, parsed.path, "wait=true", ""))
    data = json.dumps(message).encode()
    for attempt in range(3):
        request = urllib.request.Request(target, data=data, headers={"Content-Type": "application/json", "User-Agent": "Wisp-Patch-Notes/1.0"})
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                result = json.load(response)
                if not result.get("id"):
                    raise RuntimeError("Discord did not confirm delivery")
                if message.get("content") == "@here" and result.get("mention_everyone") is not True:
                    raise RuntimeError("Discord delivered the notes but did not confirm the requested @here mention; inspect permissions before retrying")
                return
        except urllib.error.HTTPError as error:
            if error.code == 429 and attempt < 2:
                retry = json.loads(error.read(4096)).get("retry_after", 2)
                time.sleep(min(30, max(1, float(retry))))
                continue
            raise RuntimeError(f"Discord delivery failed (HTTP {error.code})") from None
        except (urllib.error.URLError, TimeoutError):
            # Delivery is uncertain; automatic retry could post duplicate notes.
            raise RuntimeError("Discord delivery could not be confirmed; inspect the channel before retrying") from None


if __name__ == "__main__":
    try:
        after = os.environ["WISP_AFTER"]
        body, notify_here = notes(os.environ["WISP_BEFORE"], after)
        message = payload(body, os.environ["GITHUB_REPOSITORY"], after, notify_here=notify_here)
        post(os.environ["WISP_DISCORD_PATCH_NOTES"], message)
        print("Discord confirmed patch-note delivery" + (" and the requested @here mention" if notify_here else ""))
    except Exception as error:
        # Do not print arbitrary network exceptions: they can contain the URL.
        if isinstance(error, (RuntimeError, ValueError)):
            print(str(error))
        else:
            print("Patch-note delivery failed; check workflow configuration")
        raise SystemExit(1) from None
