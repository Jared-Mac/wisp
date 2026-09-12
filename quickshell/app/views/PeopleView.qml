import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  objectName: "peopleNavigation"
  required property var bridge
  required property var theme
  property string presentation: "app"
  signal selected()
  readonly property bool friendsMode: bridge.serverMember === false || bridge.friendPreferences.friendsViewFor(presentation)
  readonly property var contacts: {
    var result=[]
    bridge.serverStates.forEach(function(state) {
      root.bridge.friendships.state(state.server.id).people.forEach(function(person) { result.push(person) })
    })
    return result
  }
  readonly property var requests: contacts.filter(function(person) {
    var relationship=root.bridge.friendships.relationship(person)
    return relationship==="incoming" || relationship==="outgoing"
  }).sort(function(a,b) {return (a.relationship==="incoming" ? 0 : 1)-(b.relationship==="incoming" ? 0 : 1) || a.display_name.localeCompare(b.display_name)})
  readonly property int incoming: requests.filter(function(person) {return person.relationship==="incoming"}).length
  readonly property var allFriends: {
    var result=[]
    bridge.serverStates.forEach(function(state) {
      (state.friends || []).forEach(function(friend) {result.push(Object.assign({},friend,{server_id:String(state.server.id),server_name:state.server.name}))})
    })
    return result
  }
  readonly property var directChats: bridge.conversations.filter(function(chat) {return chat.kind==="direct"})
  spacing: theme.spacing.sm

  Flow {
    width: parent.width; spacing: root.theme.space(4)
    InboxButton {id:inbox;width:Math.min(parent.width,root.theme.space(36));bridge:root.bridge;theme:root.theme}
    ChatButton {
      objectName: "toggleFriendsView"
      theme: root.theme
      width: parent.width < root.theme.space(90) ? parent.width : Math.max(0,parent.width-inbox.width-parent.spacing)
      height: root.theme.space(32)
      text: root.bridge.serverMember === false ? "Friends" : root.friendsMode ? "Server members" : "Friends"+(root.incoming ? " · "+root.incoming : "")
      iconName: root.friendsMode && root.bridge.serverMember !== false ? "room" : "people"
      iconOnly: width < root.theme.space(110); forceIcon: iconOnly
      primary: !root.friendsMode && root.incoming>0
      enabled: root.bridge.serverMember !== false
      Accessible.name: root.bridge.serverMember === false ? "Friends" : root.friendsMode ? "Show server members" : "Show friends"+(root.incoming ? " · "+root.incoming+" friend requests" : "")
      ToolTip.visible: hovered || visualFocus; ToolTip.text: Accessible.name
      onClicked: root.bridge.friendPreferences.setFriendsView(!root.friendsMode,root.presentation)
    }
  }
  Column {
    objectName: "serverPeopleTab"
    width: parent.width; spacing: root.theme.spacing.sm; visible: !root.friendsMode
    FriendsView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:root.presentation;adaptive:true;collapsible:true;serverOnly:true;onSelected:root.selected()}
    ServerMembersView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:root.presentation}
  }
  Column {
    id: friendsTab; objectName: "friendsTab"
    width: parent.width; spacing: root.theme.spacing.sm; visible: root.friendsMode
    Text {
      visible:root.requests.length>0;width:parent.width;elide:Text.ElideRight
      text:"Friend requests · "+root.requests.length
      color:root.theme.friendSectionColor;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;font.bold:true
    }
    Repeater {
      model: root.friendsMode ? root.requests : []
      Rectangle {
        id: request; required property var modelData
        objectName: "friendRequest-"+modelData.server_id+"-"+modelData.id
        width: friendsTab.width; implicitHeight: requestBody.implicitHeight+root.theme.space(12)
        color:root.theme.alpha(root.theme.foreground,0.04);radius:root.theme.cornerRadius
        Column {
          id:requestBody;x:root.theme.space(6);y:x;width:Math.max(1,parent.width-x*2);spacing:root.theme.spacing.xs
          Text {width:parent.width;text:request.modelData.display_name;textFormat:Text.PlainText;elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          FriendshipActions {width:parent.width;bridge:root.bridge;theme:root.theme;person:request.modelData;compact:root.width>=root.theme.space(140)}
          Text {
            readonly property var state:root.bridge.friendships.state(request.modelData.server_id)
            width:parent.width;visible:!!text;text:state.error || "";textFormat:Text.PlainText;wrapMode:Text.Wrap
            color:root.theme.danger;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
          }
        }
      }
    }
    FriendsView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:root.presentation;adaptive:true;collapsible:true;people:root.allFriends;showCalls:false;onSelected:root.selected()}
    Text {
      visible:!root.allFriends.length && !root.requests.length;width:parent.width;wrapMode:Text.Wrap
      text:"Add a friend to start a private chat."
      color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
    }
    Text {
      visible:root.directChats.length>0;width:parent.width;elide:Text.ElideRight;text:"Direct messages"
      color:root.theme.friendSectionColor;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;font.bold:true
    }
    Repeater {
      model: root.friendsMode ? root.directChats : []
      ChatButton {
        required property var modelData
        objectName:"friendsDirect-"+modelData.id
        readonly property int pending:root.bridge.pendingCount(modelData.id)
        theme:root.theme;width:friendsTab.width;height:root.theme.space(34)
        text:String(modelData.label || "Chat")+(pending>0 ? " · "+pending : "")
        iconName:"chat";iconOnly:width<root.theme.space(70);forceIcon:iconOnly
        formatLabel:false;textAlignment:Text.AlignLeft;primary:pending>0;quiet:!primary
        ToolTip.visible:hovered || visualFocus;ToolTip.text:text
        onClicked:{root.bridge.openPendingChat(modelData.id);root.selected()}
      }
    }
  }
}
