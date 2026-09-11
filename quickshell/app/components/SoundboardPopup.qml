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
  readonly property bool canPreview: !bridge.selfState.deafened && !(bridge.ownModeration || {}).deafened && (!bridge.currentVoiceRoom || bridge.effectiveMuted)
  readonly property string callKey: bridge.currentVoiceRoom ? bridge.voiceServerId + ":" + bridge.currentVoiceRoom.id : ""
  property point cursorAnchor: Qt.point(0, 0)
  readonly property real fullGridHeight:Math.ceil(sounds.length/(sounds.length===1 ? 1 : 2))*theme.space(34)
  readonly property real gridLimit:Math.max(theme.space(34),Math.min(theme.space(238),parent ? parent.height-padding*2-theme.space(32) : theme.space(238)))
  readonly property bool overflowing:fullGridHeight>gridLimit

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
  height: Math.min((sounds.length ? Math.min(fullGridHeight,gridLimit)+(overflowing ? theme.space(20) : 0) : empty.implicitHeight) + padding*2,
    parent ? parent.height-theme.space(12) : theme.space(260))
  x: Math.max(theme.space(6), Math.min(cursorAnchor.x + theme.space(6), (parent ? parent.width : width)-width-theme.space(6)))
  y: Math.max(theme.space(6), Math.min(cursorAnchor.y + theme.space(6), (parent ? parent.height : height)-height-theme.space(6)))
  modal: true
  dim: false
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.separator }
  contentItem: Item {
    implicitHeight: root.sounds.length ? pads.implicitHeight+overflowHint.height : empty.implicitHeight
    Text {
      id: empty
      anchors.fill:parent
      visible: !root.sounds.length
      text: root.board.loading[root.serverId] ? "Loading sounds…" : "No sounds — add them in Settings."
      wrapMode: Text.Wrap; textFormat: Text.PlainText
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    GridView {
      id: pads
      objectName: "soundboardPads"
      anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.bottom:overflowHint.top
      visible: root.sounds.length > 0
      readonly property int columns: root.sounds.length === 1 ? 1 : 2
      cellWidth: (width-(scrollbar.visible ? root.theme.space(10) : 0)) / columns
      cellHeight: root.theme.space(34)
      implicitHeight: Math.min(Math.ceil(count/columns)*cellHeight, root.theme.space(238))
      model: root.sounds
      clip: true; boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {id:scrollbar;objectName:"soundboardScrollBar";policy:root.overflowing ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff}
      function focusPad(index) {
        currentIndex = Math.max(0, Math.min(count-1, index))
        positionViewAtIndex(currentIndex, GridView.Contain)
        if (currentItem) currentItem.focusPlay()
      }
      delegate: Item {
        id: padContainer
        required property var modelData
        required property int index
        width: pads.cellWidth-root.theme.space(4); height: pads.cellHeight-root.theme.space(4)
        HoverHandler {id:padHover}
        ChatButton {
        id:pad;objectName: "soundboardQuickPlay-" + padContainer.modelData.id
        anchors.fill:parent
        theme: root.theme; text: padContainer.modelData.name; formatLabel: false
        leftPadding:preview.opacity>0 ? preview.width+root.theme.space(4) : root.theme.space(4)
        enabled: root.canPlay && !root.board.pending && root.bridge.daemonConnected
        Accessible.name: "Play " + padContainer.modelData.name + " into voice"
        ToolTip.visible: hovered || visualFocus
        ToolTip.text: !root.canPlay ? "Join voice and unmute to play sounds." : padContainer.modelData.name + " · " + (padContainer.modelData.duration_ms/1000).toFixed(1) + "s"
        onClicked: root.board.play(root.serverId, padContainer.modelData.id, false)
        Keys.onLeftPressed: pads.focusPad(padContainer.index-1)
        Keys.onRightPressed: pads.focusPad(padContainer.index+1)
        Keys.onUpPressed: pads.focusPad(padContainer.index-pads.columns)
        Keys.onDownPressed: pads.focusPad(padContainer.index+pads.columns)
        }
        function focusPlay(){pad.forceActiveFocus()}
        ChatButton {
          id:preview;objectName:"soundboardQuickPreview-"+padContainer.modelData.id
          anchors.left:parent.left;anchors.leftMargin:root.theme.space(2);anchors.verticalCenter:parent.verticalCenter
          width:root.theme.space(24);height:width;theme:root.theme;text:"Listen locally";iconName:"volume";iconOnly:true;forceIcon:true
          opacity:padHover.hovered || pad.visualFocus || visualFocus ? 1 : 0
          enabled:root.canPreview && !root.board.pending && root.bridge.daemonConnected
          Accessible.name:"Listen to "+padContainer.modelData.name+" privately"
          ToolTip.visible:hovered || visualFocus
          ToolTip.text:root.canPreview ? "Listen only on this device" : root.bridge.selfState.deafened || (root.bridge.ownModeration || {}).deafened ? "Undeafen to listen" : "Mute your microphone to preview privately"
          onClicked:root.board.play(root.serverId,padContainer.modelData.id,true)
        }
      }
    }
    ChatButton {
      id:overflowHint;objectName:"soundboardOverflowHint"
      anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom
      visible:root.overflowing;height:visible ? root.theme.space(20) : 0
      theme:root.theme;quiet:true;text:pads.atYEnd ? "↑ Back to top" : "↓ More sounds"
      onClicked:pads.contentY=pads.atYEnd ? pads.originY : Math.min(pads.originY+pads.contentHeight-pads.height,pads.contentY+pads.height*0.75)
    }
  }
}
