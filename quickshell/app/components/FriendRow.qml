import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var friend
  required property var bridge
  required property var theme
  property bool adaptive: false
  property bool dense: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(80)
  readonly property bool compactActions: adaptive && width < theme.space(220)
  signal selected()
  readonly property bool favorite: root.bridge.friendPreferences.isFavorite(root.friend)

  readonly property bool canRequest: root.bridge.participantServer(root.friend).server_member !== false && root.friend.online &&
    (root.friend.presence === "open" || root.friend.presence === "knock")

  implicitHeight: root.theme.space(root.theme.friendly ? (dense ? 36 : 44) : root.theme.comfortable ? (dense ? 32 : 38) : root.theme.tui ? 28 : 32)

  activeFocusOnTab: true
  Accessible.role: Accessible.Button
  Accessible.name: "Message " + root.friend.display_name
  Keys.onReturnPressed: root.bridge.openParticipantDirect(root.friend)
  Keys.onSpacePressed: root.bridge.openParticipantDirect(root.friend)
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Menu || event.key === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier) {
      friendMenu.popup(); event.accepted = true
    }
  }
  ToolTip.visible: rowHover.hovered && root.narrow
  ToolTip.text: "Message " + root.friend.display_name
  Menu {
    id: friendMenu; objectName: "friendActionsMenu"; width: root.theme.space(220)
    ThemeControlStyle { theme: root.theme; control: friendMenu; outline: true }
    MenuItem { id: dm; text: "Message " + root.friend.display_name; onTriggered: root.bridge.openParticipantDirect(root.friend); ThemeControlStyle { theme: root.theme; control: dm } }
    MenuItem { id: call; text: root.friend.presence === "knock" ? "Knock" : "Join voice"; enabled: root.canRequest; onTriggered: root.bridge.requestFriendVoice(root.friend.server_id,root.friend.id,root.friend.display_name); ThemeControlStyle { theme: root.theme; control: call } }
    MenuItem { id: star; text: root.favorite ? "Remove favorite" : "Add favorite"; onTriggered: root.bridge.friendPreferences.toggleFavorite(root.friend); ThemeControlStyle { theme: root.theme; control: star } }
    MenuItem { id:removeFriend;objectName:"removeFriendAction";text:"Remove friend…";onTriggered:removeDialog.confirm(root.friend);ThemeControlStyle {theme:root.theme;control:removeFriend} }
    MenuItem { id:blockAccount;text:"Block account";visible:!!root.bridge.accountActions;onTriggered:root.bridge.accountActions.act("block_person",{user_id:root.friend.id},root.friend.server_id);ThemeControlStyle {theme:root.theme;control:blockAccount} }
    MenuItem { id: volume; text: "Participant volume"; onTriggered: volumeMenu.open(); ThemeControlStyle { theme: root.theme; control: volume } }
  }
  // Observe the entire row, including child buttons, without intercepting clicks.
  HoverHandler { id: rowHover }
  TapHandler { acceptedButtons: Qt.RightButton; onTapped: friendMenu.popup() }
  RemoveFriendDialog { id:removeDialog;bridge:root.bridge;theme:root.theme }
  ParticipantVolumeMenu { id: volumeMenu; bridge: root.bridge; theme: root.theme; people: [root.friend] }

  Rectangle {
    anchors.fill: parent
    radius: root.theme.cornerRadius
    color: rowHover.hovered ? root.theme.alpha(root.theme.foreground, 0.07) : "transparent"
  }

  WispAvatar { id: avatar; objectName: "friendAvatar"; bridge: root.bridge; userId: String(root.friend.id); serverId: String(root.friend.server_id || root.bridge.activeServer.id); theme: root.theme; name: root.friend.display_name; visible: root.theme.friendly && root.theme.showAvatars; width: Math.min(root.width,root.theme.space(28)); height: width; x: root.tiny ? (root.width-width)/2 : 0; anchors.verticalCenter: parent.verticalCenter }
  Rectangle {
    anchors.fill: dot; anchors.margins: -2; z: 1
    visible: avatar.visible; color: root.theme.sidebar; radius: width/2
  }
  PresenceDot {
    id: dot
    objectName: "friendConnectionDot"
    z: 1
    x: avatar.visible ? avatar.x + avatar.width - width : root.tiny ? Math.max(0,(root.width-width-friendName.implicitWidth-root.theme.space(4))/2) : 0
    y: avatar.visible ? avatar.y + avatar.height - height : (root.height-height)/2
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
    anchors.leftMargin: root.tiny ? (avatar.visible ? 0 : root.theme.space(4)) : root.theme.spacing.sm
    anchors.verticalCenter: parent.verticalCenter
    visible: !root.tiny || !avatar.visible
    text: root.tiny ? String(root.friend.display_name || "?").slice(0,1).toUpperCase() : String(root.friend.display_name || "")
    color: root.friend.online ? (root.theme.colorEnabled("friendNames") ? root.theme.accent : root.theme.foreground) : root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
    width: Math.min(implicitWidth, Math.max(0, (root.narrow ? root.width : statusIcon.x) - x - (favoriteButton.visible ? favoriteButton.width + root.theme.spacing.md * 2 : 0)))
    elide: Text.ElideRight
  }

  Button {
    id: statusIcon
    hoverEnabled: true
    objectName: "friendPresence-" + String(root.friend.id || root.friend.display_name)
    z: 2
    anchors.right: messageButton.left
    anchors.rightMargin: root.theme.spacing.xs
    anchors.verticalCenter: parent.verticalCenter
    visible: !!root.friend.online && !root.narrow
    width: visible ? root.theme.space(28) : 0; height: root.theme.space(30)
    enabled: root.canRequest
    Accessible.name: root.friend.presence === "knock" ? "Knock to request voice with " + root.friend.display_name : "Join voice with " + root.friend.display_name
    ToolTip.visible: hovered || visualFocus; ToolTip.text: Accessible.name
    onClicked: root.bridge.requestFriendVoice(root.friend.server_id,root.friend.id,root.friend.display_name)
    background: Rectangle { radius: root.theme.cornerRadius; color: statusIcon.hovered || statusIcon.visualFocus ? root.theme.alpha(root.theme.foreground,0.08) : "transparent" }
    contentItem: Item {
      PresenceIcon { anchors.centerIn: parent; presence: String(root.friend.presence || "away"); theme: root.theme; showTooltip: false }
    }
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: root.bridge.openParticipantDirect(root.friend)
  }

  Button {
    id: favoriteButton; visible: !root.compactActions
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
    id: messageButton; objectName: "messageFriend-" + String(root.friend.id || root.friend.display_name); theme: root.theme; visible: !root.narrow
    readonly property var conversation: root.bridge.directFor(root.friend)
    readonly property int pending: conversation ? root.bridge.pendingCount(conversation.id) : 0
    text: pending>0 ? String(pending) : root.theme.tui ? "msg" : "Message"; iconName: "chat"; iconOnly: root.theme.friendly && pending===0; primary: pending>0
    width: pending>0 ? Math.max(root.theme.space(34),implicitWidth) : root.theme.friendly ? root.theme.space(34) : implicitWidth
    height: root.theme.space(30)
    anchors.right: parent.right; anchors.rightMargin: root.theme.spacing.sm; anchors.verticalCenter: parent.verticalCenter
    Accessible.name: "Message " + root.friend.display_name
    ToolTip.visible: hovered; ToolTip.text: Accessible.name
    onClicked: pending>0 ? root.bridge.openPendingChat(conversation.id) : root.bridge.openParticipantDirect(root.friend)
  }
}
