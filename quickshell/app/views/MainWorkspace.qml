import QtQuick
import QtQuick.Controls
import "../components"

Item {
  id: root
  objectName: "mainWorkspace"
  required property var bridge
  required property var theme
  signal cameraRequested()
  signal revealMainRequested()
  signal serverSettingsRequested()
  signal createRoomRequested()
  readonly property bool canAddChat: chat.paneCount < 8
  function addChat(id) { chat.addConversation(chat.activeKey, id) }
  readonly property var layout: bridge.workspaceLayout
  readonly property string dock: ["left", "right", "top", "bottom"].indexOf(layout.dock) >= 0 ? layout.dock : "left"
  readonly property bool drawerMode: (theme.comfortable || theme.refinedTui) && width < theme.space(720)
  property bool drawerOpen: false
  function toggleDrawer() { drawerOpen = !drawerOpen }
  onDrawerModeChanged: drawerOpen = false
  readonly property bool stacked: !drawerMode && (width < theme.space(600) || dock === "top" || dock === "bottom")
  readonly property bool reversed: dock === "right" || dock === "bottom"
  readonly property bool collapsed: drawerMode ? !drawerOpen : layout.activityCollapsed
  readonly property real handleSize: theme.space(theme.cleanTui ? 6 : 10)
  readonly property real dividerSize: collapsed || drawerMode ? 0 : handleSize
  readonly property real available: Math.max(1, (stacked ? height : width) - dividerSize)
  readonly property real activitySize: {
    if (drawerMode) return Math.min(theme.space(340), width - theme.space(32))
    if (collapsed) return 0
    if ((theme.cleanTui || theme.comfortable || theme.refinedTui) && !stacked) {
      var cleanMaximum = Math.max(1, Math.min(theme.space(360), available - theme.space(320)))
      var cleanRequested = available * layout.bounded(layout.activityRatio, 0.25) * 0.72
      return Math.min(cleanMaximum, Math.max(theme.space(theme.comfortable ? 310 : 220), cleanRequested))
    }
    return Math.max(
      Math.min(available * 0.3, theme.space(stacked ? 70 : theme.tui ? 220 : 180)),
      Math.min(available - theme.space(stacked ? 230 : 250), available * layout.bounded(layout.activityRatio, 0.25)))
  }

  Shortcut { sequence: "Escape"; enabled: root.drawerMode && root.drawerOpen; onActivated: root.drawerOpen = false }
  Connections {
    target: root.bridge
    function onActiveConversationIdChanged() { root.drawerOpen = false }
  }
  Rectangle {
    anchors.fill: parent; z: 1
    visible: root.drawerMode && root.drawerOpen
    color: root.theme.alpha("#000000", 0.45)
    MouseArea { anchors.fill: parent; onClicked: root.drawerOpen = false }
  }
  Component {
    id: callControls
    CurrentCallBar {
      bridge: root.bridge; theme: root.theme; roomInvitesInHeader: !root.drawerMode || root.drawerOpen
      onCameraRequested: root.cameraRequested()
    }
  }
  Item {
    id: activity
    objectName: "activityPane"
    z: root.drawerMode ? 2 : 0
    visible: !root.collapsed
    clip: true
    x: !root.drawerMode && !root.stacked && root.reversed ? root.width - width : 0
    y: root.stacked && root.reversed ? root.height - height : 0
    width: root.stacked ? root.width : root.activitySize
    height: root.stacked ? root.activitySize : root.height
    readonly property bool pinCall: (root.theme.comfortable || root.theme.refinedTui) && !root.stacked
    readonly property real callInset: pinnedCall.visible ? pinnedCall.height + root.theme.spacing.lg : 0
    readonly property real available: Math.max(1, height - root.handleSize - callInset)
    readonly property real minimumPane: root.theme.space(root.stacked ? 70 : 44)
    readonly property real frameInset: root.theme.tui || root.theme.comfortable ? root.theme.space(root.theme.cleanTui ? 10 : 8) : 0
    readonly property real frameTop: root.theme.tui || root.theme.comfortable ? root.theme.space(22) : 0
    readonly property real roomsSize: Math.max(Math.min(minimumPane, available / 2), Math.min(available - Math.min(minimumPane, available / 2), root.layout.roomsRatio <= 0 ? Math.min(available * 0.55, roomColumn.implicitHeight + frameTop + frameInset) : available * root.layout.bounded(root.layout.roomsRatio, 0.28)))
    Rectangle { anchors.fill: parent; color: root.theme.comfortable ? root.theme.surface : "transparent"; radius: root.theme.cornerRadius }
    Flickable {
      id: rooms
      objectName: "roomsPane"
      x: activity.frameInset; y: activity.frameTop
      width: parent.width - activity.frameInset * 2; height: Math.max(1, activity.roomsSize - activity.frameTop - activity.frameInset)
      contentWidth: width; contentHeight: roomColumn.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      Column {
        id: roomColumn; width: parent.width; spacing: root.theme.spacing.xs
        ServerSelector {
          width: parent.width; bridge: root.bridge; theme: root.theme; compact: true
          onSettingsRequested: root.serverSettingsRequested()
        }
        Repeater {
          model: root.bridge.knocks
          KnockCard { required property var modelData; width: roomColumn.width; knock: modelData; bridge: root.bridge; theme: root.theme }
        }
        RoomsHeader {
          width: parent.width; bridge: root.bridge; theme: root.theme
          onCreateRequested: root.createRoomRequested()
        }
        SpotsView { width: parent.width; bridge: root.bridge; theme: root.theme; mainApp: true }
        Loader {
          active: !activity.pinCall
          visible: active && !!item && item.inCall
          width: parent.width; height: visible ? item.implicitHeight : 0
          sourceComponent: callControls
        }
        ServerChannelsView { width: parent.width; bridge: root.bridge; theme: root.theme; showHeader: true }

      }
    }
    Loader {
      id: pinnedCall
      active: activity.pinCall
      visible: active && !!item && item.inCall
      x: activity.frameInset; y: activity.height - height
      width: activity.width - activity.frameInset * 2
      height: visible ? item.implicitHeight : 0
      z: 2
      sourceComponent: callControls
    }
    ResizeHandle {
      objectName: "roomsResizeHandle"
      theme: root.theme; verticalLine: false
      y: activity.roomsSize; width: parent.width; height: root.handleSize
      onMoved: function(delta) { root.layout.roomsRatio = root.layout.bounded((activity.roomsSize + delta) / activity.available, 0.28) }
      onResetRequested: root.layout.roomsRatio = 0
    }
    Flickable {
      id: friendsPane
      objectName: "friendsPane"
      x: activity.frameInset
      y: activity.roomsSize + root.handleSize + activity.frameTop; width: parent.width - activity.frameInset * 2; height: Math.max(0, parent.height - y - activity.frameInset - activity.callInset)
      contentWidth: width; contentHeight: friends.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      FriendsView { id: friends; showHeader: !root.theme.tui && !root.theme.comfortable; width: parent.width; bridge: root.bridge; theme: root.theme }
    }
    TerminalFrame {
      width: parent.width; height: activity.roomsSize
      theme: root.theme; title: root.theme.comfortable ? "Server" : root.theme.cleanTui ? "01 /server" : "01: /server"; ink: root.theme.roomSectionColor
    }
    TerminalFrame {
      y: activity.roomsSize + root.handleSize
      width: parent.width; height: Math.max(0, parent.height - y - activity.callInset)
      theme: root.theme; title: root.theme.comfortable ? "Friends" : root.theme.cleanTui ? "02 /friends" : "02: /friends"; ink: root.theme.friendSectionColor
    }
  }
  ResizeHandle {
    id: activityDivider
    objectName: "activityResizeHandle"
    visible: !root.collapsed && !root.drawerMode
    theme: root.theme; verticalLine: !root.stacked
    x: root.stacked ? 0 : (root.reversed ? chat.width : activity.width) + (root.dividerSize - width) / 2
    y: root.stacked ? (root.reversed ? chat.height : activity.height) + (root.dividerSize - height) / 2 : 0
    width: root.stacked ? root.width : root.handleSize
    height: root.stacked ? root.handleSize : root.height
    onMoved: function(delta) {
      var ratio = (root.activitySize + delta * (root.reversed ? -1 : 1)) / root.available
      root.layout.activityRatio = root.layout.bounded((root.theme.cleanTui || root.theme.comfortable) && !root.stacked ? ratio / 0.72 : ratio, 0.25)
    }
    onResetRequested: root.layout.activityRatio = 0.25
  }
  Loader {
    id: narrowCall
    active: root.drawerMode && !root.drawerOpen
    visible: active && !!item && item.inCall
    anchors.bottom: parent.bottom
    width: parent.width; height: visible ? item.implicitHeight : 0
    sourceComponent: callControls
  }
  TiledConversations {
    id: chat
    objectName: "conversationPane"
    enabled: !root.drawerMode || !root.drawerOpen
    x: !root.drawerMode && !root.stacked && !root.reversed ? activity.width + root.dividerSize : 0
    y: root.stacked && !root.reversed ? activity.height + root.dividerSize : 0
    width: root.drawerMode || root.stacked ? root.width : root.available - root.activitySize
    height: (root.stacked ? root.available - root.activitySize : root.height) - (narrowCall.visible ? narrowCall.height + root.theme.spacing.sm : 0)
    bridge: root.bridge; theme: root.theme; activityStacked: root.stacked
    onRevealMainRequested: root.revealMainRequested()
  }
}
