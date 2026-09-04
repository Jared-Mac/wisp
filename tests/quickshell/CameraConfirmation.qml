import QtQuick
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  property bool fixtureReady: false
  property int step: 0
  property var dialog: null
  function check(value, message) { if (!value) { failed = true; console.error("CAMERA_TEST_FAILED " + message) } }
  function find(item, name) {
    if (item.objectName === name) return item
    for (var child of (item.data || [])) { var result = find(child, name); if (result) return result }
    return null
  }
  function state(room, device, active) {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.hangout_id = room
    data.self.media.camera.selected_device_id = device
    data.self.media.camera.devices = [{id:device,name:"Fixture camera"}]
    data.self.media.camera.active = active
    data.hangouts = [{id:"porch",label:"Porch",members:[]}]
    bridge.applySnapshot(data)
  }
  function starts() { return bridge.sent.filter(function(command) { return command.name === "camera" && command.args.enabled }).length }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) { sent.push({name:name,args:args}); return "fixture" }
  }
  Wisp.WispTheme { id: theme; profile: "terminal" }
  Component {
    id: fakePreview
    Rectangle {
      color: "#1c202b"
      property bool captureActive: true
      readonly property bool ready: test.fixtureReady
      readonly property string error: ""
      Text { anchors.centerIn: parent; text: "LOCAL PREVIEW FIXTURE"; color: "#8d96a8" }
    }
  }
  // Parse the real local-preview component without ever creating a Camera.
  Loader { id: previewSmoke; active: false; source: "app/components/CameraPreview.qml" }
  FloatingWindow {
    id: window; visible: true; implicitWidth: 460; implicitHeight: 800
    Wisp.WispContent {
      id: content; anchors.fill: parent; bridge: bridge; theme: theme
      logoSource: Qt.resolvedUrl("app/assets/waveform.svg")
      presentation: Quickshell.env("WISP_CAMERA_PRESENTATION") || "app"
    }
  }
  Timer {
    interval: 220; running: true; repeat: true
    onTriggered: {
      if (test.step === 0) {
        var component = Qt.createComponent("app/components/CameraPreview.qml")
        test.check(component.status !== Component.Error, "real preview QML compiles: " + component.errorString())
        test.dialog = test.find(content, "cameraConfirmation")
        test.check(!!test.dialog, "dialog is hosted by this presentation")
        test.dialog.previewComponent = fakePreview
        content.requestCamera()
        test.check(!test.dialog.visible && test.starts() === 0, "not in room cannot preview or publish")
        test.state("porch", "fixture-camera", false)
        content.requestCamera()
        test.check(test.dialog.visible && test.dialog.destination === "Porch", "camera action opens destination confirmation")
        test.dialog.startSharing()
        test.check(!test.dialog.submitting && test.starts() === 0, "cannot start before preview ready")
      } else if (test.step === 1) {
        test.fixtureReady = true
        test.dialog.close()
        test.check(!test.dialog.previewRequested && test.starts() === 0, "Cancel stops preview without publishing")
      } else if (test.step === 2) {
        content.requestCamera(); test.dialog.startSharing()
        test.state("other", "fixture-camera", false)
      } else if (test.step === 3) {
        test.check(test.starts() === 0 && !test.dialog.visible, "room change cancels pending confirmation")
        test.state("porch", "fixture-camera", false)
        content.requestCamera()
        test.state("porch", "different-camera", false)
        test.check(!test.dialog.visible, "device change closes preview")
      } else if (test.step === 4) {
        test.state("porch", "fixture-camera", false)
        content.requestCamera()
        content.visible = false
        test.check(!test.dialog.previewRequested && test.starts() === 0, "hiding presentation cancels preview")
      } else if (test.step === 5) {
        content.visible = true
        content.requestCamera()
        var path = Quickshell.env("WISP_CAMERA_SCREENSHOT")
        if (path) test.dialog.contentItem.parent.grabToImage(function(result) { test.check(result.saveToFile(path), "confirmation screenshot") })
      } else if (test.step === 6) {
        test.dialog.startSharing()
        test.check(!test.dialog.previewRequested, "local preview released before publishing")
      } else if (test.step === 7) {
        test.check(test.starts() === 1, "explicit confirmation publishes exactly once")
        var command = bridge.sent[bridge.sent.length - 1]
        test.check(command.args.expected_hangout_id === "porch" && command.args.expected_camera_id === "fixture-camera", "publish pinned to confirmed target and device")
        test.state("porch", "fixture-camera", true)
        content.requestCamera()
        test.check(bridge.sent[bridge.sent.length - 1].args.enabled === false, "Camera off remains immediate")
        console.log(test.failed ? "CAMERA_TEST_FAILED" : "CAMERA_CONFIRMATION_OK")
        Qt.quit()
      }
      test.step++
    }
  }
}
