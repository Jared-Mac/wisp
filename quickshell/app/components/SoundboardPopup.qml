import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "soundboardPopup"
  required property var bridge
  required property var theme
  required property Item hostItem
  readonly property var board: bridge.soundboard
  readonly property string serverId: bridge.currentVoiceRoom ? bridge.voiceServerId : String((bridge.activeServer || {}).id || bridge.voiceServerId)
  readonly property var sounds: board.catalogs[serverId] || []
  readonly property bool canPlay: !!bridge.currentVoiceRoom && bridge.voiceServerId === serverId
    && !bridge.effectiveMuted && !bridge.selfState.deafened && !(bridge.ownModeration || {}).deafened
  readonly property string callKey: bridge.currentVoiceRoom ? bridge.voiceServerId + ":" + bridge.currentVoiceRoom.id : ""
  property point cursorAnchor: Qt.point(0, 0)

  function openAt(item, point) {
    cursorAnchor = item.mapToItem(parent, point.x, point.y)
    open()
  }
  onOpened: { board.refresh(serverId, false); board.syncPlayback() }
  onServerIdChanged: close()
  onCallKeyChanged: close()
  Connections {
    target: root.board
    function onRevisionChanged() { if (root.visible) root.board.refresh(root.serverId, false) }
  }
  // Resolve the overlay through the persistent host when Quickshell recreates
  // the bar panel's backing window after closing it.
  parent: hostItem.Overlay.overlay || hostItem
  padding: theme.space(6)
  width: Math.min(theme.space(sounds.length === 1 ? 132 : 258), parent ? parent.width-theme.space(12) : theme.space(258))
  height: Math.min((sounds.length ? pads.implicitHeight : empty.implicitHeight) + padding*2,
    parent ? parent.height-theme.space(12) : theme.space(260))
  x: Math.max(theme.space(6), Math.min(cursorAnchor.x + theme.space(6), (parent ? parent.width : width)-width-theme.space(6)))
  y: Math.max(theme.space(6), Math.min(cursorAnchor.y + theme.space(6), (parent ? parent.height : height)-height-theme.space(6)))
  modal: true
  dim: false
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.separator }
  contentItem: Item {
    implicitHeight: root.sounds.length ? pads.implicitHeight : empty.implicitHeight
    Text {
      id: empty
      anchors.fill: parent
      visible: !root.sounds.length
      text: root.board.loading[root.serverId] ? "Loading sounds…" : "No sounds — add them in Settings."
      wrapMode: Text.Wrap; textFormat: Text.PlainText
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    GridView {
      id: pads
      objectName: "soundboardPads"
      anchors.fill: parent
      visible: root.sounds.length > 0
      readonly property int columns: root.sounds.length === 1 ? 1 : 2
      cellWidth: width / columns
      cellHeight: root.theme.space(34)
      implicitHeight: Math.min(Math.ceil(count/columns)*cellHeight, root.theme.space(238))
      model: root.sounds
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      function focusPad(index) {
        currentIndex = Math.max(0, Math.min(count-1, index))
        positionViewAtIndex(currentIndex, GridView.Contain)
        if (currentItem) currentItem.forceActiveFocus()
      }
      delegate: ChatButton {
        id: pad
        required property var modelData
        required property int index
        objectName: "soundboardQuickPlay-" + modelData.id
        width: pads.cellWidth-root.theme.space(4); height: pads.cellHeight-root.theme.space(4)
        theme: root.theme; text: modelData.name; formatLabel: false
        enabled: root.canPlay && !root.board.pending && root.bridge.daemonConnected
        Accessible.name: "Play " + modelData.name + " into voice"
        ToolTip.visible: hovered || visualFocus
        ToolTip.text: !root.canPlay ? "Join voice and unmute to play sounds." : modelData.name + " · " + (modelData.duration_ms/1000).toFixed(1) + "s"
        onClicked: root.board.play(root.serverId, modelData.id, false)
        Keys.onLeftPressed: pads.focusPad(index-1)
        Keys.onRightPressed: pads.focusPad(index+1)
        Keys.onUpPressed: pads.focusPad(index-pads.columns)
        Keys.onDownPressed: pads.focusPad(index+pads.columns)
      }
    }
  }
}
