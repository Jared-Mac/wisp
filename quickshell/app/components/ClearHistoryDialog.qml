import QtQuick
import QtQuick.Controls

Dialog {
  TrialControlStyle { theme: root.theme; control: root }
  Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
  Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
  id: root
  required property var bridge
  required property var theme
  property string conversationId: ""
  property bool forEveryone: false
  property bool clearing: false
  property string error: ""
  function confirm(id) {
    conversationId = String(id)
    var c = bridge.conversationById(conversationId)
    forEveryone = !!(c && c.kind !== "direct" && c.can_clear_for_everyone)
    error = ""; clearing = false; open()
  }
  parent: Overlay.overlay
  x: parent ? (parent.width - width) / 2 : 0
  y: parent ? (parent.height - height) / 2 : 0
  width: parent ? Math.min(parent.width - 32, 440) : 440
  implicitHeight: 300
  modal: true
  closePolicy: clearing ? Popup.NoAutoClose : Popup.CloseOnEscape
  title: forEveryone ? "Clear room chat for everyone?" : "Clear Chat History?"
  palette.window: theme.surface; palette.windowText: theme.foreground
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.color: root.theme.alpha(root.theme.foreground, 0.12) }
  contentItem: Column {
    spacing: root.theme.spacing.lg
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
      width: parent.width; wrapMode: Text.Wrap
      text: root.forEveryone
        ? "This will permanently clear this room's chat history and attachments for all users, including files marked Keep. This cannot be reversed."
        : "Clear your chat history? In a DM, messages and attachments are permanently removed from server storage once both participants have cleared them. This cannot be reversed."
      color: root.theme.foreground
    }
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
       width: parent.width; visible: root.error !== ""; text: root.error; color: root.theme.danger; wrapMode: Text.Wrap }
    Row {
      spacing: root.theme.spacing.lg
      ChatButton { theme: root.theme; text: "Cancel"; enabled: !root.clearing; onClicked: root.close() }
      ChatButton {
        theme: root.theme; primary: true; destructive: true
        text: root.clearing ? "Clearing…" : "Yes, clear"; enabled: !root.clearing
        onClicked: root.clearing = root.bridge.clearChatHistory(root.conversationId, root.forEveryone)
      }
    }
  }
  Connections {
    target: root.bridge
    function onHistoryClearFinished(id, success, error) {
      if (id !== root.conversationId) return
      root.clearing = false
      if (success) root.close(); else root.error = error
    }
    function onDaemonConnectedChanged() {
      if (!root.bridge.daemonConnected && root.clearing) { root.clearing = false; root.error = "Disconnected. Reconnect and try again." }
    }
  }
}
