import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  objectName:"serverPeopleDialog"
  required property var bridge
  required property var theme
  property string serverId:""
  readonly property var state: bridge.friendships.state(serverId)
  readonly property var people: state.people.filter(function(p) {
    return p.server_member === true
      && (!search.text.trim() || String(p.display_name).toLocaleLowerCase().indexOf(search.text.trim().toLocaleLowerCase())>=0)
  })
  function showServer(id) {serverId=String(id);search.text="";bridge.friendships.refresh(serverId);open()}
  parent:Overlay.overlay
  width:Math.min(theme.space(460),parent ? parent.width-24 : 460)
  height:Math.min(theme.space(560),parent ? parent.height-24 : 560)
  x:parent ? (parent.width-width)/2 : 0;y:parent ? (parent.height-height)/2 : 0
  modal:true;onOpened:search.forceActiveFocus()
  background:Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
  header:Label {text:"People in this server";padding:root.theme.spacing.lg;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title}
  contentItem:Column {
    spacing:root.theme.spacing.sm
    WispComboBox {
      id:serverPicker;objectName:"memberServerPicker";theme:root.theme;width:parent.width;model:root.bridge.servers;textRole:"name"
      currentIndex:{for(var i=0;i<model.length;i++)if(String(model[i].id)===root.serverId)return i;return -1}
      onActivated:{root.serverId=String(model[currentIndex].id);root.bridge.friendships.refresh(root.serverId)}
    }
    TextField {id:search;objectName:"memberSearch";width:parent.width;placeholderText:"Find a person";ThemeControlStyle {theme:root.theme;control:search}}
    ServerMemberList {
      id:members;objectName:"serverPeopleList";width:parent.width
      height:Math.max(40,parent.height-serverPicker.height-search.height-status.height-retry.height-parent.spacing*4)
      people:root.people;bridge:root.bridge;theme:root.theme
      Text {anchors.centerIn:parent;width:parent.width;horizontalAlignment:Text.AlignHCenter;wrapMode:Text.Wrap;visible:members.count===0
        text:root.state.loading || root.state.waiting ? "Loading people…" : root.state.error ? "" : !root.bridge.friendships.connected(root.serverId) ? "Server offline" : "No matching people"
        color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    }
    Text {id:status;width:parent.width;textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.state.error || root.state.feedback || "";color:root.state.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    ChatButton {id:retry;objectName:"refreshMembers";theme:root.theme;text:"Refresh";iconName:"refresh";enabled:!root.state.loading && !root.state.action;onClicked:root.bridge.friendships.refresh(root.serverId)}
  }
  footer:Item {
    implicitHeight:footerContent.implicitHeight+root.theme.spacing.md*2
    Column {
    id:footerContent;x:root.theme.spacing.md;y:root.theme.spacing.md
    width:parent.width-root.theme.spacing.md*2;spacing:root.theme.spacing.sm
    ChatButton {theme:root.theme;text:"Close";onClicked:root.close()}
  }
  }
}
