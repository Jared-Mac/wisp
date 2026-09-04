import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  objectName: "cameraConfirmation"
  required property var bridge
  required property var theme
  property bool hostVisible: false
  // Injectable local fixture; production defaults to the hardware preview.
  property Component previewComponent: null
  property string roomId: ""
  property string deviceId: ""
  property string destination: ""
  property bool previewRequested: false
  property bool submitting: false
  property bool previewTimedOut: false
  readonly property bool targetValid: !!roomId && String(bridge.selfState.hangout_id || "") === roomId
    && String(bridge.cameraState.selected_device_id || "") === deviceId
    && !bridge.cameraActive && !bridge.cameraStarting
  readonly property bool previewReady: !!preview.item && !!preview.item.ready
  readonly property string previewError: previewTimedOut ? "No camera frames arrived. Cancel and check the selected camera in Settings." : preview.status === Loader.Error ? "Camera preview could not load. Qt Multimedia is required." : preview.item ? String(preview.item.error || "") : ""
  onTargetValidChanged: if (visible && !targetValid) close()
  onHostVisibleChanged: if (!hostVisible) close()
  function confirm() {
    if (!hostVisible || bridge.cameraActive || bridge.cameraStarting || visible) return
    roomId = String(bridge.selfState.hangout_id || "")
    deviceId = String(bridge.cameraState.selected_device_id || "")
    if (!roomId || !deviceId) { bridge.lastError = !roomId ? "Join a room before starting the camera." : "Select a camera in Settings first."; return }
    var room = (bridge.hangouts || []).find(function(value) { return String(value.id) === root.roomId })
    if (!room) { bridge.lastError = "Room information is unavailable. Reopen the camera confirmation after reconnecting."; return }
    destination = room.label && room.label !== "Hangout" ? String(room.label) : (room.members || []).map(function(member) { return member.display_name }).join(" + ") || "Current room"
    submitting = false
    previewTimedOut = false
    open()
    previewRequested = true
  }
  function startSharing() {
    if (!visible || !hostVisible || !targetValid || !previewReady || submitting) return
    submitting = true
    // Release the local device before handing it to the publishing backend.
    if (preview.item) preview.item.captureActive = false
    previewRequested = false
    handoff.restart()
  }
  onAboutToHide: { previewRequested = false; handoff.stop(); submitting = false }
  Timer {
    interval: 8000
    running: root.previewRequested && !root.previewReady && !root.previewError
    onTriggered: { root.previewTimedOut = true; root.previewRequested = false }
  }
  Timer {
    id: handoff; interval: 150
    onTriggered: {
      if (root.visible && root.hostVisible && root.targetValid && root.submitting)
        root.bridge.send("camera", {enabled: true, expected_hangout_id: root.roomId, expected_camera_id: root.deviceId})
      root.close()
    }
  }
  parent: Overlay.overlay
  x: parent ? (parent.width - width) / 2 : 0
  y: parent ? (parent.height - height) / 2 : 0
  width: parent ? Math.min(parent.width - 24, theme.space(460)) : theme.space(460)
  modal: true; focus: true
  closePolicy: Popup.CloseOnEscape
  title: "Share your camera?"
  ThemeControlStyle { theme: root.theme; control: root; outline: true }
  font.family: theme.font.family; font.pixelSize: theme.font.body
  palette.window: theme.surface; palette.windowText: theme.foreground
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius }
  contentItem: Column {
    spacing: root.theme.spacing.lg
    Text {
      width: parent.width; wrapMode: Text.Wrap
      text: "Stream to: " + root.destination
      color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    }
    Rectangle {
      width: parent.width; height: Math.min(width * 9 / 16, root.theme.space(210))
      color: root.theme.background; radius: root.theme.cornerRadius
      Loader {
        id: preview
        objectName: "cameraConfirmationPreview"
        anchors.fill: parent; active: root.previewRequested && root.visible && root.hostVisible
        sourceComponent: root.previewComponent
        onActiveChanged: if (active && !root.previewComponent) setSource("CameraPreview.qml", {deviceId: root.deviceId})
      }
      Text {
        anchors.centerIn: parent; width: parent.width - root.theme.space(24)
        visible: !root.previewReady
        horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap
        text: root.previewError || (root.submitting ? "Starting camera…" : "Preparing local preview…")
        color: root.previewError ? root.theme.danger : root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      text: "Only you can see this preview. Your camera is not being shared yet."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Flow {
      width: parent.width; spacing: root.theme.spacing.md
      ChatButton { objectName: "cameraCancel"; theme: root.theme; text: "Cancel"; onClicked: root.close() }
      ChatButton {
        objectName: "cameraStartSharing"
        theme: root.theme; primary: true
        text: "Start Sharing Camera"
        enabled: root.targetValid && root.previewReady && !root.submitting
        onClicked: root.startSharing()
      }
    }
  }
}
