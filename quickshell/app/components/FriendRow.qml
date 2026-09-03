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
    id: friendName
    anchors.left: dot.right
    anchors.leftMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    text: String(root.friend.display_name || "")
    color: root.friend.online ? root.theme.foreground : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
    width: root.theme.terminal ? Math.max(0, statusText.x - x - root.theme.spacing.md) : implicitWidth
    elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
  }

  Text {
    id: statusText
    anchors.right: parent.right
    anchors.rightMargin: root.theme.terminal ? messageButton.width + root.theme.spacing.lg * 2 : root.theme.space(76)
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

  Rectangle {
    id: messageButton
    width: root.theme.terminal ? messageLabel.implicitWidth + root.theme.space(18) : root.theme.space(66)
    height: root.theme.space(28)
    anchors.right: parent.right
    anchors.rightMargin: root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    radius: root.theme.cornerRadius
    color: messageMouse.containsMouse
      ? root.theme.alpha(root.theme.accent, 0.25)
      : root.theme.alpha(root.theme.foreground, 0.07)

    Text {
      id: messageLabel
      anchors.centerIn: parent
      text: "Message"
      color: root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }

    MouseArea {
      id: messageMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.bridge.openDirect(root.friend.display_name)
    }
  }
}
