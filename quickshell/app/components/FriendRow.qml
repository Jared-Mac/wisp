import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var friend
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  signal selected()
  readonly property bool favorite: root.bridge.friendPreferences.isFavorite(root.friend)

  readonly property bool canRequest: root.friend.online &&
    (root.friend.presence === "open" || root.friend.presence === "knock")

  implicitHeight: root.theme.space(root.theme.friendly ? 44 : root.theme.tui ? 28 : 32)

  activeFocusOnTab: root.narrow
  Accessible.role: Accessible.Button
  Accessible.name: root.friend.display_name + " friend actions"
  Keys.onReturnPressed: friendMenu.popup()
  Keys.onSpacePressed: friendMenu.popup()
  ToolTip.visible: rowHover.hovered && root.narrow
  ToolTip.text: root.friend.display_name + " · " + (root.friend.online ? root.friend.presence : "offline")
  Menu {
    id: friendMenu; width: root.theme.space(220)
    ThemeControlStyle { theme: root.theme; control: friendMenu; outline: true }
    MenuItem { id: dm; text: "Message " + root.friend.display_name; onTriggered: root.bridge.openDirect(root.friend.display_name); ThemeControlStyle { theme: root.theme; control: dm } }
    MenuItem { id: call; text: root.friend.presence === "knock" ? "Knock" : "Join voice"; enabled: root.canRequest; onTriggered: root.bridge.joinFriend(root.friend.display_name); ThemeControlStyle { theme: root.theme; control: call } }
    MenuItem { id: star; text: root.favorite ? "Remove favorite" : "Add favorite"; onTriggered: root.bridge.friendPreferences.toggleFavorite(root.friend); ThemeControlStyle { theme: root.theme; control: star } }
    MenuItem { id: volume; text: "Participant volume"; onTriggered: volumeMenu.open(); ThemeControlStyle { theme: root.theme; control: volume } }
  }
  // Observe the entire row, including child buttons, without intercepting clicks.
  HoverHandler { id: rowHover }
  TapHandler { acceptedButtons: Qt.RightButton; onTapped: if(root.narrow) friendMenu.popup(); else volumeMenu.open() }
  ParticipantVolumeMenu { id: volumeMenu; bridge: root.bridge; theme: root.theme; people: [root.friend] }

  Rectangle {
    anchors.fill: parent
    radius: root.theme.cornerRadius
    color: rowHover.hovered ? root.theme.alpha(root.theme.foreground, 0.07) : "transparent"
  }

  WispAvatar { id: avatar; bridge: root.bridge; userId: String(root.friend.id); serverId: String(root.friend.server_id || root.bridge.activeServer.id); theme: root.theme; name: root.friend.display_name; visible: root.theme.friendly && root.theme.showAvatars; width: Math.min(root.width,root.theme.space(root.narrow ? 20 : 28)); height: width; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter }
  PresenceDot {
    id: dot
    objectName: "friendConnectionDot"
    z: 1
    anchors.left: parent.left
    anchors.leftMargin: avatar.visible ? Math.max(0,avatar.width-root.theme.space(7)) : 0
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
    anchors.left: avatar.visible ? avatar.right : dot.right
    anchors.leftMargin: root.tiny ? 0 : root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    visible: !root.tiny || !avatar.visible
    text: root.tiny ? String(root.friend.display_name || "?").slice(0,1).toUpperCase() : String(root.friend.display_name || "")
    color: root.friend.online ? (root.theme.colorEnabled("friendNames") ? root.theme.accent : root.theme.foreground) : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
    width: Math.min(implicitWidth, Math.max(0, (root.narrow ? root.width : statusIcon.x) - x - (favoriteButton.visible ? favoriteButton.width + root.theme.spacing.md * 2 : 0)))
    elide: Text.ElideRight
  }

  PresenceIcon {
    id: statusIcon
    objectName: "friendPresence-" + String(root.friend.id || root.friend.display_name)
    z: 1
    anchors.right: messageButton.left
    anchors.rightMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    visible: !!root.friend.online && !root.narrow
    width: visible ? implicitWidth : 0
    presence: String(root.friend.presence || "away")
    theme: root.theme
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: root.canRequest ? Qt.PointingHandCursor : Qt.ArrowCursor
    onClicked: if (root.narrow) friendMenu.popup(); else if (root.canRequest) {
      root.bridge.joinFriend(root.friend.display_name)
      // Keep the panel visible for the server's knock acknowledgment or error.
    }
  }

  Button {
    id: favoriteButton; visible: !root.narrow
    objectName: "favorite-" + String(root.friend.id || root.friend.display_name)
    anchors.left: friendName.right; anchors.leftMargin: root.theme.spacing.xs; anchors.verticalCenter: parent.verticalCenter
    width: root.theme.space(26); height: root.theme.tui ? root.height : root.theme.space(32)
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
    contentItem: Item {
      WispIcon { anchors.centerIn: parent; theme: root.theme; name: "star"; ink: root.favorite ? root.theme.warning : root.theme.muted; visible: root.theme.friendly }
      Text { anchors.fill: parent; visible: !root.theme.friendly
      text: root.favorite ? "★" : "☆"
      color: root.favorite ? root.theme.warning : root.theme.muted
      font.pixelSize: root.theme.space(17)
      horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
    }
    }
  }

  ChatButton {
    id: messageButton; theme: root.theme; visible: !root.narrow
    text: root.theme.tui ? "msg" : "Message"; iconName: "chat"; iconOnly: root.theme.friendly
    width: root.theme.friendly ? root.theme.space(34) : implicitWidth
    height: root.theme.space(30)
    anchors.right: parent.right; anchors.rightMargin: root.theme.spacing.sm; anchors.verticalCenter: parent.verticalCenter
    Accessible.name: "Message " + root.friend.display_name
    ToolTip.visible: hovered; ToolTip.text: Accessible.name
    onClicked: root.bridge.openDirect(root.friend.display_name)
  }
}
