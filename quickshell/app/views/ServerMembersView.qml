import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  objectName:"serverPeopleSection"
  required property var bridge
  required property var theme
  readonly property string serverId:String(bridge.activeServer.id)
  readonly property var state:bridge.friendships.state(serverId)
  readonly property var otherMembers:state.people.filter(function(person) {
    var relationship=bridge.friendships.relationship(person)
    return relationship!=="self" && relationship!=="friend"
  })
  spacing:theme.spacing.xs
  ServerPeopleDialog {id:directory;bridge:root.bridge;theme:root.theme}
  Row {
    width:parent.width;spacing:root.theme.space(2)
    ChatButton {
      id:heading;objectName:"members-collapse";theme:root.theme
      visible:root.width>=root.theme.space(56);width:visible ? Math.max(0,parent.width-browse.width-parent.spacing) : 0;height:root.theme.space(30)
      text:(root.theme.tui ? "/members · " : "Other members · ")+root.otherMembers.length;iconName:"people";iconOnly:width<root.theme.space(110);forceIcon:iconOnly;quiet:true;textAlignment:Text.AlignLeft
      Accessible.name:root.bridge.friendPreferences.membersCollapsed ? "Expand other members" : "Collapse other members"
      onClicked:root.bridge.friendPreferences.setMemberPreference("membersCollapsed",!root.bridge.friendPreferences.membersCollapsed)
    }
    ChatButton {id:browse;objectName:"browseMembers";theme:root.theme;text:"All server members";iconName:"search";iconOnly:true;forceIcon:true;width:Math.min(root.theme.space(28),parent.width);height:root.theme.space(28);onClicked:directory.showServer(root.serverId)}
  }
  ServerMemberList {
    id:members;objectName:"sidebarServerMembers";width:parent.width
    visible:!root.bridge.friendPreferences.membersCollapsed
    height:visible ? Math.min(root.theme.space(240),contentHeight) : 0
    people:root.visible ? root.otherMembers : [];bridge:root.bridge;theme:root.theme;compact:true
  }
  Text {
    width:parent.width;visible:!root.bridge.friendPreferences.membersCollapsed && !!text
    textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.state.error || root.state.feedback || (root.state.loading && !root.state.ready ? "Loading…" : root.state.ready && !root.otherMembers.length ? "No other members" : "")
    color:root.state.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
  }
}
