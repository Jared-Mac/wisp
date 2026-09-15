import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "../MentionLogic.js" as Mentions

Column {
  id: root
  required property var bridge
  required property var theme
  required property string conversationId
  property bool spacious: false
  property bool autoGrow: false
  property bool compact: false
  property string previousTypingConversation: ""
  function stopTyping() { if (bridge.typing) bridge.typing.stop(previousTypingConversation || conversationId) }
  onConversationIdChanged: { stopTyping(); previousTypingConversation=conversationId }
  onVisibleChanged: if (!visible) stopTyping()
  onBusyChanged: if (busy) stopTyping()
  Component.onDestruction: stopTyping()
  readonly property alias emojiPicker: composerEmojiPicker
  readonly property alias mentionPicker: mentionPicker
  readonly property var mentionQuery: Mentions.query(editor.text,editor.cursorPosition)
  readonly property var mentionChoices: Mentions.suggestions(bridge.mentionPeople(conversationId),mentionQuery)
  property int mentionSelection: 0
  property string dismissedMention: ""
  readonly property string mentionSignature: conversationId+":"+editor.cursorPosition+":"+editor.text
  onMentionSignatureChanged: dismissedMention=""
  onMentionQueryChanged: mentionSelection=0
  function chooseMention(index) {
    if (!mentionQuery || !mentionChoices[index] || busy || pendingAccess) return
    var query=mentionQuery, value=Mentions.token(mentionChoices[index].display_name)+" "
    editor.remove(query.start,query.end)
    editor.insert(query.start,value)
    editor.cursorPosition=query.start+value.length
    editor.forceActiveFocus()
  }
  function handleMentionKey(event) {
    if (!mentionPicker.visible || editor.inputMethodComposing || event.modifiers!==Qt.NoModifier) return false
    if (event.key===Qt.Key_Down || event.key===Qt.Key_Up) {
      mentionSelection=(mentionSelection+(event.key===Qt.Key_Down ? 1 : mentionChoices.length-1))%mentionChoices.length
      mentionList.positionViewAtIndex(mentionSelection,ListView.Contain)
    } else if (event.key===Qt.Key_Return || event.key===Qt.Key_Enter || event.key===Qt.Key_Tab) {
      if (!event.isAutoRepeat) chooseMention(mentionSelection)
    } else if (event.key===Qt.Key_Escape) dismissedMention=mentionSignature
    else return false
    event.accepted=true; return true
  }
  signal editorFocused()
  property real maximumEditorHeight: theme.space(160)
  readonly property real editorMargin: compact ? theme.space(4) : theme.tui ? theme.spacing.sm : theme.spacing.lg
  readonly property real naturalEditorHeight: Math.max(theme.space(40), editor.contentHeight + editor.topPadding + editor.bottomPadding + editorMargin * 2)
  property real editorHeight: theme.space(theme.tui && !spacious ? 44 : spacious ? 106 : 66)
  readonly property var attachments: bridge.attachmentsFor(conversationId)
  readonly property var conversation: bridge.conversationById(conversationId)
  readonly property string destination: conversation && conversation.label ? (conversation.label === "Hangout" ? "Room" : String(conversation.label)) : ""
  readonly property bool pendingAccess: !!(conversation && conversation.pending_access)
  readonly property bool busy: !!bridge.sendingConversations[conversationId] || !!bridge.importingConversations[conversationId]
  readonly property var replyingTo: bridge.messageActions.replyFor(conversationId)
  function focusEditor() { editor.forceActiveFocus() }
  spacing: autoGrow ? theme.spacing.xs : theme.tui ? theme.spacing.sm : theme.spacing.lg


  Text {
    objectName: "chatTypingIndicator"
    width: parent.width
    height: root.theme.space(18)
    text: root.bridge.typing ? root.bridge.typing.label(root.conversationId) : ""
    textFormat: Text.PlainText
    elide: Text.ElideRight
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    Accessible.name: text
  }

  Text {
    objectName: "roomChatAccessPending"
    visible: root.pendingAccess
    width: parent.width; wrapMode: Text.WordWrap
    text: "Chat access is pending. You can join voice now."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }

  Text {
    objectName: "terminalChatPrompt"
    visible: root.theme.tui && !root.theme.comfortable
    width: parent.width; elide: Text.ElideRight
    text: root.theme.refinedTui ? "message /" + root.destination
      : String(root.bridge.selfState.display_name || "user").toLowerCase() + "@wisp:~/chat/" + root.destination + " $"
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
    objectName: "composerReplyBar"
    visible: !!root.replyingTo
    width: parent.width; height: root.theme.space(48)
    color: root.theme.alpha(root.theme.accent,0.09); radius: root.theme.cornerRadius
    Rectangle { width: root.theme.space(2); height: parent.height; color: root.theme.accent }
    Column {
      anchors.left: parent.left; anchors.right: cancelReply.left; anchors.verticalCenter: parent.verticalCenter; anchors.margins: root.theme.spacing.md
      Text { width: parent.width; textFormat: Text.PlainText; elide: Text.ElideRight; text: "Replying to " + String((root.replyingTo || {}).sender_name || ""); color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
      Text { width: parent.width; textFormat: Text.PlainText; elide: Text.ElideRight; text: String((root.replyingTo || {}).preview || "").replace(/\s+/g," "); color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
    }
    ChatButton {
      id: cancelReply; objectName: "cancelReply"; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      width: root.theme.space(32); height: width; theme: root.theme; text: "×"; enabled: !root.busy
      Accessible.name: "Cancel reply"; ToolTip.visible: hovered; ToolTip.text: Accessible.name
      onClicked: { root.bridge.messageActions.cancelReply(root.conversationId); root.focusEditor() }
    }
  }

  Rectangle {
    id: messageBox
    objectName: "composerMessageBox"
    width: parent.width
    height: root.autoGrow || root.compact ? Math.min(root.maximumEditorHeight, root.naturalEditorHeight) : root.editorHeight
    radius: root.theme.cornerRadius
    color: root.theme.comfortable ? root.theme.surface : root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.06)
    border.width: editor.activeFocus || root.theme.tui ? 1 : 0
    border.color: editor.activeFocus ? root.theme.accent : root.theme.separator
    Text {
      visible: root.theme.tui && !root.theme.comfortable
      x: root.theme.space(8); y: root.theme.space(11)
      text: ">"; color: root.theme.accent
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    }
    ScrollView {
      objectName: "composerTextViewport"
      anchors.fill: parent
      anchors.margins: root.editorMargin
      anchors.leftMargin: root.theme.friendly ? root.theme.space(42) : root.theme.tui && !root.theme.comfortable ? root.theme.space(24) : root.theme.spacing.lg
      anchors.rightMargin: sendButton.width + emojiButton.width + root.theme.spacing.lg * 3
      TextArea {
        ThemeControlStyle { theme: root.theme; control: editor }
        id: editor
        objectName: root.autoGrow ? "mainComposerEditor" : "trayComposerEditor"
        property bool wispTextEditor: true
        // Keep the submitted draft stable until its acknowledgement arrives.
        readOnly: root.busy || root.pendingAccess
        onActiveFocusChanged: { if (activeFocus) root.editorFocused(); else root.stopTyping() }
        text: root.bridge.draftFor(root.conversationId)
        onTextChanged: {
          var edited = root.bridge.draftFor(root.conversationId) !== text
          root.bridge.setDraft(root.conversationId, text)
          if (edited && activeFocus && root.visible && !root.busy && root.bridge.typing)
            root.bridge.typing.edited(root.conversationId,text)
        }
        color: root.theme.foreground
        placeholderText: root.pendingAccess ? "Waiting for chat access" : root.destination ? "Message " + root.destination : "Message"
        placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        wrapMode: TextEdit.Wrap
        verticalAlignment: root.compact ? TextEdit.AlignVCenter : TextEdit.AlignTop
        Binding { target: editor; property: "topPadding"; value: 0; when: root.compact; restoreMode: Binding.RestoreBindingOrValue }
        Binding { target: editor; property: "bottomPadding"; value: 0; when: root.compact; restoreMode: Binding.RestoreBindingOrValue }
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
          if (root.pendingAccess) return
          if (root.handleMentionKey(event)) return
          if (event.key === Qt.Key_Escape && root.replyingTo && !root.busy) {
            root.bridge.messageActions.cancelReply(root.conversationId); event.accepted=true
          } else if (event.matches(StandardKey.Paste)) {
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
    Popup {
      id: mentionPicker; objectName: "mentionPicker"
      parent: messageBox
      y: -height-root.theme.spacing.xs
      width: Math.min(root.width,root.theme.space(340))
      implicitHeight: Math.min(root.mentionChoices.length,5)*root.theme.space(36)+padding*2
      padding: root.theme.spacing.xs
      visible: root.visible && editor.activeFocus && !editor.inputMethodComposing
        && !root.busy && !root.pendingAccess && !!root.mentionQuery && root.mentionChoices.length>0
        && root.dismissedMention!==root.mentionSignature
      focus: false; closePolicy: Popup.NoAutoClose
      background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.accent }
      contentItem: ListView {
        id: mentionList; clip:true
        model: root.mentionChoices
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        delegate: ItemDelegate {
          id: mentionChoice
          required property var modelData
          required property int index
          objectName: "mentionChoice-"+modelData.id
          width: mentionList.width; height: root.theme.space(36)
          focusPolicy: Qt.NoFocus
          Accessible.name: "Mention "+modelData.display_name
          onClicked: root.chooseMention(index)
          background: Rectangle { color: mentionChoice.hovered || mentionChoice.index===root.mentionSelection ? root.theme.alpha(root.theme.accent,0.18) : "transparent"; radius: root.theme.cornerRadius }
          contentItem: Text {
            text: "@"+mentionChoice.modelData.display_name; textFormat: Text.PlainText; elide: Text.ElideRight
            color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
            verticalAlignment: Text.AlignVCenter
          }
        }
      }
    }
    ChatButton {
      id: attachButton; theme: root.theme; objectName: "composerAttachButton"; visible: root.theme.friendly
      text: "+"; iconName: "attach"; iconOnly: true
      width: root.theme.space(32); height: width; anchors.left: parent.left; anchors.bottom: parent.bottom; anchors.margins: root.theme.space(4)
      enabled: !root.busy && !root.pendingAccess; Accessible.name: "Attach files"; onClicked: attachmentPicker.open()
      FileDialog { id: attachmentPicker; title: "Attach files"; fileMode: FileDialog.OpenFiles; onAccepted: root.bridge.importChatFiles(root.conversationId, selectedFiles.map(function(url){return String(url)})) }
    }
  ChatButton {
    id:emojiButton;anchors.right:sendButton.left;anchors.bottom:parent.bottom;anchors.margins:root.theme.space(4);width:root.theme.space(36);height:root.theme.space(32)
    objectName:"composerEmojiButton";theme:root.theme;text:"☺";enabled:!root.busy && !root.pendingAccess
    onClicked:composerEmojiPicker.open()
    EmojiPicker {
      id:composerEmojiPicker;bridge:root.bridge;theme:root.theme;serverId:String((root.conversation || {}).server_id || root.bridge.activeServer.id)
      width:Math.min(root.width,root.theme.space(340));y:-height
      onPicked:emoji=>{editor.insert(editor.cursorPosition,emoji);editor.forceActiveFocus()}
    }
  }
    ChatButton {
      id: sendButton
      objectName: "composerSendButton"
      anchors.right: parent.right; anchors.bottom: parent.bottom
      anchors.margins: root.theme.space(4)
      width: root.theme.space(root.theme.tui || root.theme.comfortable ? 60 : 32); height: root.theme.space(32)
      theme: root.theme; primary: true
      readonly property string statusText: root.bridge.sendingConversations[root.conversationId] ? "Sending" + root.bridge.transferLabel("upload", root.attachments.length ? root.attachments[0].token : "") : root.bridge.importingConversations[root.conversationId] ? "Preparing…" : "Send message"
      Accessible.name: statusText
      ToolTip.visible: hovered
      ToolTip.text: statusText
      enabled: !root.busy && !root.pendingAccess
        && (root.attachments.length > 0 || root.bridge.draftFor(root.conversationId).trim().length > 0)
      onClicked: root.bridge.sendComposedMessage(root.conversationId)
      contentItem: Item {
        WispIcon { anchors.centerIn: parent; theme: root.theme; name: "send"; ink: root.theme.accentText; visible: root.theme.friendly && !root.busy; opacity: sendButton.enabled ? 1 : 0.4 }
        Canvas {
          anchors.centerIn: parent; width: root.theme.space(14); height: width
          visible: !root.busy && !root.theme.tui && !root.theme.comfortable && !root.theme.friendly
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
          visible: (root.theme.tui || root.theme.comfortable) && !root.busy
          text: root.theme.comfortable ? "Send" : "[send]"
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
