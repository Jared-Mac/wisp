import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var bridge
  required property var theme
  property bool collapsible: false
  property bool collapsed: false
  signal toggled()
  signal createRequested()
  implicitHeight: theme.space(30)
  Button {
    id: toggle; objectName: "rooms-collapse"
    anchors.left: parent.left; anchors.right: create.left; anchors.rightMargin: root.theme.spacing.xs
    height: parent.height; enabled: root.collapsible
    Accessible.name: root.collapsed ? "Expand rooms" : "Collapse rooms"
    onClicked: root.toggled()
    background: Rectangle {
      color: toggle.hovered ? root.theme.alpha(root.theme.foreground, 0.06) : "transparent"
      border.width: toggle.visualFocus ? 1 : 0; border.color: root.theme.focusBorder
    }
    contentItem: Text {
      objectName: "roomsSectionHeader"; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
      text: (root.collapsible ? (root.collapsed ? "▸ " : "▾ ") : "") + (root.theme.tui ? "/rooms · " : "ROOMS · ") + root.bridge.roomCount
      color: root.theme.roomSectionColor; font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption; font.bold: true
    }
  }
  ChatButton {
    id: create; objectName: "createRoomButton"
    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
    theme: root.theme; text: "+"; implicitWidth: root.theme.space(30)
    enabled: root.bridge.activeServer.connected !== false
    Accessible.name: "Create a room"; ToolTip.visible: hovered; ToolTip.text: "Create a room"
    onClicked: root.createRequested()
  }
}
