import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  required property string conversationId
  property bool spacious: false
  readonly property var attachments: bridge.attachmentsFor(conversationId)
  readonly property bool busy: !!bridge.sendingConversations[conversationId] || !!bridge.importingConversations[conversationId]
  spacing: theme.spacing.lg

  Flickable {
    visible: root.attachments.length > 0
    width: parent.width; height: root.theme.space(124)
    contentWidth: attachmentRow.width; contentHeight: height
    clip: true; flickableDirection: Flickable.HorizontalFlick
    ScrollBar.horizontal: ScrollBar {}
    Row {
      id: attachmentRow
      spacing: root.theme.spacing.lg
      Repeater {
        model: root.attachments
        Rectangle {
          required property var modelData
          width: root.theme.space(180); height: root.theme.space(116)
          color: root.theme.surface; radius: root.theme.cornerRadius
          Image {
            anchors.left: parent.left; anchors.top: parent.top; anchors.margins: root.theme.spacing.md
            width: root.theme.space(70); height: root.theme.space(50)
            visible: !!modelData.is_image; source: modelData.url || ""
            fillMode: Image.PreserveAspectFit
          }
          Text {
            Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
            anchors.left: parent.left; anchors.top: parent.top; anchors.margins: root.theme.spacing.lg
            visible: !modelData.is_image
            text: "FILE"; color: root.theme.muted; font.pixelSize: root.theme.font.caption
          }
          ChatButton {
            anchors.right: parent.right; anchors.top: parent.top
            theme: root.theme; text: "×"; implicitWidth: root.theme.space(28)
            enabled: !root.busy
            onClicked: root.bridge.removeAttachment(root.conversationId, modelData.token, false)
            ToolTip.visible: hovered; ToolTip.text: "Remove attachment"
          }
          Text {
            Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
            anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: keepBox.top
            anchors.margins: root.theme.spacing.md
            text: String(modelData.file_name || "Screenshot.png") + " · " + root.bridge.fileSize(Number(modelData.size || 0))
            elide: Text.ElideMiddle; color: root.theme.foreground; font.pixelSize: root.theme.font.caption
          }
          CheckBox {
            ThemeControlStyle { theme: root.theme; control: keepBox }
            Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
            Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
            id: keepBox
            anchors.left: parent.left; anchors.bottom: parent.bottom
            height: root.theme.space(32); visible: !modelData.is_image
            text: "Keep on server"; checked: !!modelData.keep; enabled: !root.busy
            palette.windowText: root.theme.foreground
            onToggled: root.bridge.setAttachmentKeep(root.conversationId, modelData.token, checked)
          }
        }
      }
    }
  }

  Rectangle {
    width: parent.width
    height: root.theme.space(root.spacious ? 106 : 66)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.06)
    border.width: editor.activeFocus ? 1 : 0
    border.color: root.theme.accent
    ScrollView {
      anchors.fill: parent
      anchors.margins: root.theme.spacing.lg
      TextArea {
        ThemeControlStyle { theme: root.theme; control: editor }
        id: editor
        property bool wispTextEditor: true
        text: root.bridge.draftFor(root.conversationId)
        onTextChanged: root.bridge.setDraft(root.conversationId, text)
        color: root.theme.foreground
        placeholderText: "Message"
        placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        wrapMode: TextEdit.Wrap
        textFormat: TextEdit.PlainText
        selectByMouse: true
        background: null
        Keys.onPressed: function(event) {
          if (event.matches(StandardKey.Paste)) {
            root.bridge.pasteClipboard(root.conversationId)
            event.accepted = true
          } else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
              && !(event.modifiers & Qt.ShiftModifier)) {
            root.bridge.sendComposedMessage(root.conversationId)
            event.accepted = true
          }
        }
      }
    }
  }
  Row {
    width: parent.width
    spacing: root.theme.spacing.lg
    Item { width: Math.max(0, parent.width - (pasteHint.visible ? pasteHint.implicitWidth : 0) - sendButton.width - parent.spacing * 2); height: 1 }
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      id: pasteHint
      visible: root.spacious
      anchors.verticalCenter: parent.verticalCenter
      text: "Shift+Enter for a new line"
      color: root.theme.muted; font.pixelSize: root.theme.font.caption
    }
    ChatButton {
      id: sendButton
      theme: root.theme; primary: true
      text: root.bridge.sendingConversations[root.conversationId] ? "Sending" + root.bridge.transferLabel("upload", root.attachments.length ? root.attachments[0].token : "") : root.bridge.importingConversations[root.conversationId] ? "Preparing…" : "Send"
      enabled: !root.busy
        && (root.attachments.length > 0 || root.bridge.draftFor(root.conversationId).trim().length > 0)
      onClicked: root.bridge.sendComposedMessage(root.conversationId)
    }
  }
  Connections {
    target: root.bridge
    enabled: root.visible
    function onClipboardTextReady(conversationId, value) {
      if (conversationId === root.conversationId) editor.insert(editor.cursorPosition, value)
    }
  }
}
