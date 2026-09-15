import QtQuick
import QtQuick.Controls
import "../ChatMarkup.js" as Markup

Rectangle {
  id: root
  objectName: "messageFeed"
  required property var bridge
  required property var theme
  required property string conversationId
  signal replyRequested()
  readonly property bool compactChat: theme.chatLayout === "compact"
  readonly property bool softGroups: theme.chatLayout === "soft_groups"
  readonly property bool chatAvatars: theme.showAvatars && !compactChat
  function sameRun(previous, current) {
    if (!previous || !current) return false
    var gap=Date.parse(current.created_at)-Date.parse(previous.created_at)
    return String(previous.sender.id)===String(current.sender.id)
      && String(previous.server_id || bridge.activeServer.id)===String(current.server_id || bridge.activeServer.id)
      && previous.content_type!=="application/vnd.wisp.room-invitation+json"
      && current.content_type!=="application/vnd.wisp.room-invitation+json"
      && gap>=0 && gap<5*60*1000
      && new Date(previous.created_at).toDateString()===new Date(current.created_at).toDateString()
  }
  property bool readerFocused: false
  property bool readerEngaged: false
  property string readerKey: ""
  readonly property var unreadBoundary: bridge.unreadMarkers.boundary(conversationId)
  function engageReader() { readerEngaged = true; scheduleRead() }
  function scheduleRead() {
    if (!readerKey) return
    bridge.unreadMarkers.report(readerKey, conversationId, readerFocused, false)
    readTimer.restart()
  }
  onReaderFocusedChanged: { if (!readerFocused) { if(readerEngaged && !awayFromLatest) bridge.acknowledgeConversation(conversationId,false); readerEngaged=false } scheduleRead() }
  onAwayFromLatestChanged: scheduleRead()
  Timer {
    id: readTimer; interval: 700
    onTriggered: if (root.readerFocused && root.readerEngaged && !root.awayFromLatest)
      root.bridge.unreadMarkers.report(root.readerKey,root.conversationId,true,true)
  }
  Component.onDestruction: if (readerKey) bridge.unreadMarkers.report(readerKey,"",false,false)

  property string highlightedId: ""
  function revealMessage(id) {
    for (var i=0;i<stableMessages.count;i++) if (String(stableMessages.get(i).modelData.id)===String(id)) {
      messages.followBottom=false; messages.positionViewAtIndex(i,ListView.Center); highlightedId=String(id); highlightTimer.restart(); return true
    }
    return false
  }
  Timer { id: highlightTimer; interval: 2500; onTriggered: root.highlightedId="" }
  Connections { target: root.bridge.messageActions; function onMessageLocated(conversationId,messageId) { if (root.bridge.messageActions.canonical(root.conversationId)===conversationId) Qt.callLater(function(){root.revealMessage(messageId)}) } }
  ParticipantMenu { id: authorMenu; bridge:root.bridge; theme:root.theme; voiceControls:false }
  ForwardMessageDialog { id: forwardDialog; bridge: root.bridge; theme: root.theme }
  property string editingId: ""
  property string deletingId: ""
  property bool editingImage: false
  property bool savingEdit: false
  property string editError: ""
  readonly property var incomingMessages: bridge.messagesFor(conversationId)
  readonly property string latestMessageId: incomingMessages.length ? String(incomingMessages[incomingMessages.length-1].id) : ""
  onLatestMessageIdChanged: scheduleRead()
  // Preserve delegates (including a playing embed) across unrelated snapshots.
  ListModel {id:stableMessages;dynamicRoles:true}
  function syncMessages() {
    var incoming=incomingMessages || []
    for(var i=0;i<incoming.length;i++) {
      var id=String(incoming[i].id), found=-1
      for(var j=i;j<stableMessages.count;j++) if(String(stableMessages.get(j).modelData.id)===id){found=j;break}
      if(found<0) stableMessages.insert(i,{modelData:incoming[i]})
      else {
        if(found!==i) stableMessages.move(found,i,1)
        if(JSON.stringify(stableMessages.get(i).modelData)!==JSON.stringify(incoming[i])) stableMessages.set(i,{modelData:incoming[i]})
      }
    }
    if(stableMessages.count>incoming.length)stableMessages.remove(incoming.length,stableMessages.count-incoming.length)
  }
  // Status snapshots rebuild the array even when the chat is unchanged. Only
  // new messages or reader/scroll changes should reset the read dwell timer.
  onIncomingMessagesChanged: syncMessages()
  Component.onCompleted: { readerKey="feed-"+(++bridge.unreadMarkers.serial); syncMessages(); scheduleRead() }
  readonly property bool editOpen: editDialog.opened
  readonly property bool awayFromLatest: messages.count > 0 && !messages.atYEnd
    && messages.contentHeight + messages.originY - messages.contentY - messages.height > theme.space(4)
  function scrollToLatest() {
    engageReader()
    messages.cancelFlick()
    messages.followBottom = true
    messages.positionViewAtEnd()
    Qt.callLater(messages.followLatest)
  }
  function beginEdit(message) {
    editingId = String(message.id)
    editingImage = message.content_type === "image/png" || message.content_type === "application/octet-stream"
    editField.text = editingImage ? String(message.payload.caption || "") : String(message.payload || "")
    editError = ""
    editDialog.open()
    editField.forceActiveFocus()
  }
  function saveEdit() {
    if (savingEdit || (!editingImage && !editField.text.trim())) return
    editError = ""
    savingEdit = true
    if (!bridge.editChatMessage(editingId, editField.text)) savingEdit = false
  }
  radius: theme.cornerRadius
  border.width: theme.tui ? 0 : theme.terminal ? 1 : 0
  border.color: theme.separator
  color: theme.friendly ? "transparent" : theme.tui ? (theme.cleanTui ? "transparent" : theme.background) : theme.alpha(theme.foreground, 0.025)
  onConversationIdChanged: { readerEngaged=false; scheduleRead(); messages.followBottom = true; Qt.callLater(messages.followLatest) }
  ListView {
    id: messages
    objectName: "messageList"
    anchors.fill: parent
    anchors.margins: root.theme.space(root.theme.cleanTui ? 12 : root.theme.tui ? 6 : 16)
    clip: true
    spacing: 0
    model: stableMessages
    property bool followBottom: true
    function followLatest() { if (followBottom && !moving && !messageScrollBar.pressed) positionViewAtEnd() }
    onMovementStarted: { followBottom = false; root.engageReader() }
    onMovementEnded: followBottom = !root.awayFromLatest
    onCountChanged: if (followBottom) Qt.callLater(followLatest)
    onContentHeightChanged: if (followBottom) Qt.callLater(followLatest)
    onHeightChanged: if (followBottom) Qt.callLater(followLatest)
    ScrollBar.vertical: ScrollBar {
      id: messageScrollBar
      onPressedChanged: messages.followBottom = pressed ? false : !root.awayFromLatest
    }
    delegate: Item {
      id: message
      required property int index
      readonly property var previousMessage: index > 0 ? root.incomingMessages[index-1] : null
      readonly property bool sameAuthor: !startsUnread && root.sameRun(previousMessage,modelData)
      readonly property var nextMessage: index+1<root.incomingMessages.length ? root.incomingMessages[index+1] : null
      readonly property bool endsRun: !root.sameRun(modelData,nextMessage)
        || (!!root.unreadBoundary && nextMessage && root.unreadBoundary.firstId===String(nextMessage.id))
      readonly property real messageGap: index===0 ? (root.compactChat ? root.theme.space(22) : 0) : root.theme.space(sameAuthor ? (root.softGroups ? 0 : 3) : root.compactChat ? 8 : 14)
      readonly property real topInset: root.softGroups ? root.theme.space(sameAuthor ? 3 : 10) : 0
      readonly property real bottomInset: root.softGroups ? root.theme.space(endsRun ? 10 : 3) : 0
      readonly property real contentTop: newMessagesDivider.height + messageGap + topInset
      readonly property color senderColor: !root.theme.colorEnabled("senderNames") ? root.theme.foreground : ownMessage ? root.theme.accent : root.theme.secondaryAccent
      readonly property string shortTime: Qt.formatDateTime(new Date(modelData.created_at),root.theme.tui ? "HH:mm" : "h:mm AP")
      readonly property bool inlineBody: root.compactChat && !isImage && !isFile && !isInvitation
        && !(modelData.context || {}).reply_to && !(modelData.context || {}).forwarded_from
      // Use the preference, not effective child visibility: hiding a tile must
      // not feed back into its delegate geometry while the view relayouts.
      implicitHeight: Math.max(transcript.implicitHeight, root.chatAvatars && !sameAuthor ? root.theme.space(32) : 0) + contentTop + bottomInset
      readonly property bool startsUnread: !!root.unreadBoundary && root.unreadBoundary.firstId===String(modelData.id)
      required property var modelData
      readonly property bool isImage: modelData.content_type === "image/png"
      readonly property bool isFile: modelData.content_type === "application/octet-stream"
      readonly property bool isInvitation: modelData.content_type === "application/vnd.wisp.room-invitation+json"
      readonly property bool ownMessage: modelData.sender.id === root.bridge.selfState.id
      readonly property string copyText: isImage ? String(modelData.payload.caption || "") : isFile ? String(modelData.payload.caption || modelData.payload.file_name || "") : isInvitation ? "" : String(modelData.payload || "")
      readonly property string imageUrl: root.bridge.chatImageUrls[String(modelData.id)] || ""
      readonly property string serverId: String(modelData.server_id || root.bridge.activeServer.id)
      width: Math.min(messages.width, root.theme.comfortable ? root.theme.space(860) : messages.width)
      Rectangle {
        id: groupBackground; objectName:"messageGroup-"+String(message.modelData.id)
        visible:root.softGroups; x:0; y:newMessagesDivider.height+message.messageGap
        width:parent.width; height:parent.height-y
        color:Qt.tint(root.theme.background,root.theme.alpha(root.theme.foreground,root.theme.light ? 0.055 : 0.045))
        radius:root.theme.space(10)
        Rectangle {visible:message.sameAuthor;anchors.top:parent.top;width:parent.width;height:parent.radius;color:parent.color}
        Rectangle {visible:!message.endsRun;anchors.bottom:parent.bottom;width:parent.width;height:parent.radius;color:parent.color}
      }
      Rectangle { anchors.fill: parent; visible: root.highlightedId===String(message.modelData.id); color: root.theme.alpha(root.theme.accent,0.12); radius: root.theme.cornerRadius }
      HoverHandler { id: messageHover }
      Component.onCompleted: { if (isImage) root.bridge.loadChatImage(String(modelData.id));root.bridge.chatExtras.loadText(serverId,copyText) }
      onCopyTextChanged:root.bridge.chatExtras.loadText(serverId,copyText)
      Item {
        id: newMessagesDivider
        y: message.messageGap
        objectName: "newMessagesDivider-" + String(message.modelData.id)
        width: messages.width; height: message.startsUnread ? root.theme.space(28) : 0
        visible: message.startsUnread
        Accessible.role: Accessible.StaticText
        Accessible.name: "New messages start here"
        Rectangle {
          anchors.left: parent.left; anchors.right: newMessagesLabel.left; anchors.rightMargin: root.theme.space(8)
          anchors.verticalCenter: newMessagesLabel.verticalCenter; height: 1; color: root.theme.accent
        }
        Text {
          id: newMessagesLabel; anchors.right: parent.right
          text: "New messages"; color: root.theme.accent
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.bold: true
        }
      }
      WispAvatar {
        id: avatar; objectName:"messageAvatar-"+String(message.modelData.id)
        x: root.softGroups ? root.theme.space(10) : 0
        y: message.contentTop
        TapHandler { onTapped: authorMenu.showPerson(Object.assign({},message.modelData.sender,{server_id:message.serverId}),parent) }
        HoverHandler { cursorShape:Qt.PointingHandCursor }
        bridge: root.bridge; userId: String(message.modelData.sender.id); serverId: message.serverId; theme: root.theme; name: message.modelData.sender.display_name || ""; visible: root.chatAvatars && !message.sameAuthor; width: root.theme.space(32); height: width }
      Text {
        objectName:"messageTime-"+String(message.modelData.id)
        visible:root.compactChat || message.sameAuthor
        x:0; y:message.contentTop+root.theme.space(2)
        width:root.compactChat ? root.theme.space(52) : Math.max(0,transcript.x-root.theme.space(6))
        opacity: !message.sameAuthor || messageHover.hovered ? 1 : 0
        text:message.shortTime; textFormat:Text.PlainText; elide:Text.ElideRight
        color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
      }
      Row {
        id: actionToolbar
        anchors.right:parent.right; anchors.rightMargin:root.softGroups ? root.theme.space(8) : 0
        y:message.contentTop-root.theme.space(root.compactChat ? 22 : message.sameAuthor ? 16 : 0)
        z:5; spacing:root.theme.space(3)
        HoverHandler {id:actionHover}
        Rectangle {parent:message;x:actionToolbar.x-2;y:actionToolbar.y-2;width:actionToolbar.width+4;height:actionToolbar.height+4;z:4;radius:root.theme.cornerRadius;color:root.theme.surface;visible:messageHover.hovered || actionHover.hovered || optionsButton.activeFocus || reactionBar.actionEngaged || messageMenu.opened}
        ChatButton {
          id: optionsButton; objectName: "messageOptions-" + String(message.modelData.id)
          theme: root.theme; text: "···"; quiet: root.theme.comfortable || root.theme.refinedTui
          opacity: messageHover.hovered || actionHover.hovered || reactionBar.actionEngaged || activeFocus || messageMenu.opened ? 1 : 0
          ToolTip.visible: hovered; ToolTip.text: "Message actions · " + Qt.formatDateTime(new Date(message.modelData.created_at), "MMM d, h:mm:ss AP")
          Accessible.name: "Message actions"
          implicitWidth: root.theme.space(26); implicitHeight: root.theme.space(20)
          onClicked: messageMenu.open()
          Menu {
            ThemeControlStyle { theme: root.theme; control: messageMenu; outline: true; menuOutline: true }
            Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
            Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
            id: messageMenu
            onAboutToShow: root.bridge.messageActions.loadPins(root.conversationId)
            objectName: "messageMenu-" + String(message.modelData.id)
            palette.window: root.theme.surface
            palette.text: root.theme.foreground
            MenuItem {
              id: replyControl; objectName: "replyMessage-" + String(message.modelData.id)
              text: "Reply"; enabled: !message.isInvitation && !(root.bridge.conversationById(root.conversationId) || {}).pending_access
              ThemeControlStyle { theme: root.theme; control: replyControl }
              onTriggered: { root.bridge.messageActions.beginReply(root.conversationId,message.modelData); root.replyRequested() }
            }
            MenuItem {
              id: forwardControl; objectName: "forwardMessage-" + String(message.modelData.id)
              text: "Forward…"; enabled: !message.isInvitation && !(message.isFile && message.modelData.payload.expired)
              ThemeControlStyle { theme: root.theme; control: forwardControl }
              onTriggered: forwardDialog.showMessage(message.modelData)
            }
            MenuItem {
              id: pinControl; objectName: "pinMessage-" + String(message.modelData.id)
              visible: root.bridge.messageActions.canPin(root.conversationId); height: visible ? implicitHeight : 0
              enabled: root.bridge.messageActions.pinsFor(root.conversationId).ready
              text: root.bridge.messageActions.isPinned(root.conversationId,message.modelData.id) ? "Unpin message" : "Pin message"
              ThemeControlStyle { theme: root.theme; control: pinControl }
              onTriggered: root.bridge.messageActions.setPin(root.conversationId,message.modelData.id,!root.bridge.messageActions.isPinned(root.conversationId,message.modelData.id))
            }
            MenuItem {
              id: copyControl
              objectName: "copyMessage-" + String(message.modelData.id)
              ThemeControlStyle { theme: root.theme; control: copyControl }
              font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
              text: "Copy"
              enabled: message.copyText.length > 0 || message.isImage
              onTriggered: {
                if (message.copyText.length > 0) root.bridge.copyChatText(message.copyText)
                else if (message.isImage) root.bridge.copyChatImage(String(message.modelData.id))
              }
            }
            MenuItem {
              id: trialControl0
              objectName: "editMessage-" + String(message.modelData.id)
              ThemeControlStyle { theme: root.theme; control: trialControl0 }
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
              Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
               visible: message.ownMessage && !message.isInvitation; height: visible ? implicitHeight : 0
               text: message.isImage || message.isFile ? "Edit caption…" : "Edit message…"; onTriggered: root.beginEdit(message.modelData) }
            MenuItem {
              id: trialControl1
              objectName: "deleteMessage-" + String(message.modelData.id)
              visible: message.ownMessage; height: visible ? implicitHeight : 0
              ThemeControlStyle { theme: root.theme; control: trialControl1 }
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
              Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
               text: "Delete message…"; onTriggered: { root.deletingId = String(message.modelData.id); deleteDialog.open() } }
          }
        }
      }
      Column {
        id: transcript
        y: message.contentTop
        x: (root.softGroups ? root.theme.space(10) : 0) + (root.compactChat ? root.theme.space(58) : root.chatAvatars ? root.theme.space(44) : 0)
        width: Math.max(1,parent.width-x-(root.softGroups ? root.theme.space(10) : 0))
        spacing: root.theme.space(4)
      Row {
        id: messageHeading
        visible: root.compactChat ? !message.inlineBody : !message.sameAuthor
        spacing: root.theme.space(8)
        Button {
          id: authorButton
          objectName:"messageAuthor-"+String(message.modelData.id)
          padding:0; implicitHeight:authorLabel.implicitHeight; implicitWidth:Math.min(authorLabel.implicitWidth,Math.max(24,transcript.width-root.theme.space(root.compactChat ? 64 : 125)))
          Accessible.name:"View "+String(message.modelData.sender.display_name || "member")
          onClicked:authorMenu.showPerson(Object.assign({},message.modelData.sender,{server_id:message.serverId}),authorButton)
          background:Rectangle {color:authorButton.hovered ? root.theme.alpha(root.theme.accent,0.10) : "transparent";radius:2}
          contentItem:Text {
          id:authorLabel; textFormat:Text.PlainText;elide:Text.ElideRight
          text: (root.theme.cleanTui || root.theme.refinedTui) ? String(message.modelData.sender.display_name || "") : root.theme.tui ? "<" + String(message.modelData.sender.display_name || "") + ">" : String(message.modelData.sender.display_name || "")
          color: message.senderColor
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.bold: true
        }
        }
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          visible: !root.compactChat
          text: root.theme.friendly ? Qt.formatDateTime(new Date(message.modelData.created_at), "h:mm AP") : (root.theme.cleanTui || root.theme.refinedTui) ? Qt.formatDateTime(new Date(message.modelData.created_at), "HH:mm") : root.theme.tui ? "[" + Qt.formatDateTime(new Date(message.modelData.created_at), "HH:mm:ss") + "]" : Qt.formatDateTime(new Date(message.modelData.created_at), "MMM d · h:mm AP")
          color: root.theme.muted; font.pixelSize: root.theme.font.caption
        }
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          visible: !!message.modelData.edited_at
          text: "edited"
          color: root.theme.alpha(root.theme.muted, 0.8)
          font.pixelSize: root.theme.space(root.theme.comfortable ? 12 : 10)
        }

      }
      Button {
        id: replyPreview; objectName: "messageReplyPreview-" + String(message.modelData.id)
        readonly property var reference: (message.modelData.context || {}).reply_to || null
        visible: !!reference; width: parent.width; height: visible ? root.theme.space(42) : 0; padding: root.theme.spacing.sm
        Accessible.name: "View message from " + (reference ? reference.sender_name : "")
        onClicked: root.bridge.messageActions.locate(root.conversationId,reference.message_id)
        background: Rectangle { color: root.theme.alpha(root.theme.accent,replyPreview.hovered ? 0.12 : 0.06); radius: root.theme.cornerRadius; Rectangle { width: 2; height: parent.height; color: root.theme.accent } }
        contentItem: Column {
          Text { width: parent.width; textFormat: Text.PlainText; text: "↪ " + String((replyPreview.reference || {}).sender_name || ""); elide: Text.ElideRight; color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
          Text { width: parent.width; textFormat: Text.PlainText; text: String((replyPreview.reference || {}).preview || "").replace(/\s+/g," "); elide: Text.ElideRight; color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
        }
      }
      Text {
        objectName: "messageForwarded-" + String(message.modelData.id)
        visible: !!(message.modelData.context || {}).forwarded_from
        width: parent.width; textFormat: Text.PlainText; elide: Text.ElideRight
        text: "Forwarded · " + String(((message.modelData.context || {}).forwarded_from || {}).sender_name || "")
        color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.italic: true
      }
      Rectangle {
        id: imagePreview
        objectName: "chatImagePreview-" + String(message.modelData.id)
        visible: message.isImage
        readonly property real pixelRatio: Math.max(1, photo.Screen.devicePixelRatio)
        readonly property real nativeWidth: Math.max(1,Number(message.modelData.payload.width || 320)) / pixelRatio
        readonly property real nativeHeight: Math.max(1,Number(message.modelData.payload.height || 180)) / pixelRatio
        readonly property real previewScale: Math.min(1, parent.width/nativeWidth, Math.max(1,messages.height)/nativeHeight)
        width: message.isImage ? nativeWidth*previewScale : 0
        height: message.isImage ? nativeHeight*previewScale : 0
        radius: root.theme.cornerRadius
        color: root.theme.surface
        Image {
          id: photo
          objectName: "chatImageTexture-" + String(message.modelData.id)
          anchors.fill: parent
          // Decode at the display's pixel density, using buckets to avoid a
          // reload on every resize pixel. Layout uses original metadata so a
          // smaller decoded texture cannot feed back into the preview geometry.
          sourceSize.width: Math.min(imagePreview.nativeWidth * imagePreview.pixelRatio,
            Math.max(1, Math.ceil(width * imagePreview.pixelRatio / 128) * 128))
          source: message.imageUrl
          asynchronous: true
          fillMode: Image.PreserveAspectFit
        }
        Text {
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
          anchors.centerIn: parent
          visible: photo.status !== Image.Ready
          text: root.bridge.imageErrors[String(message.modelData.id)] ? "Image unavailable · click to retry" : "Loading image…"
          color: root.theme.muted
        }
        MouseArea {
          objectName: "openChatImage-" + String(message.modelData.id)
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            if (photo.status === Image.Ready) imageViewer.openImage(message.imageUrl,String(message.modelData.id))
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
            Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
            width: parent.width; elide: Text.ElideMiddle
            text: String(message.modelData.payload.file_name || "File")
            color: root.theme.foreground; font.pixelSize: root.theme.font.body
          }
          Row {
            spacing: root.theme.spacing.lg
            Text {
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
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
              Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
              anchors.verticalCenter: parent.verticalCenter
              text: message.modelData.payload.expired ? "Removed from server" : message.modelData.payload.keep ? "Kept on server" : message.modelData.payload.expires_at ? "Expires " + Qt.formatDateTime(new Date(message.modelData.payload.expires_at), "MMM d, h:mm AP") : "No automatic expiry"
              color: root.theme.muted; font.pixelSize: root.theme.space(root.theme.comfortable ? 12 : 10)
            }
          }
        }
      }
      Rectangle {
        width: parent.width
        height: bodyText.implicitHeight
        color: "transparent"
        radius: root.theme.cornerRadius
        visible: !message.isInvitation && bodyText.text !== ""
      TextEdit {
        id: bodyText
        x: 0
        y: 0
        objectName:"messageBody-"+String(message.modelData.id)
        width: Math.max(1,parent.width-x*2)
        text: (message.inlineBody ? '<a href="wisp-author" style="text-decoration:none;color:'+message.senderColor+'"><b>'+Markup.escape(message.modelData.sender.display_name || "member")+'</b></a>&nbsp; ' : "")
          + root.bridge.chatExtras.richText(message.serverId,message.copyText,Math.round(root.theme.font.body*1.5),root.theme.accent,root.conversationId)
          + ((message.sameAuthor || message.inlineBody) && message.modelData.edited_at ? ' <span style="color:'+root.theme.muted+';font-size:'+root.theme.font.caption+'px">(edited)</span>' : "")
        visible: !message.isInvitation && text !== ""
        color: root.theme.foreground
        readOnly: true; selectByMouse: true
        textFormat: TextEdit.RichText
        selectedTextColor:root.theme.foreground
        onLinkActivated:link=>{if(link==="wisp-author")authorMenu.showPerson(Object.assign({},message.modelData.sender,{server_id:message.serverId}),bodyText);else if(Markup.safeLink(link) && !root.bridge.accountActions.openInvitation(link))Qt.openUrlExternally(link)}
        wrapMode: TextEdit.Wrap
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      }
      }
      ReactionBar {id:reactionBar;allowed:!message.isInvitation;actionHost:actionToolbar;revealActions:messageHover.hovered || actionHover.hovered || optionsButton.activeFocus || messageMenu.opened; width:parent.width;bridge:root.bridge;theme:root.theme;serverId:message.serverId;messageId:String(message.modelData.id)}
      Repeater {
        model:Markup.youtube(message.copyText)
        VideoEmbed {required property string modelData;theme:root.theme;videoId:modelData}
      }
      Loader {
        width: parent.width
        active: message.isInvitation
        sourceComponent: RoomInvitationCard { bridge: root.bridge; theme: root.theme; invitation: message.modelData.payload; outgoing: message.modelData.sender.id === root.bridge.selfState.id }
      }
    }
  }
  }
  ChatImageWindow { id:imageViewer;objectName:"chatImageViewer";theme:root.theme;bridge:root.bridge }
  ChatButton {
    id: latestButton
    objectName: "scrollToLatestButton"
    theme: root.theme
    visible: root.awayFromLatest
    anchors.right: messages.right; anchors.bottom: messages.bottom
    anchors.margins: root.theme.space(8)
    width: root.theme.space(root.theme.performative ? 46 : 34); height: root.theme.space(34)
    z: 2
    Accessible.name: "Scroll to latest messages"
    ToolTip.visible: hovered; ToolTip.text: "Latest messages"
    onClicked: root.scrollToLatest()
    background: Rectangle {
      radius: root.theme.cornerRadius
      color: root.theme.surface
      border.width: latestButton.visualFocus ? 2 : 1
      border.color: latestButton.hovered || latestButton.visualFocus ? root.theme.foreground : root.theme.accent
      Rectangle {
        anchors.fill: parent; radius: parent.radius
        color: root.theme.alpha(root.theme.accent, latestButton.down ? 0.28 : latestButton.hovered ? 0.16 : 0.08)
      }
    }
    contentItem: Item {
      Text {
        objectName: "latestMessagesText"
        anchors.centerIn: parent
        visible: root.theme.performative
        text: "[vv]"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
      }
      Canvas {
        visible: !root.theme.performative
        anchors.centerIn: parent; width: root.theme.space(18); height: width
        property color ink: root.theme.foreground
        onInkChanged: requestPaint()
        onPaint: {
          var ctx=getContext("2d")
          ctx.reset();ctx.strokeStyle=ink;ctx.lineWidth=1.6;ctx.lineCap="round";ctx.lineJoin="round"
          ctx.beginPath();ctx.moveTo(width*0.5,height*0.12);ctx.lineTo(width*0.5,height*0.65)
          ctx.moveTo(width*0.25,height*0.42);ctx.lineTo(width*0.5,height*0.67);ctx.lineTo(width*0.75,height*0.42)
          ctx.moveTo(width*0.2,height*0.86);ctx.lineTo(width*0.8,height*0.86);ctx.stroke()
        }
      }
    }
  }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    anchors.centerIn: parent
    visible: messages.count === 0
    text: root.theme.cleanTui ? "— beginning of chat —" : root.theme.tui ? "-- beginning of chat log --" : "This is the start of your conversation."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  Dialog {
    ThemeControlStyle { theme: root.theme; control: editDialog; outline: true }
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
    id: editDialog
    objectName: "messageEditDialog"
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
          ThemeControlStyle { theme: root.theme; control: editField }
          Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
          Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
          id: editField
          objectName: "messageEditField"
          property bool wispTextEditor: true
          enabled: !root.savingEdit
          color: root.theme.foreground
          wrapMode: TextEdit.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true
          Keys.onPressed: function(event) {
            if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                && !(event.modifiers & Qt.ShiftModifier)) {
              if (editField.inputMethodComposing) return
              if (!event.isAutoRepeat) root.saveEdit()
              event.accepted = true
            }
          }
          background: Rectangle {
            color: root.theme.background; radius: root.theme.cornerRadius
            border.width: root.theme.terminal ? 1 : 0
            border.color: editField.activeFocus ? root.theme.focusBorder : root.theme.separator
          }
        }
      }
      Text {
        Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
         width: parent.width; text: root.editError; visible: text !== ""; color: root.theme.danger; wrapMode: Text.Wrap; font.pixelSize: root.theme.font.caption }
      Row {
        spacing: root.theme.spacing.lg
        ChatButton { theme: root.theme; text: "Cancel"; enabled: !root.savingEdit; onClicked: editDialog.close() }
        ChatButton {
          theme: root.theme; primary: true; text: root.savingEdit ? "Saving…" : "Save changes"
          enabled: !root.savingEdit && (root.editingImage || editField.text.trim().length > 0)
          onClicked: root.saveEdit()
        }
      }
    }
  }
  Dialog {
    ThemeControlStyle { theme: root.theme; control: deleteDialog; outline: true }
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
    id: deleteDialog
    parent: Overlay.overlay
    x: parent ? (parent.width - width) / 2 : 0; y: parent ? (parent.height - height) / 2 : 0
    width: Math.min(root.width, root.theme.space(400)); implicitHeight: root.theme.space(210)
    modal: true
    title: "Delete message?"
    standardButtons: Dialog.Cancel | Dialog.Yes
    palette.window: root.theme.surface; palette.windowText: root.theme.foreground
    contentItem: Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
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
