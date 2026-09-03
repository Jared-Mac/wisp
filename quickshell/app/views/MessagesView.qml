import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: theme.spacing.sm

  function conversationLabel(c) { return c.label === "Hangout" ? "Room" : c.label }
  Item {
    width: parent.width; height: root.theme.space(30)
    Text {
      anchors.left: parent.left; anchors.right: allConversations.left
      anchors.rightMargin: root.theme.spacing.md; anchors.verticalCenter: parent.verticalCenter
      elide: Text.ElideRight
      text: root.bridge.activeConversation ? "MESSAGES · " + root.conversationLabel(root.bridge.activeConversation) : "MESSAGES"
      color: root.theme.muted; font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption; font.bold: true
    }
    ChatButton {
      id: allConversations
      anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      visible: !!root.bridge.activeConversation
      text: "All conversations" + (root.bridge.unreadMessages > 0 ? " · " + root.bridge.unreadMessages : "")
      theme: root.theme
      onClicked: root.bridge.closeConversation()
    }
  }
  Text {
    visible: !root.bridge.activeConversation && root.bridge.conversations.length === 0
    text: "Start a private message from a friend row."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  Repeater {
    model: root.bridge.activeConversation ? [] : root.bridge.conversations
    delegate: Rectangle {
      required property var modelData
      width: root.width; height: root.theme.space(48)
      radius: root.theme.cornerRadius
      color: conversationMouse.containsMouse ? root.theme.alpha(root.theme.foreground, 0.09) : root.theme.alpha(root.theme.foreground, 0.045)
      Column {
        anchors.left: parent.left; anchors.right: unreadLabel.left
        anchors.leftMargin: root.theme.spacing.lg; anchors.rightMargin: root.theme.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        Text {
          text: root.conversationLabel(modelData)
          color: root.theme.foreground; font.pixelSize: root.theme.font.body; font.bold: true
        }
        Text {
          width: parent.width; elide: Text.ElideRight
          text: modelData.last_message
            ? modelData.last_message.sender.display_name + ": " + (modelData.last_message.content_type === "image/png" ? "Image" : modelData.last_message.content_type === "application/octet-stream" ? String(modelData.last_message.payload.file_name || "File") : String(modelData.last_message.payload || ""))
            : modelData.spot_id ? "persistent spot" : modelData.kind === "hangout" ? "room" : modelData.kind
          color: root.theme.muted; font.pixelSize: root.theme.font.caption
        }
      }
      Text {
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
        width: parent.width; height: root.theme.space(220)
        bridge: root.bridge; theme: root.theme; conversationId: root.bridge.activeConversationId
      }
      ChatComposer {
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
}
