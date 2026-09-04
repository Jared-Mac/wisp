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

  implicitHeight: root.theme.space(root.theme.performative ? 28 : 32)

  // Observe the entire row, including child buttons, without intercepting clicks.
  HoverHandler { id: rowHover }
  TapHandler { acceptedButtons: Qt.RightButton; onTapped: volumeMenu.open() }
  ParticipantVolumeMenu { id: volumeMenu; bridge: root.bridge; theme: root.theme; people: [root.friend] }

  Rectangle {
    anchors.fill: parent
    radius: root.theme.cornerRadius
    color: rowHover.hovered ? root.theme.alpha(root.theme.foreground, 0.07) : "transparent"
  }

  PresenceDot {
    id: dot
    objectName: "friendConnectionDot"
    z: 1
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    // Connectivity is distinct from access: an online Closed friend isn't offline.
    presence: root.friend.online ? "open" : "closed"
    theme: root.theme
    Accessible.role: Accessible.StaticText
    Accessible.name: root.friend.online ? "Online" : "Offline"
    HoverHandler { id: connectionHover }
    ToolTip.visible: connectionHover.hovered
    ToolTip.text: root.friend.online ? "Online" : "Offline"
  }

  Text {
    id: friendName
    objectName: "friendName"
    anchors.left: dot.right
    anchors.leftMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    text: String(root.friend.display_name || "")
    color: root.friend.online ? (root.theme.performative ? root.theme.accent : root.theme.foreground) : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
    width: Math.min(implicitWidth, Math.max(0, statusIcon.x - x - favoriteButton.width - root.theme.spacing.md * 2))
    elide: Text.ElideRight
  }

  PresenceIcon {
    id: statusIcon
    objectName: "friendPresence-" + String(root.friend.id || root.friend.display_name)
    z: 1
    anchors.right: messageButton.left
    anchors.rightMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    visible: !!root.friend.online
    width: visible ? implicitWidth : 0
    presence: String(root.friend.presence || "away")
    theme: root.theme
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
    anchors.left: friendName.right; anchors.leftMargin: root.theme.spacing.xs; anchors.verticalCenter: parent.verticalCenter
    width: root.theme.space(26); height: root.theme.performative ? root.height : root.theme.space(32)
    // Keep its footprint and tab stop so names don't shift and keyboard users
    // can still discover the action; hide both filled and empty stars at rest.
    opacity: rowHover.hovered || visualFocus ? 1 : 0
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
    color: root.theme.performative && !messageMouse.containsMouse ? "transparent" : messageMouse.containsMouse
      ? root.theme.alpha(root.theme.accent, 0.25)
      : root.theme.alpha(root.theme.foreground, 0.07)

    Text {
      id: messageLabel
      anchors.centerIn: parent
      text: root.theme.performative ? "[msg]" : "Message"
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
