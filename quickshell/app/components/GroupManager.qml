import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  required property var bridge
  required property var theme
  property string conversationId: ""
  property bool busy: false
  property string error: ""
  readonly property var conversation: bridge.conversationById(conversationId)
  readonly property bool owner: conversation && conversation.self_role==="host"
  readonly property var serverState: bridge.serverStates.filter(function(state) {
    return conversation && String(state.server.id)===String(conversation.server_id)
  })[0] || ({friends:[]})
  readonly property var invitees: {
    var members=conversation ? conversation.members || [] : []
    return (serverState.friends || []).filter(function(friend) {
      return !members.some(function(member) { return String(member.id)===String(friend.id) })
    })
  }
  function manage(id) { conversationId=String(id); busy=false; error=""; open() }
  function perform(action,args) { error=""; busy=bridge.roomAction(action,args) }
  parent: Overlay.overlay
  x: parent ? (parent.width-width)/2 : 0; y: parent ? (parent.height-height)/2 : 0
  width: parent ? Math.min(parent.width-theme.space(24),theme.space(500)) : theme.space(500)
  height: parent ? Math.min(parent.height-theme.space(24),theme.space(540)) : theme.space(540)
  modal: true
  closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape | Popup.CloseOnPressOutside
  title: "Group members · " + (conversation ? conversation.label : "")
  font.family: theme.font.family; font.pixelSize: theme.font.caption
  palette.window: theme.surface; palette.windowText: theme.foreground
  background: Rectangle { color:root.theme.surface; border.width:1; border.color:root.theme.muted; radius:root.theme.cornerRadius }
  header: Label { text:root.title; padding:root.theme.spacing.lg; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.title; background:Rectangle{color:root.theme.surface} }
  contentItem: Column {
    spacing: root.theme.spacing.md
    Text {
      width: parent.width; wrapMode:Text.Wrap
      text: root.owner ? "You own this group. Membership changes rotate its encryption roster; newly added people cannot decrypt older messages." : "Only the group owner can add or remove people."
      color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
    }
    ScrollView {
      width:parent.width; height:Math.max(1,parent.height-actions.height-parent.spacing-root.theme.space(70))
      contentWidth:availableWidth
      Column {
        width:parent.width; spacing:root.theme.spacing.xs
        Text { text:"Members"; color:root.theme.accent; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption; font.bold:true }
        Repeater {
          model:root.conversation ? root.conversation.members || [] : []
          Row {
            required property var modelData
            width:parent.width; spacing:root.theme.spacing.sm
            readonly property string role:root.conversation && root.conversation.member_roles ? root.conversation.member_roles[modelData.id] || "member" : "member"
            Text { width:parent.width-remove.width-parent.spacing; anchors.verticalCenter:parent.verticalCenter; text:String(modelData.display_name)+(parent.role==="host"?" · owner":""); elide:Text.ElideRight; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
            ChatButton { id:remove; theme:root.theme; text:"Remove"; destructive:true; visible:root.owner && parent.role!=="host"; enabled:!root.busy; onClicked:root.perform("group_remove_member",{conversation_id:root.conversationId,user_id:modelData.id}) }
          }
        }
        Text { visible:root.owner && root.invitees.length>0; text:"Add friends"; color:root.theme.accent; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption; font.bold:true }
        Repeater {
          model:root.owner ? root.invitees : []
          Row {
            required property var modelData
            width:parent.width; spacing:root.theme.spacing.sm
            Text { width:parent.width-add.width-parent.spacing; anchors.verticalCenter:parent.verticalCenter; text:String(modelData.display_name); elide:Text.ElideRight; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
            ChatButton { id:add; theme:root.theme; text:"Add"; primary:true; enabled:!root.busy; onClicked:root.perform("group_add_member",{conversation_id:root.conversationId,user_id:modelData.id}) }
          }
        }
      }
    }
    Text { width:parent.width; visible:!!root.error; text:root.error; wrapMode:Text.Wrap; color:root.theme.danger; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
    Row {
      id:actions; spacing:root.theme.spacing.sm
      ChatButton { theme:root.theme; text:"Done"; enabled:!root.busy; onClicked:root.close() }
    }
  }
  Connections {
    target:root.bridge
    function onRoomActionFinished(action,success,message) {
      if(!root.opened || ["group_add_member","group_remove_member"].indexOf(action)<0)return
      root.busy=false
      if(!success)root.error=message
    }
  }
}
