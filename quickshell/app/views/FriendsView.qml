import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal selected()
  property bool collapsible: false
  readonly property bool collapsed: collapsible && bridge.friendPreferences.collapsed
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs

  Button {
    id: collapseButton
    objectName: "friends-collapse"
    width: parent.width
    height: root.collapsible ? root.theme.space(30) : root.theme.space(20)
    enabled: root.collapsible
    Accessible.name: root.collapsed ? "Expand friends" : "Collapse friends"
    onClicked: root.bridge.friendPreferences.toggleCollapsed()
    background: Rectangle {
      color: collapseButton.hovered && root.collapsible ? root.theme.alpha(root.theme.foreground, 0.06) : "transparent"
      radius: root.theme.cornerRadius
      border.width: collapseButton.visualFocus ? 1 : 0
      border.color: root.theme.focusBorder
    }
    contentItem: Item {
      Text {
        anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
        text: "FRIENDS" + (root.collapsible ? " · " + root.bridge.friends.length : "")
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption; font.weight: Font.Bold
        font.letterSpacing: root.theme.terminal ? 1 : 0
      }
      Text {
        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
        visible: root.collapsible; text: root.collapsed ? "▸" : "▾"
        color: root.theme.muted; font.pixelSize: root.theme.font.body
      }
    }
  }

  Repeater {
    model: root.collapsed ? [] : root.bridge.sortedFriends
    delegate: FriendRow {
      required property var modelData
      width: root.width
      friend: modelData
      bridge: root.bridge
      theme: root.theme
      onSelected: root.selected()
    }
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    visible: root.bridge.friendPreferences.error !== ""
    text: root.bridge.friendPreferences.error
    color: root.theme.danger; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
