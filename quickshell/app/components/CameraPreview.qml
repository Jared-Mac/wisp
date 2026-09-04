import QtQuick
import QtMultimedia

// This session has ONLY a local VideoOutput: no microphone, recorder, file,
// bridge or networking. Instantiated only after the user requests Camera on.
Item {
  id: root
  required property string deviceId
  property bool captureActive: true
  readonly property bool ready: camera.active && output.sourceRect.width > 0 && !error
  readonly property string error: camera.errorString || (selectedIndex < 0 ? "The selected camera is unavailable for preview." : "")
  function decodedId(value) {
    if (value instanceof ArrayBuffer) return String.fromCharCode.apply(null, new Uint8Array(value))
    return String(value)
  }
  readonly property int selectedIndex: {
    for (var i = 0; i < devices.videoInputs.length; i++)
      if (decodedId(devices.videoInputs[i].id) === deviceId) return i
    return -1
  }
  MediaDevices { id: devices }
  Camera {
    id: camera
    active: root.captureActive && root.selectedIndex >= 0
    cameraDevice: root.selectedIndex >= 0 ? devices.videoInputs[root.selectedIndex] : devices.defaultVideoInput
  }
  CaptureSession { camera: camera; videoOutput: output }
  VideoOutput { id: output; anchors.fill: parent; fillMode: VideoOutput.PreserveAspectFit }
}
