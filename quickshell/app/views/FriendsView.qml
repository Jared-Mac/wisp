import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal selected()
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs

  Text {
    text: "FRIENDS"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
  }

  Repeater {
    model: root.bridge.friends
    delegate: FriendRow {
      required property var modelData
      width: root.width
      friend: modelData
      bridge: root.bridge
      theme: root.theme
      onSelected: root.selected()
    }
  }
}
