import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  signal roomLeft()
  signal cameraRequested()
  readonly property bool collapsed: bridge.workspaceLayout.trayRoomsCollapsed
  spacing: theme.spacing.xs

  Button {
    id: toggle
    objectName: "rooms-collapse"
    width: parent.width; height: root.theme.space(root.theme.tui ? 26 : 30)
    Accessible.name: root.collapsed ? "Expand rooms" : "Collapse rooms"
    onClicked: root.bridge.workspaceLayout.trayRoomsCollapsed = !root.collapsed
    background: Rectangle {
      color: toggle.hovered ? root.theme.alpha(root.theme.foreground, 0.06) : "transparent"
      radius: root.theme.cornerRadius
      border.width: toggle.visualFocus ? 1 : 0
      border.color: root.theme.focusBorder
    }
    contentItem: Item {
      Text {
        anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
        text: root.theme.tui ? "┌─ 01: /rooms" : "ROOMS"; color: root.theme.roomSectionColor
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption; font.weight: Font.Bold
        font.letterSpacing: root.theme.terminal ? 1 : 0
      }
      Text {
        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
        text: root.collapsed ? "▸" : "▾"
        color: root.theme.muted; font.pixelSize: root.theme.font.body
      }
    }
  }
  Column {
    width: parent.width
    visible: !root.collapsed
    spacing: root.theme.spacing.xs
    NowView {
      width: parent.width; showHeader: false
      bridge: root.bridge; theme: root.theme
      onJoined: root.joined()
      onRoomLeft: root.roomLeft()
      onCameraRequested: root.cameraRequested()
    }
    SpotsView {
      width: parent.width
      bridge: root.bridge; theme: root.theme
      onJoined: root.joined()
    }
  }
}
