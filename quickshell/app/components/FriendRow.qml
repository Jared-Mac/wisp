import QtQuick

Item {
  id: root
  required property var friend
  required property var bridge
  required property var theme
  signal selected()

  readonly property bool canRequest: root.friend.online &&
    (root.friend.presence === "open" || root.friend.presence === "knock")

  implicitHeight: root.theme.space(42)

  Rectangle {
    anchors.fill: parent
    radius: root.theme.cornerRadius
    color: mouse.containsMouse ? root.theme.alpha(root.theme.foreground, 0.07) : "transparent"
  }

  PresenceDot {
    id: dot
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    presence: root.friend.online ? String(root.friend.presence) : "closed"
    theme: root.theme
  }

  Text {
    anchors.left: dot.right
    anchors.leftMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    text: String(root.friend.display_name || "")
    color: root.friend.online ? root.theme.foreground : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
  }

  Text {
    anchors.right: parent.right
    anchors.rightMargin: root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    text: root.friend.online ? String(root.friend.presence) : "offline"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: root.canRequest ? Qt.PointingHandCursor : Qt.ArrowCursor
    onClicked: if (root.canRequest) {
      root.bridge.joinFriend(root.friend.display_name)
      root.selected()
    }
  }
}
