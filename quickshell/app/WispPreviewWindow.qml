import QtQuick
import Quickshell
import "components"

FloatingWindow {
  id: root

  required property var bridge
  required property var theme

  signal dockRequested()

  readonly property bool hasBroadcast: root.bridge.shareStarting
    || root.bridge.sharing || root.bridge.cameraStarting || root.bridge.cameraActive

  title: "Wisp preview"
  implicitWidth: root.theme.space(380)
  implicitHeight: previewColumn.implicitHeight + root.theme.spacing.xxl * 2
  minimumSize: Qt.size(root.theme.space(260), root.theme.space(190))
  color: root.theme.background

  function reveal() {
    if (!hasBroadcast) return
    visible = true
    minimized = false
    Qt.callLater(function() {
      if (root.contentItem && root.contentItem.window)
        root.contentItem.window.requestActivate()
    })
  }

  onHasBroadcastChanged: {
    if (!hasBroadcast && visible) {
      visible = false
      dockRequested()
    }
  }
  onClosed: dockRequested()

  Rectangle {
    anchors.fill: parent
    color: root.theme.background
    SurfaceOutline { theme: root.theme; radius: 0 }

    Column {
      id: previewColumn
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      spacing: root.theme.spacing.lg

      LocalVideoPreview {
        width: parent.width
        theme: root.theme
        title: "Your screen"
        previewUrl: root.bridge.screenSharePreviewUrl
        starting: root.bridge.shareStarting
        active: root.bridge.sharing
        viewers: root.bridge.screenShareState.viewers || []
        actionMode: "dock"
        onActionRequested: root.dockRequested()
      }

      LocalVideoPreview {
        width: parent.width
        theme: root.theme
        title: "Your camera"
        previewUrl: root.bridge.cameraPreviewUrl
        starting: root.bridge.cameraStarting
        active: root.bridge.cameraActive
        viewers: root.bridge.cameraState.viewers || []
        mirrored: true
        actionMode: "dock"
        onActionRequested: root.dockRequested()
      }
    }
  }
}
