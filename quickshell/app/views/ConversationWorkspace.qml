import QtQuick
import QtQuick.Controls
import "../components"
import "../ChatLogic.js" as ChatLogic

Rectangle {
  id: root
  required property var bridge
  required property var theme
  property var tabIds: []
  property string confirmationId: ""
  readonly property string currentId: String(bridge.activeConversationId || "")
  readonly property var current: bridge.conversationById(currentId)
  color: theme.surface
  radius: theme.cornerRadius

  function label(c) { return c && c.label === "Hangout" ? "Room" : String(c ? c.label : "Messages") }
  function syncTabs() {
    tabIds = ChatLogic.reconcileTabs(tabIds, bridge.conversations)
    if (visible && (!current || current.tab_closed) && tabIds.length > 0)
      bridge.selectConversation(tabIds[0])
    else if (current && current.tab_closed && tabIds.length === 0) bridge.closeConversation()
  }
  Component.onCompleted: syncTabs()
  onVisibleChanged: if (visible) syncTabs()
  Connections { target: root.bridge; function onConversationsChanged() { root.syncTabs() } }

  Row {
    id: heading
    anchors.top: parent.top; anchors.left: parent.left; anchors.right: parent.right
    anchors.margins: root.theme.space(18)
    height: root.theme.space(36)
    Text {
      width: parent.width - allButton.width
      anchors.verticalCenter: parent.verticalCenter
      text: "Messages"
      color: root.theme.foreground; font.pixelSize: root.theme.font.title; font.bold: true
    }
    ChatButton { id: allButton; theme: root.theme; text: "All conversations ▾"; onClicked: allMenu.open() }
  }
  Menu {
    id: allMenu
    palette.window: root.theme.surface
    palette.text: root.theme.foreground
    x: root.width - width - root.theme.space(18); y: heading.y + heading.height
    Repeater {
      model: root.bridge.conversations
      MenuItem {
        required property var modelData
        text: root.label(modelData) + (modelData.tab_closed ? " · closed" : "")
        onTriggered: root.bridge.selectConversation(modelData.id)
      }
    }
  }
  Flickable {
    id: tabs
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: heading.bottom
    anchors.margins: root.theme.space(12)
    height: root.theme.space(42)
    contentWidth: tabRow.width; contentHeight: height
    flickableDirection: Flickable.HorizontalFlick; clip: true
    Row {
      id: tabRow
      spacing: root.theme.spacing.md
      Repeater {
        model: root.tabIds
        ChatButton {
          required property string modelData
          readonly property var conversation: root.bridge.conversationById(modelData)
          theme: root.theme
          primary: root.currentId === modelData
          text: root.label(conversation) + (conversation && conversation.unread_count ? " · " + conversation.unread_count : "")
          onClicked: root.bridge.selectConversation(modelData)
        }
      }
    }
    ScrollBar.horizontal: ScrollBar {}
  }
  Row {
    id: chatHeading
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: tabs.bottom
    anchors.margins: root.theme.space(18)
    height: root.theme.space(36)
    visible: !!root.current
    Column {
      width: parent.width - optionsButton.width
      Text { text: root.label(root.current); color: root.theme.foreground; font.bold: true; font.pixelSize: root.theme.font.body }
      Text { text: root.current && root.current.kind === "direct" ? "Direct message · history saved" : "Conversation · history saved"; color: root.theme.muted; font.pixelSize: root.theme.font.caption }
    }
    ChatButton { id: optionsButton; theme: root.theme; text: "Chat options ▾"; onClicked: optionsMenu.open() }
  }
  Menu {
    id: optionsMenu
    palette.window: root.theme.surface
    palette.text: root.theme.foreground
    x: root.width - width - root.theme.space(18); y: chatHeading.y + chatHeading.height
    MenuItem { text: "Close conversation"; onTriggered: root.bridge.exitConversation(root.currentId) }
    MenuItem { text: "Room settings…"; visible: !!(root.current && root.current.spot_id); height: visible ? implicitHeight : 0; onTriggered: roomManager.manage(root.currentId) }
    MenuSeparator {}
    MenuItem { text: "Clear Chat History…"; onTriggered: confirmClear.confirm(root.currentId) }
  }
  MessageFeed {
    anchors.left: parent.left; anchors.right: parent.right
    anchors.top: chatHeading.bottom; anchors.bottom: composer.top
    anchors.margins: root.theme.space(16)
    visible: !!root.current
    bridge: root.bridge; theme: root.theme; conversationId: root.currentId
  }
  ChatComposer {
    id: composer
    anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
    anchors.margins: root.theme.space(16)
    visible: !!root.current
    bridge: root.bridge; theme: root.theme; conversationId: root.currentId
    spacious: true
  }
  Text {
    anchors.centerIn: parent
    visible: !root.current
    width: parent.width - root.theme.space(48)
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.Wrap
    text: "No conversations open\nMessage a friend or reopen one from All conversations."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  ChatDropArea {
    anchors.left: parent.left; anchors.right: parent.right
    anchors.top: chatHeading.bottom; anchors.bottom: parent.bottom
    z: 5
    visible: !!root.current
    bridge: root.bridge; theme: root.theme; conversationId: root.currentId
  }
  ClearHistoryDialog { id: confirmClear; bridge: root.bridge; theme: root.theme }
  RoomManager { id: roomManager; bridge: root.bridge; theme: root.theme }
}
