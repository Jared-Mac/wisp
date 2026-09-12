import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  objectName:"serverPeopleSection"
  required property var bridge
  required property var theme
  visible: bridge.serverMember !== false
  property string presentation: "app"
  property bool collapsible: true
  readonly property bool collapsed: collapsible && bridge.friendPreferences.membersCollapsedFor(presentation)
  readonly property string serverId:String(bridge.activeServer.id)
  readonly property var state:bridge.friendships.state(serverId)
  function ensureMembers() { if (visible) bridge.friendships.ensure(serverId) }
  Component.onCompleted: Qt.callLater(ensureMembers)
  onServerIdChanged: Qt.callLater(ensureMembers)
  onVisibleChanged: if (visible) Qt.callLater(ensureMembers)
  readonly property var otherMembers:state.people.filter(function(person) {
    var relationship=bridge.friendships.relationship(person)
    return person.server_member === true && relationship!=="self" && relationship!=="friend"
  })
  spacing:theme.spacing.xs
  ServerPeopleDialog {id:directory;bridge:root.bridge;theme:root.theme}
  Row {
    width:parent.width;spacing:root.theme.space(2)
    Button {
      id:heading;objectName:"members-collapse"
      enabled:root.collapsible
      visible:root.width>=root.theme.space(56);width:visible ? Math.max(0,parent.width-browse.width-parent.spacing) : 0;height:root.theme.space(30)
      background:Rectangle {color:heading.hovered ? root.theme.alpha(root.theme.foreground,0.06) : "transparent";radius:root.theme.cornerRadius;border.width:heading.visualFocus ? 1 : 0;border.color:root.theme.focusBorder}
      contentItem:Item {
        Text {anchors.left:parent.left;anchors.right:arrow.left;anchors.verticalCenter:parent.verticalCenter;elide:Text.ElideRight;text:(root.theme.tui ? "/members · " : "Other members · ")+root.otherMembers.length;color:root.theme.friendSectionColor;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;font.bold:true}
        Text {id:arrow;visible:root.collapsible;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;text:root.collapsed ? "▸" : "▾";color:root.theme.muted;font.pixelSize:root.theme.font.body}
      }
      Accessible.name:root.collapsed ? "Expand other members" : "Collapse other members"
      onClicked:root.bridge.friendPreferences.setMemberPreference("membersCollapsed",!root.collapsed,root.presentation)
    }
    ChatButton {id:browse;objectName:"browseMembers";theme:root.theme;text:"All server members";iconName:"search";iconOnly:true;forceIcon:true;width:Math.min(root.theme.space(28),parent.width);height:root.theme.space(28);onClicked:directory.showServer(root.serverId)}
  }
  ServerMemberList {
    id:members;objectName:"sidebarServerMembers";width:parent.width
    visible:!root.collapsed
    height:visible ? contentHeight : 0
    interactive:false
    people:root.visible ? root.otherMembers : [];bridge:root.bridge;theme:root.theme;compact:true
  }
  Text {
    width:parent.width;visible:!root.collapsed && !!text
    textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.state.error || root.state.feedback || ((root.state.loading || root.state.waiting) && !root.state.ready ? "Loading…" : root.state.ready && !root.otherMembers.length ? "No other members" : "")
    color:root.state.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
  }
}
