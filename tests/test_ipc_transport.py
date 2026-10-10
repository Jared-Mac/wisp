"""Exercise the real IPC relay against an untrusted Unix socket peer."""
import importlib.util
import json
import os
from pathlib import Path
import select
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "quickshell/app/ipc-transport.py"
spec = importlib.util.spec_from_file_location("transport", SCRIPT)
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)


def frame(value):
    return json.dumps(value, ensure_ascii=False).encode() + b"\n"


def result(value):
    return {"v": 1, "type": "result", "id": "test", "ok": True, "value": value}


def snapshot(**changes):
    return {"v": 1, "type": "snapshot", "snapshot": {
        "self": {}, "friends": [], "hangouts": [], **changes}}


class ValidationTests(unittest.TestCase):
    def validate(self, value):
        transport.validate_frame(frame(value)[:-1])

    def test_normal_envelopes_and_media(self):
        for value in [snapshot(), result({"url": "data:image/png;base64," + "A" * 100_000}),
                      result({"payload": "x" * transport.MAX_TEXT}),
                      {"v": 1, "type": "hello", "daemon": "wispd", "profile": "テスト"},
                      {"v": 1, "type": "event", "name": "state_changed", "payload": snapshot()}]:
            self.validate(value)

    def test_collection_boundaries_in_results_and_snapshots(self):
        for field, maximum in transport.COLLECTIONS.items():
            with self.subTest(field=field):
                self.validate(result({field: [{}] * maximum}))
                with self.assertRaises(transport.Rejected):
                    self.validate(result({field: [{}] * (maximum + 1)}))
        for value in [snapshot(friends=[{}] * 513), snapshot(messages=[{}] * 2049),
                      snapshot(conversations=[{}] * 513), snapshot(server_states=[{}] * 17)]:
            with self.assertRaises(transport.Rejected):
                self.validate(value)

    def test_fields_shapes_depth_and_node_budget(self):
        values = [None, [], {}, {"v": 1, "type": "snapshot", "snapshot": None},
                  snapshot(messages={}), snapshot(friends="bad"), snapshot(messages=[{}]),
                  result({"display_name": "x" * 257}), result({"payload": "x" * (transport.MAX_TEXT + 1)}),
                  result({"unknown": "x" * (transport.MAX_STRING + 1)}),
                  result({"__proto__": {"injected": True}}), result({"n": float("inf")}),
                  result({"rows": [[0] * 512] * 200})]
        nested = None
        for _ in range(33):
            nested = [nested]
        values.append(result(nested))
        for value in values:
            with self.subTest(value_type=type(value).__name__), self.assertRaises(transport.Rejected):
                self.validate(value)
        for data in [b"{", b"\xff", b'{"v":1,"type":"result","id":"x","ok":true,"value":1e999}']:
            with self.assertRaises(ValueError):
                transport.validate_frame(data)


class RelayTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = str(Path(self.temp.name) / "daemon.sock")
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(self.path)
        self.listener.listen()
        self.listener.settimeout(5)
        self.process = subprocess.Popen([sys.executable, "-I", "-u", str(SCRIPT), self.path],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.peer, _ = self.listener.accept()
        self.peer.settimeout(5)
        self.output = bytearray()
        self.assertEqual(self.line(), b"@connected")

    def tearDown(self):
        self.peer.close()
        self.listener.close()
        if self.process.poll() is None:
            self.process.terminate()
        self.process.communicate(timeout=5)
        self.temp.cleanup()

    def line(self):
        deadline = time.monotonic() + 5
        while b"\n" not in self.output:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, "relay response timed out")
            self.assertTrue(select.select([self.process.stdout], [], [], remaining)[0])
            data = os.read(self.process.stdout.fileno(), 16384)
            self.assertTrue(data, "relay closed stdout")
            self.output.extend(data)
        end = self.output.index(b"\n")
        value = bytes(self.output[:end])
        del self.output[:end + 1]
        return value

    def ack(self):
        self.process.stdin.write(b"@ack\n")
        self.process.stdin.flush()

    def rejected(self):
        self.assertEqual(self.line(), b"@rejected")
        self.assertEqual(self.process.wait(timeout=5), 2)
        self.assertEqual(self.process.stderr.read(), b"")
        self.assertEqual(self.peer.recv(1), b"")

    def test_fragmented_unicode_coalesced_frames_and_backpressure(self):
        message = frame(result({"text": "friends 💬 日本語"}))
        for byte in message:
            self.peer.sendall(bytes([byte]))
        self.assertEqual(self.line(), message[:-1])
        self.peer.sendall(frame(snapshot()) + frame(result({"text": "third"})))
        self.assertFalse(self.output)
        self.assertFalse(select.select([self.process.stdout], [], [], 0.1)[0])
        # Commands must still flow while the relay awaits the UI's receipt.
        command = b'{"v":1,"type":"command","name":"hello","args":{}}\n'
        self.process.stdin.write(command)
        self.process.stdin.flush()
        self.assertEqual(self.peer.recv(4096), command)
        self.ack()
        self.assertEqual(self.line(), frame(snapshot())[:-1])
        self.assertFalse(select.select([self.process.stdout], [], [], 0.1)[0])
        self.ack()
        self.assertEqual(self.line(), frame(result({"text": "third"}))[:-1])

    def test_unterminated_oversize_disconnects_before_parse(self):
        # Invalid JSON deliberately never ends: rejection must not wait for LF.
        self.peer.sendall(b"x" * (transport.MAX_FRAME + 1))
        self.rejected()

    def test_exact_byte_ceiling_is_accepted(self):
        message = result({"pieces": ["x" * transport.MAX_STRING] * 511 + [""]})
        missing = transport.MAX_FRAME - len(frame(message)) + 1
        self.assertLessEqual(missing, transport.MAX_STRING)
        message["value"]["pieces"][-1] = "x" * missing
        data = frame(message)
        self.assertEqual(len(data), transport.MAX_FRAME + 1)
        self.peer.sendall(data)
        self.assertEqual(self.line(), data[:-1])

    def test_invalid_collection_never_reaches_stdout(self):
        self.peer.sendall(frame(snapshot(friends=[{}] * 513)))
        self.rejected()

    def test_partial_frame_is_not_published_at_eof(self):
        self.peer.sendall(frame(snapshot())[:-1])
        self.peer.shutdown(socket.SHUT_WR)
        self.rejected()

    def test_utf8_bytes_count_toward_frame_limit(self):
        self.peer.sendall(b'"' + "💬".encode() * (transport.MAX_FRAME // 4))
        self.rejected()

    def test_command_buffer_is_bounded(self):
        self.process.stdin.write(b"x" * (transport.MAX_COMMAND + 1))
        self.process.stdin.flush()
        self.rejected()


@unittest.skipUnless(shutil.which("qs"), "Quickshell is unavailable")
class QmlTests(unittest.TestCase):
    def test_installed_transport_paths_handshake_and_rejection(self):
        with tempfile.TemporaryDirectory(prefix="wisp ipc ") as temporary:
            root = Path(temporary)
            app = root / "plugin/app"
            app.mkdir(parents=True)
            for name in ("WispIpcConnection.qml", "ipc-transport.py"):
                shutil.copy(REPO / "quickshell/app" / name, app / name)
            (root / "shell.qml").write_text('''import QtQuick
import Quickshell
import "plugin/app"
ShellRoot {
  property bool received: false
  WispIpcConnection {
    path: Quickshell.env("WISP_SOCKET")
    onConnectedChanged: if (connected) write('{"v":1,"type":"command","name":"hello"}\\n')
    onRead: function(line) {
      if (JSON.parse(line).value.text !== "💬 日本語") throw new Error("Unicode corrupted")
      received = true
    }
    onUnsafeResponse: {
      if (connected || !received) throw new Error("Invalid transport state")
      console.log("BOUNDED_IPC_OK")
      Qt.quit()
    }
  }
  Timer { interval: 5000; running: true; onTriggered: Qt.quit() }
}
''')
            path = str(root / "test.sock")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
                listener.bind(path)
                listener.listen()
                listener.settimeout(10)
                env = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "QT_QUICK_BACKEND": "software",
                       "XDG_CONFIG_HOME": str(root / "config"), "WISP_SOCKET": path}
                with subprocess.Popen(["qs", "--path", str(root)], env=env,
                                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT) as process:
                    try:
                        peer, _ = listener.accept()
                        with peer:
                            peer.settimeout(5)
                            self.assertIn(b'"hello"', peer.recv(4096))
                            peer.sendall(frame(result({"text": "💬 日本語"})) + frame(snapshot(messages=[{}] * 2049)))
                        output = process.communicate(timeout=10)[0].decode()
                        self.assertIn("BOUNDED_IPC_OK", output, output)
                        for error in ("ReferenceError", "TypeError", "Failed to load", "Error:"):
                            self.assertNotIn(error, output, output)
                    finally:
                        if process.poll() is None:
                            process.kill()
                            process.wait()


if __name__ == "__main__":
    unittest.main()
