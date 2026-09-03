import QtQuick

Item {
  id: root

  required property var bridge
  required property var theme
  property bool poppedOut: false

  signal popOutRequested()

  readonly property bool hasBroadcast: root.bridge.shareStarting
    || root.bridge.sharing || root.bridge.cameraStarting || root.bridge.cameraActive
  readonly property int edgeMargin: root.theme.spacing.xxl

  visible: hasBroadcast && !poppedOut

  function clampPosition() {
    previewStack.x = Math.max(edgeMargin,
      Math.min(previewStack.x, width - previewStack.width - edgeMargin))
    previewStack.y = Math.max(edgeMargin,
      Math.min(previewStack.y, height - previewStack.height - edgeMargin))
  }

  function dockToCorner() {
    previewStack.x = Math.max(edgeMargin,
      width - previewStack.width - edgeMargin)
    previewStack.y = Math.max(edgeMargin,
      height - previewStack.height - edgeMargin)
  }

  onWidthChanged: Qt.callLater(clampPosition)
  onHeightChanged: Qt.callLater(clampPosition)
  onVisibleChanged: if (visible) Qt.callLater(dockToCorner)

  Column {
    id: previewStack
    z: 10
    width: Math.max(root.theme.space(176),
      Math.min(root.theme.space(210), root.width - root.edgeMargin * 2))
    spacing: root.theme.spacing.lg
    onHeightChanged: Qt.callLater(root.clampPosition)

    LocalVideoPreview {
      width: parent.width
      theme: root.theme
      title: "Your screen"
      previewUrl: root.bridge.screenSharePreviewUrl
      starting: root.bridge.shareStarting
      active: root.bridge.sharing
      viewers: root.bridge.screenShareState.viewers || []
      dragTarget: previewStack
      dragMinimumX: root.edgeMargin
      dragMaximumX: Math.max(root.edgeMargin,
        root.width - previewStack.width - root.edgeMargin)
      dragMinimumY: root.edgeMargin
      dragMaximumY: Math.max(root.edgeMargin,
        root.height - previewStack.height - root.edgeMargin)
      onActionRequested: root.popOutRequested()
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
      dragTarget: previewStack
      dragMinimumX: root.edgeMargin
      dragMaximumX: Math.max(root.edgeMargin,
        root.width - previewStack.width - root.edgeMargin)
      dragMinimumY: root.edgeMargin
      dragMaximumY: Math.max(root.edgeMargin,
        root.height - previewStack.height - root.edgeMargin)
      onActionRequested: root.popOutRequested()
    }
  }
}
