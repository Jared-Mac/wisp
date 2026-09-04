import QtQuick
import QtQuick.Controls
import "../components"
import "../ChatLogic.js" as ChatLogic

Rectangle {
  id: root
  required property var bridge
  required property var theme
  property var workspaceLayout: null
  property bool tiled: false
  property string selectedId: ""
  property bool paneActive: true
  property bool canSplit: false
  property bool canClosePane: false
  property bool detached: false
  signal popOutRequested()
  signal dockRequested()
  signal conversationChosen(string id)
  signal activated()
  signal splitRequested(string edge)
  signal closePaneRequested()
  signal tileDragged(real px, real py)
  signal tileDropped()
  signal tileDragCanceled()
  readonly property real panelMargin: theme.space(8)
  property var tabIds: []
  property string confirmationId: ""
  readonly property string currentId: tiled ? selectedId : String(bridge.activeConversationId || "")
  readonly property var current: bridge.conversationById(currentId)
  readonly property color chatBorderColor: bridge.chatColors.colorFor(currentId, theme.conversationBorder)
  color: theme.surface
  radius: theme.cornerRadius
  border.width: theme.performative ? 0 : theme.terminal || tiled ? 1 : 0
  border.color: tiled && paneActive && canClosePane ? theme.alpha(theme.accent,0.65) : theme.conversationBorder
  TerminalFrame {
    objectName: "conversationColorFrame"
    anchors.fill: parent; theme: root.theme
    title: "03: /chat/" + root.label(root.current)
    ink: root.chatBorderColor
    emphasized: root.paneActive && root.tiled && root.canClosePane
  }

  function choose(id) { if (tiled) conversationChosen(id); else bridge.selectConversation(id) }
  TapHandler { onPressedChanged: if (pressed) root.activated() }

  function label(c) { return c && c.label === "Hangout" ? "Room" : String(c ? c.label : "Messages") }
  function syncTabs() {
    tabIds = ChatLogic.reconcileTabs(tabIds, bridge.conversations)
    if (tiled) return
    if (visible && (!current || current.tab_closed) && tabIds.length > 0)
      bridge.selectConversation(tabIds[0])
    else if (current && current.tab_closed && tabIds.length === 0) bridge.closeConversation()
  }
  Component.onCompleted: syncTabs()
  onVisibleChanged: if (visible) syncTabs()
  Connections { target: root.bridge; function onConversationsChanged() { root.syncTabs() } }

  Item {
    id: heading
    anchors.top: parent.top; anchors.left: parent.left; anchors.right: parent.right
    anchors.margins: root.panelMargin
    anchors.topMargin: root.theme.performative ? root.theme.space(22) : root.panelMargin
    height: root.theme.space(34)
    Row {
      id: leadingActions
      anchors.left: parent.left
      spacing: root.theme.spacing.xs
      ChatButton {
        id: tileButton
        objectName: "chatTileMenuButton"
        visible: root.tiled && !root.detached
        theme: root.theme; text: "⠿"; implicitWidth: root.theme.space(28)
        Accessible.name: "Chat pane layout; drag to move or swap"
        onClicked: tileMenu.open()
        MouseArea {
          id: tileDragMouse
          anchors.fill: parent; cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
          preventStealing: true
          property point origin
          property bool moving: false
          onPressed: function(event) { origin=Qt.point(event.x,event.y); moving=false; root.activated() }
          onPositionChanged: function(event) {
            if (!pressed) return
            if (Math.abs(event.x-origin.x)+Math.abs(event.y-origin.y)>8) moving=true
            if (moving) { var p=mapToItem(root,event.x,event.y); root.tileDragged(p.x,p.y) }
          }
          onReleased: { if (moving) root.tileDropped(); else tileMenu.open(); moving=false }
          onCanceled: { root.tileDragCanceled(); moving=false }
        }
        ToolTip.visible: hovered && !tileDragMouse.pressed
        ToolTip.text: "Split chat pane, or drag to move/swap"
      }
      ChatButton {
        objectName: "chatAnchorButton"
        visible: root.detached
        theme: root.theme; implicitWidth: root.theme.space(30)
        Accessible.name: "Return to main window"
        onClicked: root.dockRequested()
        ToolTip.visible: hovered; ToolTip.text: "Return to main window"
        contentItem: Image {
          source: "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="' + root.theme.foreground + '" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="5" r="3"/><path d="M12 8v13M8 12h8M3 13v3a9 5 0 0 0 18 0v-3M1 15l2-2 2 2M19 15l2-2 2 2"/></svg>')
          fillMode: Image.Pad
        }
      }
    }
    Row {
      id: toolbarActions
      anchors.right: parent.right
      spacing: root.theme.spacing.xs
      ChatButton { id: optionsButton; objectName: "chatOptionsButton"; theme: root.theme; text: "⋯"; implicitWidth: root.theme.space(34); visible: !!root.current; Accessible.name: "Chat options"; onClicked: optionsMenu.open() }
    }
    ChatButton {
      id: chatSelector
      objectName: "compactChatSelector"
      anchors.left: leadingActions.right
      anchors.leftMargin: root.theme.spacing.xs
      readonly property real availableHeaderWidth: Math.max(0, toolbarActions.x - leadingActions.x - leadingActions.width - root.theme.spacing.xs * 2)
      width: root.theme.performative
        ? Math.min(availableHeaderWidth, Math.max(root.theme.space(88), Math.min(root.theme.space(260), selectorLabel.implicitWidth + root.theme.space(32))))
        : availableHeaderWidth
      theme: root.theme; primary: !root.theme.performative
      readonly property color selectorInk: root.theme.performative ? (visualFocus ? root.chatBorderColor : root.theme.foreground) : root.theme.terminal ? root.theme.accent : root.theme.accentText
      Binding { target: chatSelector; property: "leftPadding"; value: root.theme.space(8); when: root.theme.performative; restoreMode: Binding.RestoreBindingOrValue }
      Binding { target: chatSelector; property: "rightPadding"; value: root.theme.space(8); when: root.theme.performative; restoreMode: Binding.RestoreBindingOrValue }
      Binding { target: chatSelector.background; property: "color"; value: root.theme.alpha(root.theme.foreground, chatSelector.down ? 0.10 : chatSelector.hovered ? 0.06 : 0.025); when: root.theme.performative; restoreMode: Binding.RestoreBindingOrValue }
      Binding { target: chatSelector.background; property: "border.width"; value: 1; when: root.theme.performative; restoreMode: Binding.RestoreBindingOrValue }
      Binding { target: chatSelector.background; property: "border.color"; value: chatSelector.visualFocus ? root.chatBorderColor : chatSelector.hovered ? root.theme.muted : root.theme.separator; when: root.theme.performative; restoreMode: Binding.RestoreBindingOrValue }
      text: root.label(root.current) + (root.current && root.current.unread_count ? " · " + root.current.unread_count : "")
      Accessible.name: "Current chat: " + text + ". Choose conversation"
      ToolTip.visible: hovered && selectorLabel.truncated && !allMenu.opened
      ToolTip.text: text
      onClicked: allMenu.open()
      contentItem: Item {
        Text {
          id: selectorLabel
          anchors.left: parent.left; anchors.right: selectorArrow.left
          anchors.rightMargin: root.theme.spacing.xs; anchors.verticalCenter: parent.verticalCenter
          text: chatSelector.text; elide: Text.ElideRight
          color: chatSelector.selectorInk; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        Text { id: selectorArrow; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: "▾"; color: root.theme.performative ? root.theme.muted : chatSelector.selectorInk; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
      }
    }
  }
  ConversationPicker {
    id: allMenu
    objectName: "chatConversationMenu"
    bridge: root.bridge; theme: root.theme; selectedId: root.currentId
    width: Math.min(root.width - root.panelMargin * 2, root.theme.space(380))
    x: root.theme.performative ? Math.min(heading.x + chatSelector.x, root.width - width - root.panelMargin) : Math.max(root.panelMargin, root.width - width - root.panelMargin)
    y: heading.y + heading.height
    onChosen: function(id) { root.choose(id) }
    onNewChatRequested: newChatDialog.begin()
  }
  NewChatDialog { id:newChatDialog; objectName:"newChatDialog"; bridge:root.bridge; theme:root.theme; onCreated:function(id) {root.choose(id)} }
  Menu {
    id: tileMenu
    objectName: "chatTileMenu"
    x: root.panelMargin; y: heading.y+heading.height
    ThemeControlStyle { theme: root.theme; control: tileMenu; outline: true; menuOutline: true }
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    MenuItem { id: splitRight; text: "Split right"; enabled: root.canSplit; onTriggered: root.splitRequested("right"); ThemeControlStyle { theme: root.theme; control: splitRight } }
    MenuItem { id: splitBelow; text: "Split below"; enabled: root.canSplit; onTriggered: root.splitRequested("bottom"); ThemeControlStyle { theme: root.theme; control: splitBelow } }
    MenuItem { id: popOut; text: "Pop out chat"; onTriggered: root.popOutRequested(); ThemeControlStyle { theme: root.theme; control: popOut } }
    MenuSeparator {}
    MenuItem { id: closeTile; text: "Close pane"; enabled: root.canClosePane; onTriggered: root.closePaneRequested(); ThemeControlStyle { theme: root.theme; control: closeTile } }
  }
  Menu {
    ThemeControlStyle { theme: root.theme; control: optionsMenu; outline: true; menuOutline: true }
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
    id: optionsMenu
    objectName: "wispChatOptions"
    palette.window: root.theme.surface
    palette.text: root.theme.foreground
    x: root.width - width - root.panelMargin; y: heading.y + heading.height
    MenuItem {
      id: trialControl1
      objectName: "closeConversationAction"
      ThemeControlStyle { theme: root.theme; control: trialControl1 }
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
      text: root.tiled ? (root.detached ? "Return to main window" : "Close tile") : "Close conversation"
      enabled: !root.tiled || root.detached || root.canClosePane
      onTriggered: {
        if (!root.tiled) root.bridge.exitConversation(root.currentId)
        else if (root.detached) root.dockRequested()
        else root.closePaneRequested()
      }
    }
    MenuItem {
      id: trialControl2
      ThemeControlStyle { theme: root.theme; control: trialControl2 }
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
       text: "Room settings…"; visible: !!(root.current && root.current.spot_id); height: visible ? implicitHeight : 0; onTriggered: roomManager.manage(root.currentId) }
    MenuSeparator {}
    MenuItem {
      id: trialControl3
      ThemeControlStyle { theme: root.theme; control: trialControl3 }
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
       text: "Clear Chat History…"; onTriggered: confirmClear.confirm(root.currentId) }
  }
  MessageFeed {
    anchors.left: parent.left; anchors.right: parent.right
    anchors.top: heading.bottom; anchors.bottom: composerPane.top
    anchors.margins: root.panelMargin
    visible: !!root.current
    bridge: root.bridge; theme: root.theme; conversationId: root.currentId
  }
  Flickable {
    id: composerPane
    objectName: "composerPane"
    anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
    anchors.margins: root.panelMargin
    visible: !!root.current
    readonly property real available: Math.max(1, root.height - heading.y - heading.height - root.panelMargin * 2)
    height: Math.min(available * 0.45, composer.implicitHeight)
    contentWidth: width; contentHeight: composer.implicitHeight
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar { policy: composerPane.contentHeight > composerPane.height ? ScrollBar.AlwaysOn : ScrollBar.AsNeeded }
    ChatComposer {
      id: composer
      width: parent.width
      bridge: root.bridge; theme: root.theme; conversationId: root.currentId
      spacious: true
      autoGrow: true
      onEditorFocused: root.activated()
      maximumEditorHeight: Math.max(root.theme.space(40), Math.min(root.theme.space(160), composerPane.available * 0.30))
    }
  }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    anchors.centerIn: parent
    visible: !root.current
    width: parent.width - root.theme.space(48)
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.Wrap
    text: "No conversations open\nMessage a friend or reopen one from Chats."
    color: root.theme.muted; font.pixelSize: root.theme.font.body
  }
  ChatDropArea {
    anchors.left: parent.left; anchors.right: parent.right
    anchors.top: heading.bottom; anchors.bottom: parent.bottom
    z: 5
    visible: !!root.current
    bridge: root.bridge; theme: root.theme; conversationId: root.currentId
  }
  ClearHistoryDialog { id: confirmClear; bridge: root.bridge; theme: root.theme }
  RoomManager { id: roomManager; bridge: root.bridge; theme: root.theme }
}
