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
  readonly property bool stacked: (width < theme.space(600) && !(layout.activityWidth > 0)) || dock === "top" || dock === "bottom"
  readonly property bool reversed: dock === "right" || dock === "bottom"
  readonly property bool collapsed: layout.activityCollapsed
  readonly property real handleSize: theme.space(theme.cleanTui ? 6 : 10)
  readonly property real dividerSize: collapsed ? 0 : handleSize
  readonly property real available: Math.max(1, (stacked ? height : width) - dividerSize)
  readonly property real maximumActivityHeight: Math.max(1, available - Math.min(theme.space(200), available / 2))
  readonly property real minimumActivityHeight: Math.min(maximumActivityHeight, theme.space(180))
  function boundedActivityHeight(value) { return Math.max(minimumActivityHeight, Math.min(maximumActivityHeight, value)) }
  readonly property real activitySize: {
    if (collapsed) return 0
    if (stacked) return boundedActivityHeight(layout.activityHeight > 0 ? theme.space(layout.activityHeight) : theme.space(300))
    if (isFinite(layout.activityWidth) && layout.activityWidth > 0)
      return Math.max(theme.space(24), Math.min(available-theme.space(200), theme.space(layout.activityWidth)))
    if (theme.cleanTui || theme.friendly) {
      var cleanMaximum = Math.max(1, Math.min(theme.space(360), available - theme.space(320)))
      var cleanRequested = available * layout.bounded(layout.activityRatio, 0.25) * (theme.friendly ? 1 : 0.72)
      return Math.min(cleanMaximum, Math.max(theme.space(theme.friendly ? 260 : 220), cleanRequested))
    }
    return Math.max(
      Math.min(available * 0.3, theme.space(theme.tui ? 220 : 180)),
      Math.min(available - theme.space(250), available * layout.bounded(layout.activityRatio, 0.25)))
  }

  Item {
    id: activity
    objectName: "activityPane"
    visible: !root.collapsed
    clip: true
    x: !root.stacked && root.reversed ? root.width - width : 0
    y: root.stacked && root.reversed ? root.height - height : 0
    width: root.stacked ? root.width : root.activitySize
    height: root.stacked ? root.activitySize : root.height
    Rectangle { anchors.fill: parent; visible: root.theme.friendly; color: root.theme.sidebar; radius: root.theme.cornerRadius }
    readonly property real available: Math.max(1, (root.stacked ? width : height) - root.handleSize)
    readonly property real listsHeight: root.stacked ? height - roomCallBar.height - (roomCallBar.visible ? root.theme.space(8) : 0) : height
    readonly property real minimumPane: root.theme.space(44)
    readonly property real frameInset: Math.min(Math.max(2,(width-root.theme.space(24))/10), root.theme.friendly ? root.theme.space(10) : root.theme.tui ? root.theme.space(root.theme.cleanTui ? 10 : 8) : 0)
    readonly property real frameTop: width < root.theme.space(100) ? root.theme.space(4) : root.theme.friendly ? root.theme.space(12) : root.theme.tui ? root.theme.space(22) : 0
    readonly property real roomsSize: {
      if (root.stacked) {
        var minimumColumn = Math.min(available / 2, root.theme.space(200))
        return Math.max(minimumColumn, Math.min(available - minimumColumn, available * root.layout.bounded(root.layout.activityColumnsRatio, 0.58)))
      }
      var callSpace = roomCallBar.height + (roomCallBar.visible ? root.theme.space(8) : 0)
      var minimumRooms = Math.min(available/2, minimumPane + callSpace + frameTop + frameInset)
      var minimumFriends = Math.min(available/2, minimumPane)
      var preferred = root.layout.roomsRatio <= 0
        ? Math.min(available - Math.min(available*0.3, root.theme.space(160)), roomColumn.implicitHeight + callSpace + frameTop + frameInset)
        : available * root.layout.bounded(root.layout.roomsRatio, 0.28)
      return Math.max(minimumRooms, Math.min(available-minimumFriends, preferred))
    }
    Flickable {
      id: rooms
      objectName: "roomsPane"
      x: activity.frameInset; y: activity.frameTop
      width: (root.stacked ? activity.roomsSize : parent.width) - activity.frameInset * 2
      height: Math.max(1, (root.stacked ? activity.listsHeight : activity.roomsSize - roomCallBar.height - (roomCallBar.visible ? root.theme.space(8) : 0)) - activity.frameTop - activity.frameInset)
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
    CurrentCallBar {
          id: roomCallBar
          x: activity.frameInset; y: root.stacked ? parent.height - height - activity.frameInset : rooms.y + rooms.height + (visible ? root.theme.space(8) : 0)
          width: root.stacked ? parent.width - activity.frameInset * 2 : rooms.width; height: visible ? implicitHeight : 0
          bridge: root.bridge; theme: root.theme
          maximumHeight: Math.min(root.theme.space(210), activity.height/2)
          compact: true; adaptive: true; horizontal: root.stacked
          roomInvitesInHeader: true
          onCameraRequested: root.cameraRequested()
        }
    ResizeHandle {
      objectName: "roomsResizeHandle"
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
      x: (root.stacked ? activity.roomsSize + root.handleSize : 0) + activity.frameInset
      y: (root.stacked ? 0 : activity.roomsSize + root.handleSize) + activity.frameTop
      width: parent.width - x - activity.frameInset; height: Math.max(0, activity.listsHeight - y - activity.frameInset)
      contentWidth: width; contentHeight: friends.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      FriendsView { id: friends; showHeader: !root.theme.tui; width: parent.width; bridge: root.bridge; theme: root.theme; adaptive: true }
    }
    TerminalFrame {
      visible: root.theme.tui && activity.width >= root.theme.space(100)
      width: root.stacked ? activity.roomsSize : parent.width; height: root.stacked ? activity.listsHeight : activity.roomsSize
      theme: root.theme; title: root.theme.cleanTui ? "01 /server" : "01: /server"; ink: root.theme.roomSectionColor
    }
    TerminalFrame {
      visible: root.theme.tui && activity.width >= root.theme.space(100)
      x: root.stacked ? activity.roomsSize + root.handleSize : 0; y: root.stacked ? 0 : activity.roomsSize + root.handleSize
      width: parent.width - x; height: activity.listsHeight - y
      theme: root.theme; title: root.theme.cleanTui ? "02 /friends" : "02: /friends"; ink: root.theme.friendSectionColor
    }
  }
  ResizeHandle {
    id: activityDivider
    objectName: "activityResizeHandle"
    visible: !root.collapsed
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
  TiledConversations {
    id: chat
    objectName: "conversationPane"
    x: !root.stacked && !root.reversed ? activity.width + root.dividerSize : 0
    y: root.stacked && !root.reversed ? activity.height + root.dividerSize : 0
    width: root.stacked ? root.width : root.available - root.activitySize
    height: root.stacked ? root.available - root.activitySize : root.height
    bridge: root.bridge; theme: root.theme; activityStacked: root.stacked
    onRevealMainRequested: root.revealMainRequested()
  }
}
