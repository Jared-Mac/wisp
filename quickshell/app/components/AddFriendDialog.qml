import QtQuick
import QtQuick.Controls

Popup {
  id: root
  required property var bridge
  required property var theme
  readonly property string serverId: String(bridge.activeServer.id)
  readonly property var account: bridge.accountActions ? bridge.accountActions.state(serverId) : ({overview:{}})
  readonly property var peopleState: bridge.friendships.state(serverId)
  parent:Overlay.overlay
  width: Math.min(theme.space(360), parent ? parent.width-theme.space(24) : theme.space(360))
  height: Math.min(content.implicitHeight+padding*2, parent ? parent.height-theme.space(40) : theme.space(600))
  x:parent ? (parent.width-width)/2 : 0
  y:parent ? Math.max(theme.space(12),(parent.height-height)/2) : 0
  padding:theme.spacing.md
  modal:true;focus:true
  closePolicy:Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background:Rectangle {color:root.theme.surface;border.color:root.theme.separator;radius:root.theme.cornerRadius}
  onOpened: {bridge.friendships.ensure(serverId);if(bridge.accountActions)bridge.accountActions.act("account_overview",{},serverId);handle.forceActiveFocus()}
  contentItem:ScrollView {
    clip:true
    Column {
      id:content;width:root.availableWidth;spacing:root.theme.spacing.sm
      Text {text:"Add friend";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.bold:true}
      Text {width:parent.width;text:"Find someone by their public username. They'll receive a friend request.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      TextField {id:handle;objectName:"friendHandle";width:parent.width;maximumLength:33;placeholderText:"@username";selectByMouse:true;onAccepted:if(find.enabled)find.clicked();ThemeControlStyle {theme:root.theme;control:handle}}
      ChatButton {id:find;objectName:"findFriend";theme:root.theme;text:"Find account";iconName:"search";enabled:!!handle.text.trim() && !root.account.busy;onClicked:root.bridge.accountActions.act("lookup_person",{handle:handle.text.trim()},root.serverId)}
      Text {width:parent.width;visible:!!root.account.person;text:root.account.person ? String(root.account.person.display_name)+" · @"+String(root.account.person.handle) : "";textFormat:Text.PlainText;wrapMode:Text.Wrap;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
      FriendshipActions {width:parent.width;bridge:root.bridge;theme:root.theme;person:Object.assign({},root.account.person || {},{server_id:root.serverId});visible:!!root.account.person}
      Text {width:parent.width;wrapMode:Text.Wrap;text:root.account.error || root.account.feedback || root.peopleState.error || root.peopleState.feedback || "";visible:!!text;color:root.account.error || root.peopleState.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      Text {visible:requests.count>0;text:"Friend requests";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.bold:true}
      Repeater {
        id:requests;model:root.peopleState.people.filter(function(p){return ["incoming","outgoing"].indexOf(p.relationship)>=0})
        Column {
          required property var modelData
          width:content.width;spacing:root.theme.spacing.xs
          Text {width:parent.width;text:modelData.display_name;textFormat:Text.PlainText;elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          FriendshipActions {width:parent.width;bridge:root.bridge;theme:root.theme;person:parent.modelData}
        }
      }
      ChatButton {theme:root.theme;text:"Close";onClicked:root.close()}
    }
  }
}
