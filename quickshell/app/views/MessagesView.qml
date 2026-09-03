import QtQuick

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  Row {
    width: parent.width
    spacing: root.theme.spacing.sm

    Text {
      text: root.bridge.activeConversation ? "MESSAGES · "
        + String(root.bridge.activeConversation.label || "Conversation") : "MESSAGES"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }

    Text {
      visible: root.bridge.unreadMessages > 0
      text: String(root.bridge.unreadMessages) + " unread"
      color: root.theme.accent
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Text {
    visible: !root.bridge.activeConversation && root.bridge.conversations.length === 0
    text: "Start a private message from a friend row."
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
  }

  Repeater {
    model: root.bridge.activeConversation ? [] : root.bridge.conversations
    delegate: Rectangle {
      required property var modelData
      width: root.width
      height: root.theme.space(48)
      radius: root.theme.cornerRadius
      color: conversationMouse.containsMouse
        ? root.theme.alpha(root.theme.foreground, 0.09)
        : root.theme.alpha(root.theme.foreground, 0.045)

      Column {
        anchors.left: parent.left
        anchors.right: unreadLabel.left
        anchors.leftMargin: root.theme.spacing.lg
        anchors.rightMargin: root.theme.spacing.md
        anchors.verticalCenter: parent.verticalCenter

        Text {
          text: String(modelData.label || "Conversation")
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body
          font.weight: Font.DemiBold
        }

        Text {
          width: parent.width
          elide: Text.ElideRight
          text: modelData.last_message
            ? String(modelData.last_message.sender.display_name) + ": "
              + String(modelData.last_message.payload || "")
            : String(modelData.kind || "").replace("_", " ")
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }

      Text {
        id: unreadLabel
        anchors.right: parent.right
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        visible: Number(modelData.unread_count || 0) > 0
        text: String(modelData.unread_count)
        color: root.theme.accent
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.Bold
      }

      MouseArea {
        id: conversationMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.selectConversation(modelData.id)
      }
    }
  }

  Column {
    visible: !!root.bridge.activeConversation
    width: parent.width
    spacing: root.theme.spacing.sm

    Rectangle {
      width: parent.width
      height: root.theme.space(220)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, 0.035)

      ListView {
        id: messageList
        anchors.fill: parent
        anchors.margins: root.theme.spacing.md
        clip: true
        spacing: root.theme.spacing.sm
        model: root.bridge.activeMessages
        onCountChanged: positionViewAtEnd()

        delegate: Column {
          required property var modelData
          width: messageList.width
          spacing: root.theme.spacing.xs

          Text {
            text: String(modelData.sender.display_name || "")
            color: modelData.sender.id === root.bridge.selfState.id
              ? root.theme.accent : root.theme.muted
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.caption
            font.weight: Font.DemiBold
          }

          Text {
            width: parent.width
            text: String(modelData.payload || "")
            color: root.theme.foreground
            wrapMode: Text.Wrap
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.body
          }
        }
      }
    }

    Row {
      width: parent.width
      spacing: root.theme.spacing.sm

      Rectangle {
        width: parent.width - sendButton.width - parent.spacing
        height: root.theme.space(38)
        radius: root.theme.cornerRadius
        color: root.theme.alpha(root.theme.foreground, 0.07)

        TextInput {
          id: composer
          anchors.fill: parent
          anchors.margins: root.theme.spacing.md
          color: root.theme.foreground
          selectionColor: root.theme.accent
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body
          clip: true
          onAccepted: sendCurrent()

          function sendCurrent() {
            if (root.bridge.sendMessage(text)) text = ""
          }
        }
      }

      Rectangle {
        id: sendButton
        width: root.theme.space(64)
        height: root.theme.space(38)
        radius: root.theme.cornerRadius
        color: sendMouse.containsMouse
          ? root.theme.alpha(root.theme.accent, 0.8) : root.theme.accent

        Text {
          anchors.centerIn: parent
          text: "Send"
          color: "white"
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.DemiBold
        }

        MouseArea {
          id: sendMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: composer.sendCurrent()
        }
      }
    }

    Text {
      text: "‹ All conversations"
      color: root.theme.accent
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.closeConversation()
      }
    }
  }
}
