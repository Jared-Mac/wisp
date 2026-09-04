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
  readonly property bool canAddChat: chat.paneCount < 8
  function addChat(id) { chat.addConversation(chat.activeKey, id) }
  readonly property var layout: bridge.workspaceLayout
  readonly property string dock: ["left", "right", "top", "bottom"].indexOf(layout.dock) >= 0 ? layout.dock : "left"
  readonly property bool stacked: width < theme.space(600) || dock === "top" || dock === "bottom"
  readonly property bool reversed: dock === "right" || dock === "bottom"
  readonly property bool collapsed: layout.activityCollapsed
  readonly property real handleSize: theme.space(10)
  readonly property real dividerSize: collapsed ? 0 : handleSize
  readonly property real available: Math.max(1, (stacked ? height : width) - dividerSize)
  readonly property real activitySize: collapsed ? 0 : Math.max(Math.min(available * 0.3, theme.space(stacked ? 70 : theme.tui ? 220 : 180)), Math.min(available - theme.space(stacked ? 230 : 250), available * layout.bounded(layout.activityRatio, 0.25)))

  Item {
    id: activity
    objectName: "activityPane"
    visible: !root.collapsed
    clip: true
    x: !root.stacked && root.reversed ? root.width - width : 0
    y: root.stacked && root.reversed ? root.height - height : 0
    width: root.stacked ? root.width : root.activitySize
    height: root.stacked ? root.activitySize : root.height
    readonly property real available: Math.max(1, height - root.handleSize)
    readonly property real minimumPane: root.theme.space(root.stacked ? 70 : 44)
    readonly property real frameInset: root.theme.tui ? root.theme.space(8) : 0
    readonly property real frameTop: root.theme.tui ? root.theme.space(22) : 0
    readonly property real roomsSize: Math.max(Math.min(minimumPane, available / 2), Math.min(available - Math.min(minimumPane, available / 2), root.layout.roomsRatio <= 0 ? Math.min(available * 0.55, roomColumn.implicitHeight + frameTop + frameInset) : available * root.layout.bounded(root.layout.roomsRatio, 0.28)))
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
        Repeater {
          model: root.bridge.knocks
          KnockCard { required property var modelData; width: roomColumn.width; knock: modelData; bridge: root.bridge; theme: root.theme }
        }
        NowView { width: parent.width; showHeader: !root.theme.tui; bridge: root.bridge; theme: root.theme; onCameraRequested: root.cameraRequested() }
        SpotsView { width: parent.width; bridge: root.bridge; theme: root.theme }
        Text {
          visible: root.theme.tui && !root.bridge.hangouts.length && !root.bridge.spots.length
          width: parent.width; wrapMode: Text.Wrap
          text: "(no rooms available)"
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
      }
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
      y: activity.roomsSize + root.handleSize + activity.frameTop; width: parent.width - activity.frameInset * 2; height: Math.max(0, parent.height - y - activity.frameInset)
      contentWidth: width; contentHeight: friends.implicitHeight
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      FriendsView { id: friends; showHeader: !root.theme.tui; width: parent.width; bridge: root.bridge; theme: root.theme }
    }
    TerminalFrame {
      width: parent.width; height: activity.roomsSize
      theme: root.theme; title: "01: /rooms"; ink: root.theme.warning
    }
    TerminalFrame {
      y: activity.roomsSize + root.handleSize
      width: parent.width; height: parent.height - y
      theme: root.theme; title: "02: /friends"; ink: root.theme.secondaryAccent
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
    onMoved: function(delta) { root.layout.activityRatio = root.layout.bounded((root.activitySize + delta * (root.reversed ? -1 : 1)) / root.available, 0.25) }
    onResetRequested: root.layout.activityRatio = 0.25
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
