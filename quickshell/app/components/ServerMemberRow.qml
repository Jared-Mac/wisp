import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var bridge
  required property var theme
  required property var person
  property bool compact: false
  readonly property bool tiny: width < theme.space(90)
  readonly property var conversation: bridge.directFor(person)
  readonly property int pending: conversation ? bridge.pendingCount(conversation.id) : 0
  readonly property string relationship: bridge.friendships.relationship(person)
  readonly property var state: bridge.friendships.state(person.server_id)
  implicitHeight: theme.space(compact ? 36 : 52)
  ParticipantMenu { id: menu; bridge:root.bridge; theme:root.theme; voiceControls:false }
  Button {
    id: member; objectName:"serverMember-"+String(root.person.id)
    anchors.left:parent.left; anchors.right:quick.left; anchors.rightMargin:root.theme.space(4); height:parent.height
    padding:root.theme.space(4)
    Accessible.name:String(root.person.display_name)+(root.relationship==="self" ? " · you" : "")
    onClicked: if(root.compact && (root.pending>0 || root.relationship==="friend")) root.bridge.openParticipantDirect(root.person); else menu.showPerson(root.person,member)
    TapHandler {acceptedButtons:Qt.RightButton;onTapped:menu.showPerson(root.person,member)}
    ToolTip.visible:hovered || visualFocus; ToolTip.text:Accessible.name
    background:Rectangle {color:member.hovered || member.visualFocus ? root.theme.alpha(root.theme.foreground,0.06) : "transparent";radius:root.theme.cornerRadius}
    contentItem:Item {
      WispAvatar {
        id:avatar;visible:root.theme.showAvatars && root.theme.friendly
        bridge:root.bridge;theme:root.theme;userId:String(root.person.id);serverId:String(root.person.server_id);name:String(root.person.display_name)
        width:Math.min(parent.width,root.theme.space(root.compact ? 26 : 32));height:width;anchors.verticalCenter:parent.verticalCenter
      }
      Column {
        anchors.left:avatar.visible ? avatar.right : parent.left;anchors.leftMargin:avatar.visible ? root.theme.space(6) : 0
        anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;visible:!root.tiny || !avatar.visible
        Text {width:parent.width;textFormat:Text.PlainText;text:String(root.person.display_name);elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        Text {
          width:parent.width;visible:!root.compact && text!=="";textFormat:Text.PlainText;elide:Text.ElideRight
          text:root.relationship==="friend" ? "Friends" : root.relationship==="incoming" ? "Friend request" : root.relationship==="outgoing" ? "Request sent" : root.relationship==="self" ? "You" : ""
          color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
        }
      }
    }
  }
  ChatButton {
    id:quick;objectName:"serverMemberAction-"+String(root.person.id);theme:root.theme
    anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter
    width:visible ? root.theme.space(root.compact ? 26 : 32) : 0;height:width
    visible:!root.tiny && root.relationship!=="self";iconOnly:root.pending===0;forceIcon:true
    primary:root.pending>0
    text:root.pending>0 ? String(root.pending) : root.relationship==="friend" ? "Message" : root.relationship==="incoming" ? "Review friend request" : root.relationship==="outgoing" ? "Request sent" : "Add friend"
    iconName:root.relationship==="friend" ? "chat" : root.relationship==="outgoing" ? "check" : "invite"
    enabled:!root.state.loading && !root.state.action && root.bridge.friendships.connected(root.person.server_id)
    onClicked: {
      if(root.pending>0) root.bridge.openPendingChat(root.conversation.id)
      else if(root.relationship==="friend") root.bridge.openParticipantDirect(root.person)
      else if(root.relationship==="none") root.bridge.friendships.act(root.person,"send")
      else menu.showPerson(root.person,quick)
    }
  }
}
