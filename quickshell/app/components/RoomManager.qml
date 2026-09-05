import QtQuick
import QtQuick.Controls

Dialog {
  ThemeControlStyle { theme: root.theme; control: root; outline: true }
  Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
  Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
  id: root
  required property var bridge
  required property var theme
  property bool creating: false
  property string conversationId: ""
  property bool busy: false
  property string error: ""
  property string feedback: ""
  readonly property var conversation: bridge.conversationById(conversationId)
  readonly property var conversationServerState: bridge.serverStates.filter(function(state) {
    return conversation && String(state.server.id)===String(conversation.server_id)
  })[0] || ({friends:[]})
  readonly property bool admin: !!(conversationServerState.self || {}).server_owner || !!(conversationServerState.self || {}).server_admin
  readonly property var invitees: {
    var members = conversation ? conversation.members || [] : []
    return (conversationServerState.friends || []).filter(function(friend) { return !members.some(function(member) { return member.id === friend.id }) })
  }
  function createRoom() { creating = true; conversationId = ""; nameField.text = ""; error = ""; busy = false; open() }
  function manage(id) { creating = false; conversationId = String(id); error = ""; busy = false; open() }
  function perform(action, args) { error = ""; feedback = ""; busy = bridge.roomAction(action, args) }
  parent: Overlay.overlay
  x: parent ? (parent.width - width) / 2 : 0; y: parent ? (parent.height - height) / 2 : 0
  width: parent ? Math.min(parent.width - 32, 500) : 500
  height: creating ? 260 : 470
  modal: true
  closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape
  title: creating ? "Create a room" : "Room settings · " + (conversation ? conversation.label : "")
  palette.window: theme.surface; palette.windowText: theme.foreground
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.color: root.theme.alpha(root.theme.foreground, 0.12) }
  contentItem: Column {
    spacing: root.theme.spacing.lg
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
      width: parent.width; wrapMode: Text.Wrap
      text: root.creating ? "This room uses the server’s administrators. Creating it does not join voice."
        : "Server administrators manage every room and channel. Change admin access in Server settings."
      color: root.theme.muted
    }
    TextField {
      ThemeControlStyle { theme: root.theme; control: nameField }
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
      id: nameField
      width: parent.width; visible: root.creating; enabled: !root.busy
      maximumLength: 60; placeholderText: "Room name"; placeholderTextColor: root.theme.muted; color: root.theme.foreground
      background: Rectangle {
        color: root.theme.background; radius: root.theme.cornerRadius
        border.width: root.theme.terminal ? 1 : 0
        border.color: nameField.activeFocus ? root.theme.focusBorder : root.theme.separator
      }
    }
    ScrollView {
      objectName: "roomSettingsMembers"
      width: parent.width; height: root.theme.space(260); visible: !root.creating
      contentWidth: availableWidth
      Column {
        width: parent.width; spacing: root.theme.spacing.lg
        Repeater {
          model: root.conversation ? root.conversation.members || [] : []
          Row {
            required property var modelData
            width: parent.width; spacing: root.theme.spacing.md
            Text {
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
              Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
              width: parent.width; anchors.verticalCenter: parent.verticalCenter
              text: modelData.display_name
              elide: Text.ElideRight; color: root.theme.foreground
            }

          }
        }
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
           visible: root.admin && root.invitees.length > 0; text: "Invite to room · chat and voice access"; color: root.theme.muted }
        Repeater {
          model: root.admin ? root.invitees : []
          Row {
            required property var modelData
            width: parent.width; spacing: root.theme.spacing.md
            Text {
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
              Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
               width: parent.width - inviteButton.width - parent.spacing; anchors.verticalCenter: parent.verticalCenter; text: modelData.display_name; color: root.theme.foreground }
            ChatButton {
              id: inviteButton
              theme: root.theme; text: "Invite to room"; enabled: !root.busy
              onClicked: root.perform("invite_to_room", {conversation_id:root.conversationId,user_id:modelData.id})
            }
          }
        }
      }
    }
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
       width: parent.width; visible: root.error !== ""; text: root.error; color: root.theme.danger; wrapMode: Text.Wrap }
    Text { width: parent.width; visible: root.feedback !== ""; text: root.feedback; wrapMode: Text.Wrap; color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
    Row {
      spacing: root.theme.spacing.lg
      ChatButton { theme: root.theme; text: root.creating ? "Cancel" : "Done"; enabled: !root.busy; onClicked: root.close() }
      ChatButton { theme: root.theme; primary: true; visible: root.creating; text: root.busy ? "Creating…" : "Create room"; enabled: !root.busy && nameField.text.trim().length > 0; onClicked: root.perform("create_room", {name:nameField.text.trim()}) }
    }
  }
  Connections {
    target: root.bridge
    function onRoomActionFinished(action, success, error) {
      if (!root.opened || !root.busy) return
      root.busy = false
      if (success && action === "create_room") root.manage(root.bridge.activeConversationId)
      else if (!success) root.error = error
      else if (action === "invite_to_room") root.feedback = root.bridge.roomActionFeedback
    }
    function onDaemonConnectedChanged() { if (!root.bridge.daemonConnected) root.busy = false }
  }
}
