import QtQuick
import QtQuick.Controls
import "../components"
import "../FriendLogic.js" as FriendLogic

Column {
  id: root
  required property var bridge
  required property var theme
  property string presentation: "app"
  property bool adaptive: false
  property bool serverOnly: false
  property bool showCalls: true
  property var people: bridge.friends
  readonly property var visibleFriends: serverOnly ? people.filter(function(friend) {
    return root.bridge.friendships.state(friend.server_id).people.some(function(person) {
      return String(person.id)===String(friend.id) && person.server_member===true
    })
  }) : people
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(80)
  signal selected()
  property bool collapsible: false
  property bool showHeader: true
  readonly property bool collapsed: collapsible && bridge.friendPreferences.collapsedFor(presentation)
  width: parent ? parent.width : 0
  spacing: root.theme.space(1)

  AddFriendDialog {id:addFriendDialog;bridge:root.bridge;theme:root.theme}
  Row {
    width:parent.width;spacing:root.theme.spacing.xs
  Button {
    id: collapseButton
    visible: root.showHeader && !root.tiny
    objectName: "friends-collapse"
    width: visible ? Math.max(0,parent.width-(addFriendButton.visible ? addFriendButton.width+parent.spacing : 0)) : 0
    height: root.collapsible ? root.theme.space(root.theme.tui ? 26 : 30) : root.theme.space(20)
    enabled: root.collapsible
    Accessible.name: root.collapsed ? "Expand friends" : "Collapse friends"
    onClicked: root.bridge.friendPreferences.toggleCollapsed(root.presentation)
    background: Rectangle {
      color: collapseButton.hovered && root.collapsible ? root.theme.alpha(root.theme.foreground, 0.06) : "transparent"
      radius: root.theme.cornerRadius
      border.width: collapseButton.visualFocus ? 1 : 0
      border.color: root.theme.focusBorder
    }
    contentItem: Item {
      Text {
        anchors.left: parent.left; anchors.right: parent.right; anchors.rightMargin: root.collapsible ? 16 : 0; anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight
        text: (root.serverOnly ? "Friends in server" : "Friends") + " · " + root.visibleFriends.length
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
      id:addFriendButton;objectName:"openAddFriend";theme:root.theme
      visible: !root.serverOnly
      readonly property int pending:root.bridge.friendships.state(root.bridge.activeServer.id).people.filter(function(p){return p.relationship==="incoming"}).length
      text:pending ? "Friend requests · "+pending : "Add friend";iconName:"invite";iconOnly:true;forceIcon:true
      primary:pending>0;width:Math.min(root.width,root.theme.space(28));height:root.theme.space(28)
      ToolTip.visible:hovered || visualFocus;ToolTip.text:text
      onClicked:addFriendDialog.open()
    }
  }


  NowView {
    objectName: "friendCalls"
    width: parent.width; visible: root.showCalls && !root.collapsed && visibleHangouts.length > 0
    bridge: root.bridge; theme: root.theme; adaptive: root.adaptive
    onJoined: root.selected()
  }
  Repeater {
    model: root.collapsed ? [] : FriendLogic.sorted(root.visibleFriends, root.bridge.friendPreferences.favorites)
    delegate: FriendRow {
      required property var modelData
      width: root.width
      friend: modelData
      bridge: root.bridge
      theme: root.theme; adaptive: root.adaptive
      dense: root.presentation === "panel"
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
