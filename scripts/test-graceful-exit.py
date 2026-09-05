#!/usr/bin/env python3
"""Real Quickshell exit and fake pw-play; no daemon, account, or audio output."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]
for mode in ["connected", "idle", "muted", "already-left"]:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        app = root / "app"
        shutil.copytree(REPO / "quickshell/app", app)
        # Offscreen Qt has no layer-shell backend; the real exit host remains unchanged.
        panel = app / "WispPanelWindow.qml"
        panel_source = panel.read_text().replace("PanelWindow {", "FloatingWindow {", 1)
        a = panel_source.index("  focusable: true")
        b = panel_source.index("  function reveal()", a)
        panel.write_text(panel_source[:a] + panel_source[b:])
        native = app / "native/WispVideo"
        native.mkdir(parents=True)
        for name in ["libwispvideo.so", "qmldir"]:
            shutil.copy2(REPO / "target/video-ui" / name, native / name)
        (root / "config/wisp").mkdir(parents=True)
        (root / "bin").mkdir()
        player = root / "bin/pw-play"
        player.write_text('''#!/usr/bin/env bash
set -eu
name=${3##*/}
echo "start $name" >> "$WISP_EXIT_SOUND_LOG"
if [[ "$name" == message.wav ]]; then sleep 10; else sleep 0.3; fi
echo "end $name" >> "$WISP_EXIT_SOUND_LOG"
''')
        player.chmod(0o755)
        bridge = app / "WispBridge.qml"
        source = bridge.read_text()
        start = source.index("  function send(name, args) {")
        end = source.index("\n  function replaceEntry", start)
        source = source[:start] + '''  function send(name, args) {
    voiceRecovery.manualCommand(name, args || {})
    console.log("EXIT_COMMAND " + name)
    return "fake-" + (++requestId)
  }
''' + source[end:]
        fixture = '''
  Timer { interval: 200; running: true; onTriggered: {
    var data = JSON.parse(JSON.stringify(root.snapshot))
    data.self.hangout_id = "voice"; data.self.media.livekit_connected = true
    root.notificationMuted = Quickshell.env("WISP_EXIT_MODE") === "muted"
    root.applySnapshot(data, "snapshot")
    if (Quickshell.env("WISP_EXIT_MODE") === "idle") {
      root.voiceRecovery.wasConnected = false; data.self.hangout_id = null
      data.self.media.livekit_connected = false; root.snapshot = data
    } else if (Quickshell.env("WISP_EXIT_MODE") === "already-left") {
      root.playNotificationSound("message")
      var left = JSON.parse(JSON.stringify(data)); left.self.hangout_id = null
      left.self.media.livekit_connected = false; root.applySnapshot(left, "voice_left")
    } else root.playNotificationSound("message")
    console.log("EXIT_FIXTURE_READY")
  } }
'''
        source = source[:source.rfind("}")] + fixture + "}\n"
        bridge.write_text(source)
        env = dict(os.environ, XDG_CONFIG_HOME=str(root / "config"),
                   WISP_SOCKET=str(root / "no-daemon.sock"), WISP_QUICKSHELL_PATH=str(app),
                   QML_IMPORT_PATH=str(app / "native"), QT_QPA_PLATFORM="offscreen",
                   QT_QUICK_BACKEND="software", WISP_SOUND_DIR=str(app / "assets"),
                   WISP_EXIT_SOUND_LOG=str(root / "sounds"), WISP_EXIT_MODE=mode, WISP_PRIMARY_SCREEN="fixture",
                   PATH=str(root / "bin") + ":" + os.environ["PATH"])
        log = root / "ui.log"
        with log.open("w") as output:
            process = subprocess.Popen(["qs", "--path", str(app)], env=env, stdout=output, stderr=output)
            try:
                for _ in range(100):
                    if "EXIT_FIXTURE_READY" in log.read_text():
                        break
                    if process.poll() is not None:
                        raise AssertionError(log.read_text())
                    time.sleep(0.05)
                else:
                    raise AssertionError("Exit fixture did not become ready: " + log.read_text())
                subprocess.run(["qs", "--path", str(app), "ipc", "call", "dev.wisp", "quitForSocket", str(root / "another.sock")], env=env, check=True, capture_output=True, timeout=2)
                assert process.poll() is None, "Another daemon must not close this UI"
                subprocess.run(["bash", str(REPO / "scripts/wisp-ui.sh"), "quit"], env=env,
                               check=True, capture_output=True, timeout=7)
                process.wait(timeout=2)
                lines = (root / "sounds").read_text().splitlines() if (root / "sounds").exists() else []
                expected = 1 if mode in ["connected", "already-left"] else 0
                assert lines.count("start self_leave.wav") == expected, (mode, lines)
                assert lines.count("end self_leave.wav") == expected, (mode, lines)
                if mode in ["connected", "muted"]:
                    assert "EXIT_COMMAND leave" in log.read_text()
                for error in ["TypeError", "ReferenceError", "Binding loop", "Cannot assign", "Failed to load"]:
                    assert error not in log.read_text(), log.read_text()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
        print(f"Graceful exit passed: {mode}")
