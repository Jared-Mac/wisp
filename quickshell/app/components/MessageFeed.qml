import QtQuick
import QtQuick.Controls

Rectangle {
  id: root
  objectName: "messageFeed"
  required property var bridge
  required property var theme
  required property string conversationId
  property string editingId: ""
  property string deletingId: ""
  property bool editingImage: false
  property bool savingEdit: false
  property string editError: ""
  readonly property bool editOpen: editDialog.opened
  function beginEdit(message) {
    editingId = String(message.id)
    editingImage = message.content_type === "image/png" || message.content_type === "application/octet-stream"
    editField.text = editingImage ? String(message.payload.caption || "") : String(message.payload || "")
    editError = ""
    editDialog.open()
    editField.forceActiveFocus()
  }
  radius: theme.cornerRadius
  color: theme.alpha(theme.foreground, 0.025)
  onConversationIdChanged: Qt.callLater(function() { messages.positionViewAtEnd() })
  ListView {
    id: messages
    anchors.fill: parent
    anchors.margins: root.theme.space(16)
    clip: true
    spacing: root.theme.space(18)
    model: root.bridge.messagesFor(root.conversationId)
    property bool followBottom: true
    onMovementEnded: followBottom = atYEnd
    onCountChanged: if (followBottom) Qt.callLater(function() { positionViewAtEnd() })
    ScrollBar.vertical: ScrollBar {}
    delegate: Column {
      id: message
      required property var modelData
      readonly property bool isImage: modelData.content_type === "image/png"
      readonly property bool isFile: modelData.content_type === "application/octet-stream"
      readonly property string imageUrl: root.bridge.chatImageUrls[String(modelData.id)] || ""
      width: messages.width
      spacing: root.theme.spacing.md
      Component.onCompleted: if (isImage) root.bridge.loadChatImage(String(modelData.id))
      Row {
        spacing: root.theme.spacing.lg
        Text {
          text: String(message.modelData.sender.display_name || "")
          color: message.modelData.sender.id === root.bridge.selfState.id ? root.theme.accent : root.theme.foreground
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.bold: true
        }
        Text {
          text: Qt.formatDateTime(new Date(message.modelData.created_at), "MMM d · h:mm AP")
          color: root.theme.muted; font.pixelSize: root.theme.font.caption
        }
        Text {
          visible: !!message.modelData.edited_at
          text: "edited"
          color: root.theme.alpha(root.theme.muted, 0.8)
          font.pixelSize: root.theme.space(10)
        }
        ChatButton {
          visible: message.modelData.sender.id === root.bridge.selfState.id
          theme: root.theme; text: "···"
          implicitWidth: root.theme.space(26); implicitHeight: root.theme.space(20)
          onClicked: messageMenu.open()
          Menu {
            id: messageMenu
            palette.window: root.theme.surface
            palette.text: root.theme.foreground
            MenuItem { text: message.isImage || message.isFile ? "Edit caption…" : "Edit message…"; onTriggered: root.beginEdit(message.modelData) }
            MenuItem { text: "Delete message…"; onTriggered: { root.deletingId = String(message.modelData.id); deleteDialog.open() } }
          }
        }
      }
      Rectangle {
        visible: message.isImage
        width: Math.min(parent.width, root.theme.space(640))
        height: message.isImage ? Math.min(root.theme.space(420), width * Number(message.modelData.payload.height || 180) / Math.max(1, Number(message.modelData.payload.width || 320))) : 0
        radius: root.theme.cornerRadius
        color: root.theme.surface
        Image {
          id: photo
          anchors.fill: parent
          source: message.imageUrl
          asynchronous: true
          fillMode: Image.PreserveAspectFit
        }
        Text {
          anchors.centerIn: parent
          visible: photo.status !== Image.Ready
          text: root.bridge.imageErrors[String(message.modelData.id)] ? "Image unavailable · click to retry" : "Loading image…"
          color: root.theme.muted
        }
        MouseArea {
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            if (photo.status === Image.Ready) Qt.openUrlExternally(message.imageUrl)
            else root.bridge.loadChatImage(String(message.modelData.id), true)
          }
        }
      }
      Rectangle {
        visible: message.isFile
        width: Math.min(parent.width, root.theme.space(480)); height: root.theme.space(136)
        color: root.theme.surface; radius: root.theme.cornerRadius
        Column {
          anchors.fill: parent; anchors.margins: root.theme.spacing.lg
          spacing: root.theme.spacing.md
          Text {
            width: parent.width; elide: Text.ElideMiddle
            text: String(message.modelData.payload.file_name || "File")
            color: root.theme.foreground; font.pixelSize: root.theme.font.body
          }
          Row {
            spacing: root.theme.spacing.lg
            Text {
              anchors.verticalCenter: parent.verticalCenter
              text: root.bridge.fileSize(Number(message.modelData.payload.size || 0))
              color: root.theme.muted; font.pixelSize: root.theme.font.caption
            }
            ChatButton {
              theme: root.theme
              enabled: !root.bridge.savingFiles[String(message.modelData.id)] && (!message.modelData.payload.expired || !!root.bridge.savedFiles[String(message.modelData.id)])
              text: root.bridge.savingFiles[String(message.modelData.id)] ? "Saving" + root.bridge.transferLabel("download", String(message.modelData.id)) : root.bridge.savedFiles[String(message.modelData.id)] ? "Saved · Show folder" : message.modelData.payload.expired ? "File expired" : "Save file"
              onClicked: {
                var saved = root.bridge.savedFiles[String(message.modelData.id)]
                if (saved) Qt.openUrlExternally(saved.directory_url)
                else root.bridge.saveChatFile(String(message.modelData.id))
              }
            }
          }
          Row {
            spacing: root.theme.spacing.lg
            ChatButton {
              visible: !message.modelData.payload.expired
              theme: root.theme
              text: message.modelData.payload.keep ? "Kept · Allow expiry" : "Keep file"
              onClicked: root.bridge.setFileRetention(message.modelData.id, !message.modelData.payload.keep)
            }
            Text {
              anchors.verticalCenter: parent.verticalCenter
              text: message.modelData.payload.expired ? "Removed from server" : message.modelData.payload.keep ? "Kept on server" : message.modelData.payload.expires_at ? "Expires " + Qt.formatDateTime(new Date(message.modelData.payload.expires_at), "MMM d, h:mm AP") : "No automatic expiry"
              color: root.theme.muted; font.pixelSize: root.theme.space(10)
            }
          }
        }
      }
      TextEdit {
        width: parent.width
        text: message.isImage || message.isFile ? String(message.modelData.payload.caption || "") : String(message.modelData.payload || "")
        visible: text !== ""
        color: root.theme.foreground
        readOnly: true; selectByMouse: true
        textFormat: TextEdit.PlainText
        wrapMode: TextEdit.Wrap
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      }
    }
  }
  Text {
    anchors.centerIn: parent
    visible: messages.count === 0
    text: "This is the start of your conversation."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  Dialog {
    id: editDialog
    parent: Overlay.overlay
    x: parent ? (parent.width - width) / 2 : 0; y: parent ? (parent.height - height) / 2 : 0
    width: Math.min(root.width, root.theme.space(440)); implicitHeight: root.theme.space(300)
    modal: true
    closePolicy: root.savingEdit ? Popup.NoAutoClose : Popup.CloseOnEscape
    title: root.editingImage ? "Edit caption" : "Edit message"
    palette.window: root.theme.surface; palette.windowText: root.theme.foreground
    contentItem: Column {
      spacing: root.theme.spacing.lg
      ScrollView {
        width: parent.width; height: root.theme.space(150)
        TextArea {
          id: editField
          enabled: !root.savingEdit
          color: root.theme.foreground
          wrapMode: TextEdit.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true
          background: Rectangle { color: root.theme.background; radius: root.theme.cornerRadius }
        }
      }
      Text { width: parent.width; text: root.editError; visible: text !== ""; color: root.theme.danger; wrapMode: Text.Wrap; font.pixelSize: root.theme.font.caption }
      Row {
        spacing: root.theme.spacing.lg
        ChatButton { theme: root.theme; text: "Cancel"; enabled: !root.savingEdit; onClicked: editDialog.close() }
        ChatButton {
          theme: root.theme; primary: true; text: root.savingEdit ? "Saving…" : "Save changes"
          enabled: !root.savingEdit && (root.editingImage || editField.text.trim().length > 0)
          onClicked: root.savingEdit = root.bridge.editChatMessage(root.editingId, editField.text)
        }
      }
    }
  }
  Dialog {
    id: deleteDialog
    parent: Overlay.overlay
    x: parent ? (parent.width - width) / 2 : 0; y: parent ? (parent.height - height) / 2 : 0
    width: Math.min(root.width, root.theme.space(400)); implicitHeight: root.theme.space(210)
    modal: true
    title: "Delete message?"
    standardButtons: Dialog.Cancel | Dialog.Yes
    palette.window: root.theme.surface; palette.windowText: root.theme.foreground
    contentItem: Text {
      text: "Delete this message for everyone in the conversation? This cannot be undone."
      color: root.theme.foreground; wrapMode: Text.Wrap
    }
    onAccepted: root.bridge.deleteChatMessage(root.deletingId)
  }
  Connections {
    target: root.bridge
    function onMessageMutationFinished(messageId, action, success, error) {
      if (action !== "edit" || messageId !== root.editingId) return
      root.savingEdit = false
      if (success) editDialog.close()
      else root.editError = error
    }
    function onDaemonConnectedChanged() {
      if (!root.bridge.daemonConnected && root.savingEdit) {
        root.savingEdit = false
        root.editError = "Disconnected. Your edit is kept here; try again when connected."
      }
    }
  }
}
