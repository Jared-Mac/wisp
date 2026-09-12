import QtQuick
import QtQuick.Controls
import "../components"

Item {
  id: root
  objectName: "barWorkspace"
  required property var bridge
  required property var theme
  signal cameraRequested()
  signal serverSettingsRequested()
  signal createRoomRequested()
  readonly property real gap: theme.space(14)
  readonly property bool friendsMode: bridge.serverMember === false || bridge.friendPreferences.friendsViewFor("panel")
  readonly property real roomsWidth: friendsMode ? 0 : Math.min(theme.space(224), width*0.25)
  readonly property real friendsWidth: friendsMode ? Math.min(theme.space(300),width*0.35) : Math.min(theme.space(184), width*0.20)
  readonly property real bodyHeight: Math.max(1, height)

  Item {
    id: rooms
    objectName: "barRoomsPane"
    visible: !root.friendsMode
    width: root.roomsWidth; height: root.bodyHeight
    ServerSelector {
      id: server
      width: parent.width; bridge: root.bridge; theme: root.theme
      compact: true; adaptive: true; showInvite: false
      onSettingsRequested: root.serverSettingsRequested()
    }
    Flickable {
      objectName: "barRoomsScroll"
      anchors { left: parent.left; right: parent.right; top: server.bottom; bottom: parent.bottom; topMargin: root.theme.spacing.sm }
      contentWidth: width; contentHeight: roomList.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      Column {
        id: roomList; width: parent.width; spacing: root.theme.spacing.sm
        Repeater {
          model: root.bridge.knocks
          KnockCard { required property var modelData; width: roomList.width; knock: modelData; bridge: root.bridge; theme: root.theme; adaptive: true }
        }
        RoomsHeader {
          width: parent.width; bridge: root.bridge; theme: root.theme; adaptive: true
          onCreateRequested: root.createRoomRequested()
        }
        SpotsView { width: parent.width; bridge: root.bridge; theme: root.theme; adaptive: true }
        ServerChannelsView { width: parent.width; bridge: root.bridge; theme: root.theme; adaptive: true }
      }
    }
  }

  Rectangle {
    x: rooms.width+root.gap/2; width: 1; height: root.bodyHeight
    color: root.theme.separator
  }
  Flickable {
    id: chat
    objectName: "barChatPane"
    x: rooms.width+root.gap; width: root.width-x-root.friendsWidth-root.gap
    height: root.bodyHeight
    contentWidth: width; contentHeight: messages.implicitHeight
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}
    MessagesView {
      id: messages
      width: parent.width; availableHeight: chat.height
      bridge: root.bridge; theme: root.theme
    }
  }
  Rectangle {
    x: friends.x-root.gap/2; width: 1; height: root.bodyHeight
    color: root.theme.separator
  }
  Flickable {
    id: friends
    objectName: "barFriendsPane"
    x: root.width-width; width: root.friendsWidth; height: root.bodyHeight
    contentWidth: width; contentHeight: people.implicitHeight
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}
    Column {
      id: people; width: parent.width; spacing: root.theme.space(8)
      PeopleView { width: parent.width; bridge: root.bridge; theme: root.theme; presentation: "panel" }
    }
  }
}
