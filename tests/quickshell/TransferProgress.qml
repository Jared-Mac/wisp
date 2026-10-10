import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  Wisp.WispBridge { id: bridge }
  function check(condition, message) {
    if (!condition) {
      console.error("TRANSFER_PROGRESS_FAILED: " + message)
      Qt.exit(1)
      throw new Error(message)
    }
  }
  function receive(id, bytes, total) {
    bridge.handleLine(JSON.stringify({v:1, type:"event", name:"file_transfer_progress",
      payload:{id:id, direction:"upload", bytes:bytes, total:total, extra:"not retained"}}))
  }
  Component.onCompleted: {
    for (var i = 0; i < 10000; i++) {
      receive(String(i), 1, 10)
      check(Object.keys(bridge.transferProgress).length <= 128, "repeated events exceeded the cap")
    }
    check(Object.keys(bridge.transferProgress).length === 128, "progress events reached the bridge")
    check(!bridge.transferProgress["upload:0"], "oldest entry evicted")
    check(bridge.transferLabel("upload", "9999") === " 10%", "progress label preserved")
    check(bridge.transferProgress["upload:9999"].extra === undefined, "extra payload not retained")
    bridge.daemonConnectedChanged()
    check(Object.keys(bridge.transferProgress).length === 0, "disconnect clears retained progress")
    receive("completed", 6000000000, 6000000000)
    check(bridge.transferLabel("upload", "completed") === " 100%", "completion remains briefly visible")
    receive("stalled", 1, 10)
    var stale = Object.assign({}, bridge.transferProgress)
    stale["upload:stalled"].expires = Date.now() - 1
    bridge.transferProgress = stale
  }
  Timer {
    interval: 1500; running: true
    onTriggered: {
      test.check(!bridge.transferProgress["upload:stalled"], "timer expires idle entries without incoming events")
      test.check(!!bridge.transferProgress["upload:completed"], "completion retained for five seconds")
    }
  }
  Timer {
    interval: 6500; running: true
    onTriggered: {
      test.check(Object.keys(bridge.transferProgress).length === 0, "timer expires completed transfers")
      console.log("TRANSFER_PROGRESS_OK")
      Qt.quit()
    }
  }
}
