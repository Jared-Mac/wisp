import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  required property string conversationId
  property bool spacious: false
  property bool autoGrow: false
  signal editorFocused()
  property real maximumEditorHeight: theme.space(160)
  readonly property real naturalEditorHeight: Math.max(theme.space(40), editor.contentHeight + editor.topPadding + editor.bottomPadding + (theme.tui ? theme.spacing.sm : theme.spacing.lg) * 2)
  property real editorHeight: theme.space(theme.tui && !spacious ? 44 : spacious ? 106 : 66)
  readonly property var attachments: bridge.attachmentsFor(conversationId)
  readonly property var conversation: bridge.conversationById(conversationId)
  readonly property string destination: conversation && conversation.label ? (conversation.label === "Hangout" ? "Room" : String(conversation.label)) : ""
  readonly property bool busy: !!bridge.sendingConversations[conversationId] || !!bridge.importingConversations[conversationId]
  spacing: autoGrow ? theme.spacing.xs : theme.tui ? theme.spacing.sm : theme.spacing.lg

  Text {
    objectName: "terminalChatPrompt"
    visible: root.theme.tui
    width: parent.width; elide: Text.ElideRight
    text: String(root.bridge.selfState.display_name || "user").toLowerCase() + "@wisp:~/chat/" + root.destination + " $"
    color: root.theme.accent
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }

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
    objectName: "composerMessageBox"
    width: parent.width
    height: root.autoGrow ? Math.min(root.maximumEditorHeight, root.naturalEditorHeight) : root.editorHeight
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.06)
    border.width: editor.activeFocus || root.theme.tui ? 1 : 0
    border.color: editor.activeFocus ? root.theme.accent : root.theme.separator
    Text {
      visible: root.theme.tui
      x: root.theme.space(8); y: root.theme.space(11)
      text: ">"; color: root.theme.accent
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    }
    ScrollView {
      objectName: "composerTextViewport"
      anchors.fill: parent
      anchors.margins: root.theme.tui ? root.theme.spacing.sm : root.theme.spacing.lg
      anchors.leftMargin: root.theme.tui ? root.theme.space(24) : root.theme.spacing.lg
      anchors.rightMargin: sendButton.width + root.theme.spacing.lg * 2
      TextArea {
        ThemeControlStyle { theme: root.theme; control: editor }
        id: editor
        objectName: root.autoGrow ? "mainComposerEditor" : "trayComposerEditor"
        property bool wispTextEditor: true
        // Keep the submitted draft stable until its acknowledgement arrives.
        readOnly: root.busy
        onActiveFocusChanged: if (activeFocus) root.editorFocused()
        text: root.bridge.draftFor(root.conversationId)
        onTextChanged: root.bridge.setDraft(root.conversationId, text)
        color: root.theme.foreground
        placeholderText: root.destination ? "Message " + root.destination : "Message"
        placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        wrapMode: TextEdit.Wrap
        textFormat: TextEdit.PlainText
        selectByMouse: true
        background: null
        // Keep Qt's editing, selection, IME, and shortcuts; only replace the caret.
        Component {
          id: terminalCaret
          Rectangle {
            width: root.theme.space(8)
            color: root.theme.alpha(root.theme.accent, 0.65)
          }
        }
        Binding { target: editor; property: "cursorDelegate"; value: terminalCaret; when: root.theme.tui; restoreMode: Binding.RestoreBindingOrValue }
        Keys.onPressed: function(event) {
          if (event.matches(StandardKey.Paste)) {
            root.bridge.pasteClipboard(root.conversationId)
            event.accepted = true
          } else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
              && !(event.modifiers & Qt.ShiftModifier)) {
            if (editor.inputMethodComposing) return
            if (!event.isAutoRepeat && !root.busy)
              root.bridge.sendComposedMessage(root.conversationId)
            event.accepted = true
          }
        }
      }
    }
    ChatButton {
      id: sendButton
      objectName: "composerSendButton"
      anchors.right: parent.right; anchors.bottom: parent.bottom
      anchors.margins: root.theme.space(4)
      width: root.theme.space(root.theme.tui ? 60 : 32); height: root.theme.space(32)
      theme: root.theme; primary: true
      readonly property string statusText: root.bridge.sendingConversations[root.conversationId] ? "Sending" + root.bridge.transferLabel("upload", root.attachments.length ? root.attachments[0].token : "") : root.bridge.importingConversations[root.conversationId] ? "Preparing…" : "Send message"
      Accessible.name: statusText
      ToolTip.visible: hovered
      ToolTip.text: statusText
      enabled: !root.busy
        && (root.attachments.length > 0 || root.bridge.draftFor(root.conversationId).trim().length > 0)
      onClicked: root.bridge.sendComposedMessage(root.conversationId)
      contentItem: Item {
        Canvas {
          anchors.centerIn: parent; width: root.theme.space(14); height: width
          visible: !root.busy && !root.theme.tui
          opacity: sendButton.enabled ? 1 : 0.4
          property color strokeColor: root.theme.terminal ? root.theme.accent : root.theme.accentText
          onStrokeColorChanged: requestPaint()
          onPaint: {
            var ctx = getContext("2d")
            ctx.reset(); ctx.strokeStyle = strokeColor; ctx.lineWidth = 1.8
            ctx.beginPath(); ctx.moveTo(width * 0.2, height * 0.45)
            ctx.lineTo(width * 0.5, height * 0.15); ctx.lineTo(width * 0.8, height * 0.45)
            ctx.moveTo(width * 0.5, height * 0.15); ctx.lineTo(width * 0.5, height * 0.85); ctx.stroke()
          }
        }
        Text {
          anchors.centerIn: parent
          visible: root.theme.tui && !root.busy
          text: "[send]"
          color: root.theme.selectionText; opacity: sendButton.enabled ? 1 : 0.4
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        BusyIndicator { anchors.centerIn: parent; width: root.theme.space(18); height: width; running: root.busy; visible: running }
      }
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
