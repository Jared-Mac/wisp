#!/usr/bin/env python3
"""Bound the untrusted daemon stream before it enters Quickshell's process.

stdout contains @connected, validated JSON lines, or @rejected. The UI sends
@ack after consuming each JSON line; until then we stop reading the socket.
All other stdin lines are ordinary Wisp commands. No shell is involved.
"""
import json
import math
import os
import selectors
import socket
import sys

MAX_FRAME = 8 * 1024 * 1024
MAX_COMMAND = 256 * 1024
CHUNK = 16 * 1024
MAX_DEPTH = 32
MAX_NODES = 100_000
MAX_STRING = 16 * 1024
MAX_TEXT = 64 * 1024
COLLECTIONS = {
    "servers": 16, "server_states": 16, "messages": 2048,
    "conversations": 512, "friends": 512, "hangouts": 256,
    "members": 512, "sharing": 512, "spots": 256, "devices": 128,
    "knocks": 128, "room_invitations": 128, "reactions": 2048,
}
SNAPSHOT_ARRAYS = set(COLLECTIONS) - {"members", "sharing"}


class Rejected(ValueError):
    pass


def require(condition):
    if not condition:
        raise Rejected("Invalid or oversized IPC response")


def check_depth(frame):
    # Bound parser recursion before json.loads allocates the object tree.
    depth = 0
    quoted = escaped = False
    for byte in frame:
        if quoted:
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                quoted = False
        elif byte == 34:
            quoted = True
        elif byte in (91, 123):
            depth += 1
            require(depth <= MAX_DEPTH)
        elif byte in (93, 125):
            depth -= 1


def validate_tree(value):
    nodes = 0
    stack = [(value, "", 0)]
    while stack:
        item, field, depth = stack.pop()
        nodes += 1
        require(nodes <= MAX_NODES and depth <= MAX_DEPTH)
        if isinstance(item, dict):
            require(len(item) <= 512)
            for key, child in item.items():
                require(len(key) <= 128 and key not in ("__proto__", "prototype", "constructor"))
                stack.append((child, key, depth + 1))
        elif isinstance(item, list):
            require(len(item) <= COLLECTIONS.get(field, 512))
            stack.extend((child, "", depth + 1) for child in item)
        elif isinstance(item, str):
            limit = MAX_TEXT if field in ("payload", "text", "caption", "body") else MAX_STRING
            if field in ("id", "conversation_id", "server_id", "display_name", "content_type"):
                limit = 256
            elif field in ("name", "label", "server_name", "room_label"):
                limit = 512
            elif field in ("url", "qr") and item.startswith("data:image/"):
                limit = 1024 * 1024
            require(len(item) <= limit)
        elif isinstance(item, (int, float)) and not isinstance(item, bool):
            require(math.isfinite(item) and abs(item) <= 2**53 - 1)
        else:
            require(item is None or isinstance(item, bool))


def validate_snapshot(snapshot, nested=False):
    require(isinstance(snapshot, dict) and isinstance(snapshot.get("self"), dict))
    for field in SNAPSHOT_ARRAYS:
        if field in snapshot:
            require(isinstance(snapshot[field], list))
            require(all(isinstance(entry, dict) for entry in snapshot[field]))
    for field in ("friends", "hangouts"):
        require(isinstance(snapshot.get(field), list))
    for entry in snapshot.get("conversations", []):
        require(isinstance(entry.get("id"), str))
        require(isinstance(entry.get("members", []), list))
    for message in snapshot.get("messages", []):
        require(isinstance(message.get("id"), str))
        require(isinstance(message.get("conversation_id"), str))
        require(isinstance(message.get("sender"), dict))
        require(isinstance(message.get("content_type"), str))
        if message["content_type"] == "text/plain":
            require(isinstance(message.get("payload"), str))
    require(not nested or not snapshot.get("server_states"))
    for state in snapshot.get("server_states", []):
        require(isinstance(state.get("server"), dict))
        validate_snapshot(state, nested=True)


def validate_frame(frame):
    require(0 < len(frame) <= MAX_FRAME)
    check_depth(frame)
    message = json.loads(frame.decode("utf-8"))
    validate_tree(message)
    require(isinstance(message, dict) and message.get("v") == 1)
    kind = message.get("type")
    if kind == "snapshot":
        validate_snapshot(message.get("snapshot"))
    elif kind == "event":
        require(isinstance(message.get("name"), str))
        payload = message.get("payload")
        require(isinstance(payload, dict))
        if "snapshot" in payload:
            validate_snapshot(payload["snapshot"])
    elif kind == "result":
        require(isinstance(message.get("id"), str) and isinstance(message.get("ok"), bool))
        require(message.get("error") is None or isinstance(message["error"], dict))
    elif kind == "hello":
        require(isinstance(message.get("daemon"), str) and isinstance(message.get("profile"), str))
    else:
        raise Rejected("Unknown IPC response")


def relay(path):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as peer, selectors.DefaultSelector() as poll:
        peer.settimeout(5)
        peer.connect(path)
        poll.register(peer, selectors.EVENT_READ)
        poll.register(sys.stdin.fileno(), selectors.EVENT_READ)
        sys.stdout.buffer.write(b"@connected\n")
        sys.stdout.buffer.flush()
        incoming = bytearray()
        commands = bytearray()
        waiting = False
        while True:
            if not waiting:
                end = incoming.find(b"\n")
                if end >= 0:
                    require(end <= MAX_FRAME)
                    frame = bytes(incoming[:end])
                    del incoming[:end + 1]
                    validate_frame(frame)
                    sys.stdout.buffer.write(frame + b"\n")
                    sys.stdout.buffer.flush()
                    waiting = True
                    poll.unregister(peer)
                else:
                    require(len(incoming) <= MAX_FRAME)
            for key, _ in poll.select():
                if key.fileobj is peer:
                    # Read at most one byte beyond the ceiling, including when
                    # the sender never terminates its frame with a newline.
                    chunk = peer.recv(min(CHUNK, MAX_FRAME + 1 - len(incoming)))
                    if not chunk:
                        require(not incoming)
                        return
                    incoming.extend(chunk)
                else:
                    chunk = os.read(sys.stdin.fileno(), CHUNK)
                    if not chunk:
                        return
                    commands.extend(chunk)
                    while b"\n" in commands:
                        end = commands.index(b"\n")
                        require(end <= MAX_COMMAND)
                        command = bytes(commands[:end])
                        del commands[:end + 1]
                        if command == b"@ack":
                            require(waiting)
                            waiting = False
                            poll.register(peer, selectors.EVENT_READ)
                        else:
                            peer.sendall(command + b"\n")
                    require(len(commands) <= MAX_COMMAND)


if __name__ == "__main__":
    try:
        relay(sys.argv[1])
    except (ValueError, RecursionError, OverflowError):
        # Never echo the frame, exception, paths, or account data into logs.
        sys.stdout.buffer.write(b"@rejected\n")
        sys.stdout.buffer.flush()
        sys.exit(2)
    except OSError:
        sys.exit(1)
