import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var friend
  required property var bridge
  required property var theme
  signal selected()
  readonly property bool favorite: root.bridge.friendPreferences.isFavorite(root.friend)

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
    anchors.left: favoriteButton.right
    anchors.leftMargin: root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    presence: root.friend.online ? String(root.friend.presence) : "closed"
    theme: root.theme
  }

  Text {
    id: friendName
    anchors.left: dot.right
    anchors.leftMargin: root.theme.spacing.md
    y: root.theme.space(5)
    text: String(root.friend.display_name || "")
    color: root.friend.online ? root.theme.foreground : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
    width: Math.max(0, messageButton.x - x - root.theme.spacing.md)
    elide: Text.ElideRight
  }

  Text {
    id: statusText
    anchors.left: friendName.left
    anchors.top: friendName.bottom
    width: friendName.width
    elide: Text.ElideRight
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

  Button {
    id: favoriteButton
    objectName: "favorite-" + String(root.friend.id || root.friend.display_name)
    anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
    width: root.theme.space(28); height: root.theme.space(34)
    Accessible.name: (root.favorite ? "Unfavorite " : "Favorite ") + root.friend.display_name
    ToolTip.visible: hovered
    ToolTip.text: Accessible.name
    onClicked: root.bridge.friendPreferences.toggleFavorite(root.friend)
    background: Rectangle {
      radius: root.theme.cornerRadius
      color: favoriteButton.hovered ? root.theme.alpha(root.theme.accent, 0.12) : "transparent"
      border.width: favoriteButton.visualFocus ? 1 : 0; border.color: root.theme.focusBorder
    }
    contentItem: Text {
      text: root.favorite ? "★" : "☆"
      color: root.favorite ? root.theme.warning : root.theme.muted
      font.pixelSize: root.theme.space(17)
      horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
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
