import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  objectName: "removeFriendDialog"
  required property var bridge
  required property var theme
  property var person: ({})
  property string actorId: ""
  property string connectionKey: ""
  property string requestId: ""
  property string error: ""
  readonly property bool busy: requestId!==""
  readonly property bool validContext: bridge.friendships.transportReady && bridge.friendships.connected(person.server_id)
    && actorId===String((bridge.participantServer(person).self || {}).id)
    && connectionKey===bridge.friendships.connectionKey
  function confirm(value) {
    person=Object.assign({},bridge.scopedParticipant(value))
    actorId=String((bridge.participantServer(person).self || {}).id)
    connectionKey=bridge.friendships.connectionKey
    requestId="";error="";open()
  }
  onValidContextChanged: if (visible && !validContext) close()
  parent: Overlay.overlay
  width: parent ? Math.min(parent.width-24,theme.space(400)) : theme.space(400)
  x: parent ? (parent.width-width)/2 : 0
  y: parent ? (parent.height-height)/2 : 0
  modal: true
  title: "Remove friend?"
  closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape
  ThemeControlStyle { theme:root.theme;control:root;outline:true }
  background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
  contentItem: Column {
    spacing:root.theme.spacing.lg
    Text {
      width:parent.width;wrapMode:Text.Wrap;textFormat:Text.PlainText
      text:"Remove "+String(root.person.display_name || "this person")+" from your friends? Existing chats and server membership are kept. You can send each other a new friend request."
      color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
    }
    Text {width:parent.width;visible:!!text;text:root.error;textFormat:Text.PlainText;wrapMode:Text.Wrap;color:root.theme.danger;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    Flow {
      width:parent.width;spacing:root.theme.spacing.sm
      ChatButton {objectName:"cancelRemoveFriend";theme:root.theme;text:"Cancel";enabled:!root.busy;onClicked:root.close()}
      ChatButton {
        objectName:"confirmRemoveFriend";theme:root.theme;text:root.busy ? "Removing…" : "Remove friend";destructive:true
        enabled:root.validContext && !root.busy && !root.bridge.friendships.state(root.person.server_id).loading && !root.bridge.friendships.state(root.person.server_id).action
        onClicked:{root.error="";root.requestId=root.bridge.friendships.act(root.person,"remove") || "";if(!root.requestId)root.error="Couldn't remove this friend. Refresh your friends and try again."}
      }
    }
  }
  Connections {
    target:root.bridge.friendships
    function onActionFinished(id,success,error) {
      if (id!==root.requestId) return
      root.requestId=""
      if(success)root.close();else root.error=error
    }
  }
}
