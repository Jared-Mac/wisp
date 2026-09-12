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
  readonly property bool friendsMode: bridge.serverMember === false || bridge.friendPreferences.friendsViewFor("app")
  readonly property string dock: ["left", "right", "top", "bottom"].indexOf(layout.dock) >= 0 ? layout.dock : "left"
  readonly property bool drawerMode: (theme.comfortable || theme.refinedTui) && width < theme.space(720)
    && !(layout.activityWidth > 0) && (dock === "left" || dock === "right")
  property bool drawerOpen: false
  function toggleDrawer() { drawerOpen = !drawerOpen }
  onDrawerModeChanged: drawerOpen = false
  Shortcut { sequence: "Escape"; enabled: root.drawerMode && root.drawerOpen; onActivated: root.drawerOpen = false }
  Connections { target: root.bridge; function onActiveConversationIdChanged() { root.drawerOpen = false } }
  readonly property bool stacked: !drawerMode && ((width < theme.space(600) && !(layout.activityWidth > 0)) || dock === "top" || dock === "bottom")
  readonly property bool reversed: dock === "right" || dock === "bottom"
  readonly property bool collapsed: drawerMode ? !drawerOpen : layout.activityCollapsed
  readonly property bool audioInSidebar: !collapsed
  readonly property real handleSize: theme.space(theme.cleanTui ? 6 : 10)
  readonly property real dividerSize: collapsed || drawerMode ? 0 : handleSize
  readonly property real available: Math.max(1, (stacked ? height : width) - dividerSize)
  readonly property real maximumActivityHeight: Math.max(1, available - Math.min(theme.space(200), available / 2))
  readonly property real minimumActivityHeight: Math.min(maximumActivityHeight, theme.space(180))
  function boundedActivityHeight(value) { return Math.max(minimumActivityHeight, Math.min(maximumActivityHeight, value)) }
  readonly property real activitySize: {
    if (drawerMode) return Math.min(theme.space(340), width - theme.space(32))
    if (collapsed) return 0
    if (stacked) return boundedActivityHeight(layout.activityHeight > 0 ? theme.space(layout.activityHeight) : theme.space(300))
    if (isFinite(layout.activityWidth) && layout.activityWidth > 0)
      return Math.max(theme.space(24), Math.min(available-theme.space(200), theme.space(layout.activityWidth)))
    if (theme.cleanTui || theme.friendly || theme.comfortable || theme.refinedTui) {
      var cleanMaximum = Math.max(1, Math.min(theme.space(360), available - theme.space(320)))
      var cleanRequested = available * layout.bounded(layout.activityRatio, 0.25) * (theme.friendly ? 1 : 0.72)
      return Math.min(cleanMaximum, Math.max(theme.space(theme.comfortable ? 310 : theme.friendly ? 260 : 220), cleanRequested))
    }
    return Math.max(
      Math.min(available * 0.3, theme.space(theme.tui ? 220 : 180)),
      Math.min(available - theme.space(250), available * layout.bounded(layout.activityRatio, 0.25)))
  }

  Rectangle {
    anchors.fill: parent; z: 1
    visible: root.drawerMode && root.drawerOpen; color: root.theme.alpha("#000000", 0.45)
    MouseArea { anchors.fill: parent; onClicked: root.drawerOpen = false }
  }
  Item {
    id: activity
    z: root.drawerMode ? 2 : 0
    objectName: "activityPane"
    visible: !root.collapsed
    clip: true
    x: !root.drawerMode && !root.stacked && root.reversed ? root.width - width : 0
    y: root.stacked && root.reversed ? root.height - height : 0
    width: root.stacked ? root.width : root.activitySize
    height: root.stacked ? root.activitySize : root.height
    Rectangle { anchors.fill: parent; visible: root.theme.friendly || root.theme.comfortable; color: root.theme.comfortable ? root.theme.surface : root.theme.sidebar; radius: root.theme.cornerRadius }
    readonly property real usableHeight: Math.max(0,height-audioFooter.height-frameInset-root.theme.spacing.sm)
    readonly property real available: Math.max(1, (root.stacked ? width : usableHeight) - root.handleSize)
    readonly property real listsHeight: usableHeight
    readonly property real minimumPane: root.theme.space(44)
    readonly property real frameInset: Math.min(Math.max(2,(width-root.theme.space(24))/10), root.theme.friendly ? root.theme.space(10) : (root.theme.tui || root.theme.comfortable) ? root.theme.space(root.theme.cleanTui ? 10 : 8) : 0)
    readonly property real frameTop: width < root.theme.space(100) ? root.theme.space(4) : root.theme.friendly ? root.theme.space(12) : (root.theme.tui || root.theme.comfortable) ? root.theme.space(22) : 0
    readonly property real roomsSize: {
      if (root.stacked) {
        var minimumColumn = Math.min(available / 2, root.theme.space(200))
        return Math.max(minimumColumn, Math.min(available - minimumColumn, available * root.layout.bounded(root.layout.activityColumnsRatio, 0.58)))
      }
      var minimumRooms = Math.min(available/2, minimumPane + frameTop + frameInset)
      var minimumFriends = Math.min(available/2, minimumPane)
      var preferred = root.layout.roomsRatio <= 0
        ? Math.min(available - Math.min(available*0.3, root.theme.space(160)), roomColumn.implicitHeight + frameTop + frameInset)
        : available * root.layout.bounded(root.layout.roomsRatio, 0.28)
      return Math.max(minimumRooms, Math.min(available-minimumFriends, preferred))
    }
    Flickable {
      id: rooms
      objectName: "roomsPane"
      visible: !root.friendsMode
      x: activity.frameInset; y: activity.frameTop
      width: (root.stacked ? activity.roomsSize : parent.width) - activity.frameInset * 2
      height: Math.max(1, (root.stacked ? activity.listsHeight : activity.roomsSize) - activity.frameTop - activity.frameInset)
      contentWidth: width; contentHeight: roomColumn.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      Column {
      id: roomColumn; width: parent.width; spacing: root.theme.friendly ? root.theme.space(8) : root.theme.spacing.xs
        ServerSelector {
          width: parent.width; bridge: root.bridge; theme: root.theme; compact: true; adaptive: true; horizontal: root.stacked
          onSettingsRequested: root.serverSettingsRequested()
        }
        Repeater {
          model: root.bridge.knocks
          KnockCard { required property var modelData; width: roomColumn.width; knock: modelData; bridge: root.bridge; theme: root.theme; adaptive: true }
        }
        RoomsHeader {
          width: parent.width; bridge: root.bridge; theme: root.theme
          adaptive: true
          onCreateRequested: root.createRoomRequested()
        }
        SpotsView { width: parent.width; bridge: root.bridge; theme: root.theme; mainApp: true; adaptive: true; horizontal: root.stacked }
        ServerChannelsView { width: parent.width; bridge: root.bridge; theme: root.theme; showHeader: true; adaptive: true }

      }
    }
    ResizeHandle {
      objectName: "roomsResizeHandle"
      visible: !root.friendsMode
      theme: root.theme; verticalLine: root.stacked
      x: root.stacked ? activity.roomsSize : 0; y: root.stacked ? 0 : activity.roomsSize
      width: root.stacked ? root.handleSize : parent.width; height: root.stacked ? activity.listsHeight : root.handleSize
      onMoved: function(delta) {
        if (root.stacked) root.layout.activityColumnsRatio = root.layout.bounded((activity.roomsSize + delta) / activity.available, 0.58)
        else root.layout.roomsRatio = root.layout.bounded((activity.roomsSize + delta) / activity.available, 0.28)
      }
      onResetRequested: { if (root.stacked) root.layout.activityColumnsRatio = 0.58; else root.layout.roomsRatio = 0 }
    }
    Flickable {
      id: friendsPane
      objectName: "friendsPane"
      x: (!root.friendsMode && root.stacked ? activity.roomsSize + root.handleSize : 0) + activity.frameInset
      y: (!root.friendsMode && !root.stacked ? activity.roomsSize + root.handleSize : 0) + activity.frameTop
      width: parent.width - x - activity.frameInset; height: Math.max(0, activity.listsHeight - y - activity.frameInset)
      contentWidth: width; contentHeight: peopleColumn.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      Column {
        id: peopleColumn; width: parent.width; spacing: root.theme.spacing.sm
        PeopleView { width: parent.width; bridge: root.bridge; theme: root.theme }
      }
    }
    TerminalFrame {
      visible: !root.friendsMode && (root.theme.tui || root.theme.comfortable) && activity.width >= root.theme.space(100)
      width: root.stacked ? activity.roomsSize : parent.width; height: root.stacked ? activity.listsHeight : activity.roomsSize
      theme: root.theme; title: root.theme.comfortable ? "Server" : root.theme.cleanTui ? "01 /server" : "01: /server"; ink: root.theme.roomSectionColor
    }
    TerminalFrame {
      visible: (root.theme.tui || root.theme.comfortable) && activity.width >= root.theme.space(100)
      x: !root.friendsMode && root.stacked ? activity.roomsSize + root.handleSize : 0; y: !root.friendsMode && !root.stacked ? activity.roomsSize + root.handleSize : 0
      width: parent.width - x; height: activity.listsHeight - y
      theme: root.theme; title: root.theme.comfortable ? "Friends" : root.theme.cleanTui ? "02 /friends" : "02: /friends"; ink: root.theme.friendSectionColor
    }
    SidebarAudioControls {
      id: audioFooter
      x: activity.frameInset; y: parent.height-height-activity.frameInset
      width: Math.max(1,parent.width-activity.frameInset*2)
      bridge: root.bridge; theme: root.theme
      horizontal: root.stacked
      maximumCallHeight: Math.min(root.theme.space(280), activity.height/2)
      roomInvitesInHeader: !root.drawerMode || root.drawerOpen
      onCameraRequested: root.cameraRequested()
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
      if (!root.stacked) {
        var requested = Math.max(root.theme.space(24), Math.min(root.available-root.theme.space(200), root.activitySize + delta * (root.reversed ? -1 : 1)))
        root.layout.activityWidth = requested / root.theme.spacingScale
        return
      }
      root.layout.activityHeight = root.boundedActivityHeight(root.activitySize + delta * (root.reversed ? -1 : 1)) / root.theme.spacingScale
    }
    onResetRequested: { if (root.stacked) root.layout.activityHeight = 0; else { root.layout.activityWidth = 0; root.layout.activityRatio = 0.25 } }
  }
  Loader {
    id: narrowCall
    active: root.drawerMode && !root.drawerOpen
    visible: active && !!item && item.inCall
    anchors.bottom: parent.bottom; width: parent.width
    height: visible ? item.implicitHeight : 0
    sourceComponent: CurrentCallBar {
      bridge: root.bridge; theme: root.theme; roomInvitesInHeader: false; showAudio: false
      onCameraRequested: root.cameraRequested()
    }
  }
  TiledConversations {
    id: chat
    enabled: !root.drawerMode || !root.drawerOpen
    objectName: "conversationPane"
    x: !root.drawerMode && !root.stacked && !root.reversed ? activity.width + root.dividerSize : 0
    y: root.stacked && !root.reversed ? activity.height + root.dividerSize : 0
    width: root.drawerMode || root.stacked ? root.width : root.available - root.activitySize
    height: (root.stacked ? root.available - root.activitySize : root.height) - (narrowCall.visible ? narrowCall.height + root.theme.spacing.sm : 0)
    bridge: root.bridge; theme: root.theme; activityStacked: root.stacked
    onRevealMainRequested: root.revealMainRequested()
  }
}
