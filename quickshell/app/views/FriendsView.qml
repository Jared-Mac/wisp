import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(80)
  signal selected()
  property bool collapsible: false
  property bool showHeader: true
  readonly property bool collapsed: collapsible && bridge.friendPreferences.collapsed
  width: parent ? parent.width : 0
  spacing: root.theme.space(1)

  ServerPeopleDialog { id: directory; bridge:root.bridge; theme:root.theme }
  Row {
    width:parent.width;spacing:root.theme.space(2)
  Button {
    id: collapseButton
    visible: root.showHeader && !root.tiny
    objectName: "friends-collapse"
    width: visible ? Math.max(0,parent.width-peopleButton.width-parent.spacing) : 0
    height: root.collapsible ? root.theme.space(root.theme.tui ? 26 : 30) : root.theme.space(20)
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
        anchors.left: parent.left; anchors.right: parent.right; anchors.rightMargin: root.collapsible ? 16 : 0; anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight
        text: (root.theme.tui ? "┌─ 02: /friends" : root.theme.friendly ? "Friends" : "FRIENDS") + (root.collapsible ? " · " + root.bridge.friends.length : "")
        color: root.theme.friendSectionColor
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

    ChatButton {
      id:peopleButton;objectName:"openServerPeople";theme:root.theme;text:"Members"
      readonly property int incoming:root.bridge.friendships.state(root.bridge.activeServer.id).people.filter(function(p){return p.relationship==="incoming"}).length
      iconName:"people";iconOnly:root.showHeader || root.tiny;forceIcon:iconOnly;quiet:incoming===0;primary:incoming>0
      Accessible.name:"Server members"+(incoming ? " · "+incoming+" friend requests" : " and friend requests")
      width:root.showHeader || root.tiny ? Math.min(root.theme.space(28),parent.width) : parent.width;height:root.theme.space(28)
      onClicked:directory.showServer(root.bridge.activeServer.id)
      Rectangle {visible:peopleButton.incoming>0;anchors.right:parent.right;anchors.top:parent.top;width:7;height:7;radius:4;color:root.theme.accent;border.width:1;border.color:root.theme.surface}
    }
  }

  NowView {
    objectName: "friendCalls"
    width: parent.width; visible: !root.collapsed && visibleHangouts.length > 0
    bridge: root.bridge; theme: root.theme; adaptive: root.adaptive
    onJoined: root.selected()
  }
  Repeater {
    model: root.collapsed ? [] : root.bridge.sortedFriends
    delegate: FriendRow {
      required property var modelData
      width: root.width
      friend: modelData
      bridge: root.bridge
      theme: root.theme; adaptive: root.adaptive
      onSelected: root.selected()
    }
  }
  ServerMembersView {
    width:parent.width;visible:root.bridge.friendPreferences.showMembers
    bridge:root.bridge;theme:root.theme
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    visible: root.bridge.friendPreferences.error !== ""
    text: root.bridge.friendPreferences.error
    color: root.theme.danger; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
