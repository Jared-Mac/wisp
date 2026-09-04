import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  // The tray host supplies the space remaining below its other sections.
  property real availableHeight: 0
  width: parent ? parent.width : 0
  spacing: theme.spacing.sm

  function conversationLabel(c) { return c.label === "Hangout" ? "Room" : c.label }
  Item {
    id: messageHeader
    width: parent.width; height: root.theme.space(30)
    Text {
      anchors.left: parent.left; anchors.right: chatOptions.left
      anchors.rightMargin: root.theme.spacing.md; anchors.verticalCenter: parent.verticalCenter
      elide: Text.ElideRight
      text: root.theme.tui ? "┌─ 03: /chat" : root.bridge.activeConversation ? "MESSAGES · " + root.conversationLabel(root.bridge.activeConversation) : "MESSAGES"
      color: root.theme.muted; font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption; font.bold: true
      font.letterSpacing: root.theme.terminal ? 1 : 0
    }
    ChatButton {
      id: chatOptions
      anchors.right: allConversations.left; anchors.verticalCenter: parent.verticalCenter
      anchors.rightMargin: root.theme.spacing.md
      visible: !!root.bridge.activeConversation
      theme: root.theme; text: "···"; implicitWidth: root.theme.space(28)
      onClicked: optionsMenu.open()
      Menu {
        ThemeControlStyle { theme: root.theme; control: optionsMenu; outline: true; menuOutline: true }
        Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
        Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
        id: optionsMenu
        palette.window: root.theme.surface; palette.text: root.theme.foreground
        MenuItem {
          id: trialControl0
          ThemeControlStyle { theme: root.theme; control: trialControl0 }
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
           text: "Clear Chat History…"; onTriggered: clearDialog.confirm(root.bridge.activeConversationId) }
        MenuItem {
          id: trialControl1
          ThemeControlStyle { theme: root.theme; control: trialControl1 }
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
           text: "Room settings…"; visible: !!(root.bridge.activeConversation && root.bridge.activeConversation.spot_id); height: visible ? implicitHeight : 0; onTriggered: roomManager.manage(root.bridge.activeConversationId) }
      }
    }
    ChatButton {
      id: allConversations
      anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      visible: !!root.bridge.activeConversation
      text: (root.theme.tui ? "chats" : "All conversations") + (root.bridge.unreadMessages > 0 ? " · " + root.bridge.unreadMessages : "")
      theme: root.theme
      onClicked: root.bridge.closeConversation()
    }
  }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    visible: !root.bridge.activeConversation && root.bridge.conversations.length === 0
    text: "Start a private message from a friend row."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  Repeater {
    model: root.bridge.activeConversation ? [] : root.bridge.conversations
    delegate: Rectangle {
      required property var modelData
      width: root.width; height: root.theme.space(root.theme.cleanTui ? 42 : 48)
      radius: root.theme.cornerRadius
      color: conversationMouse.containsMouse ? root.theme.alpha(root.theme.foreground, root.theme.cleanTui ? 0.07 : 0.09) : root.theme.cleanTui ? "transparent" : root.theme.alpha(root.theme.foreground, 0.045)
      Column {
        anchors.left: parent.left; anchors.right: unreadLabel.left
        anchors.leftMargin: root.theme.spacing.lg; anchors.rightMargin: root.theme.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          text: root.conversationLabel(modelData)
          color: root.theme.foreground; font.pixelSize: root.theme.font.body; font.bold: true
        }
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          width: parent.width; elide: Text.ElideRight
          text: modelData.last_message
            ? modelData.last_message.sender.display_name + ": " + (modelData.last_message.content_type === "image/png" ? "Image" : modelData.last_message.content_type === "application/octet-stream" ? String(modelData.last_message.payload.file_name || "File") : String(modelData.last_message.payload || ""))
            : modelData.spot_id ? "persistent spot" : modelData.kind === "hangout" ? "room" : modelData.kind
          color: root.theme.muted; font.pixelSize: root.theme.font.caption
        }
      }
      Text {
        Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
        id: unreadLabel
        anchors.right: parent.right; anchors.rightMargin: root.theme.spacing.lg; anchors.verticalCenter: parent.verticalCenter
        visible: Number(modelData.unread_count || 0) > 0
        text: String(modelData.unread_count)
        color: root.theme.accent; font.pixelSize: root.theme.font.caption; font.bold: true
      }
      MouseArea {
        id: conversationMouse
        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.selectConversation(modelData.id)
      }
    }
  }
  Item {
    id: activeChat
    visible: !!root.bridge.activeConversation
    width: parent.width
    height: chatColumn.implicitHeight
    Column {
      id: chatColumn
      width: parent.width
      spacing: root.theme.spacing.sm
      MessageFeed {
        width: parent.width
        height: root.availableHeight > 0
          ? Math.max(root.theme.space(140), root.availableHeight - messageHeader.height - composer.implicitHeight - root.spacing * 2)
          : root.theme.space(140)
        bridge: root.bridge; theme: root.theme; conversationId: root.bridge.activeConversationId
      }
      ChatComposer {
        id: composer
        width: parent.width
        bridge: root.bridge; theme: root.theme; conversationId: root.bridge.activeConversationId
      }
    }
    ChatDropArea {
      anchors.fill: parent
      z: 5
      bridge: root.bridge; theme: root.theme; conversationId: root.bridge.activeConversationId
    }
  }
  ClearHistoryDialog { id: clearDialog; bridge: root.bridge; theme: root.theme }
  RoomManager { id: roomManager; bridge: root.bridge; theme: root.theme }
}
