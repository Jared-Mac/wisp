import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs as Dialogs

Column {
  id: root
  objectName: "settingsSoundboard"
  required property var bridge
  required property var theme
  required property string serverId
  property bool playOnly: false
  property bool previewMode: false
  property real maximumListHeight: theme.space(240)
  readonly property var filteredSounds: sounds.filter(function(s) { return s.name.toLowerCase().indexOf(search.text.toLowerCase().trim()) >= 0 })
  readonly property var board: bridge.soundboard
  readonly property var serverState: bridge.participantServer({server_id:serverId})
  readonly property var self: serverState.self || {}
  readonly property bool deafened: Boolean(bridge.selfState.deafened) || !!(bridge.ownModeration || {}).deafened
  readonly property bool inCall: !!bridge.currentVoiceRoom
  readonly property bool canPlay: inCall && bridge.voiceServerId === serverId && !bridge.effectiveMuted && !deafened
  readonly property bool canPreview: !deafened && (!inCall || bridge.effectiveMuted)
  readonly property var sounds: board.catalogs[serverId] || []
  property string selectedPath: ""
  property string pickerServer: ""
  property var removing: null
  spacing: theme.spacing.md
  onVisibleChanged: if (visible && board) {board.refresh(serverId, false);board.syncPlayback()}
  onServerIdChanged: {
    selectedPath=""; name.text=""; search.text=""; removing=null; removeDialog.close()
    if (visible && board) {board.refresh(serverId, false);board.syncPlayback()}
  }
  Component.onCompleted: if (visible && board) {board.refresh(serverId, false);board.syncPlayback()}
  Connections {
    target: root.board
    function onRevisionChanged() { if (root.visible) Qt.callLater(function(){root.board.refresh(root.serverId,false)}) }
    function onUploaded(server) { if (server === root.serverId) {root.selectedPath="";name.text=""} }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
    visible: !root.playOnly
    text: "Soundboard · " + String((root.serverState.server || {}).name || "This server")
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    visible: !root.playOnly
    text: "Shared with this server. Add a short sound, preview it, or play it into your voice room."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  ChatButton {
    objectName: "soundboardAddDefaults"
    visible: !root.playOnly || root.sounds.length === 0
    theme: root.theme; text: "Add starter sounds"
    enabled: root.bridge.daemonConnected && !root.board.busy[root.serverId] && !root.board.loading[root.serverId] && !!root.board.catalogs[root.serverId]
    onClicked: root.board.addDefaults(root.serverId)
  }
  Flow {
    visible: !root.playOnly
    width: parent.width; spacing: root.theme.spacing.sm
    ChatButton { theme: root.theme; text: root.selectedPath ? "Change file" : "Choose sound"; enabled: !root.board.busy[root.serverId]; onClicked: {root.pickerServer=root.serverId;filePicker.open()} }
    TextField {
      id: name; objectName: "soundboardName"; width: Math.min(root.width,root.theme.space(210)); maximumLength: 32
      placeholderText: "Sound name"; Accessible.name: "Sound name"
      ThemeControlStyle { theme: root.theme; control: name }
    }
    ChatButton {
      objectName: "soundboardUpload"; theme: root.theme; text: root.board.busy[root.serverId] ? "Saving…" : "Upload"
      enabled: root.bridge.daemonConnected && !!root.selectedPath && !!name.text.trim() && !root.board.busy[root.serverId] && root.sounds.length < 64
      onClicked: root.board.upload(root.serverId, name.text.trim(), root.selectedPath)
    }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
    visible: !root.playOnly
    text: (root.selectedPath ? decodeURIComponent(root.selectedPath.split("/").pop()) + " · " : "") + "WAV, MP3, Ogg or FLAC · up to 10 seconds / 20 MB · " + root.sounds.length + "/64 sounds"
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Row {
    visible: root.playOnly
    width: parent.width; spacing: root.theme.spacing.sm
    CheckBox {
      id: previewToggle; objectName: "soundboardPreviewMode"
      text: "Private preview"; checked: root.previewMode
      enabled: root.canPreview || checked
      onToggled: root.previewMode = checked
      ThemeControlStyle {theme:root.theme;control:previewToggle}
      ToolTip.visible: hovered
      ToolTip.text: root.canPreview ? "Only you hear the sound." : "Mute your microphone to preview privately."
    }
    Text {
      width: Math.max(0, parent.width-previewToggle.width-parent.spacing)
      anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight
      text: root.deafened ? "Undeafen to listen" : root.previewMode ? "Only you hear it" : root.canPlay ? "You and your call" : root.inCall ? "Unmute to play" : "Join voice to play"
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
  Row {
    width: parent.width; spacing: root.theme.spacing.sm
    Text { id: volumeLabel; anchors.verticalCenter: parent.verticalCenter; text: "Volume " + root.board.volume + "%"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
    Slider {
      id: volume; objectName: "soundboardVolume"; width: Math.max(20,parent.width-volumeLabel.width-stop.width-parent.spacing*2)
      from: 0; to: 100; stepSize: 1; value: root.board.volume; Accessible.name: "Soundboard volume"
      onMoved: root.board.volume=Math.round(value)
      ThemeControlStyle {theme:root.theme;control:volume}
    }
    ChatButton { id: stop; objectName: "soundboardStop"; theme: root.theme; text: "Stop"; destructive: true; enabled: root.board.pending || root.board.playing || root.board.previewing; onClicked: root.board.stop() }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    visible: !root.playOnly
    text: !root.inCall ? "Join this server's voice room to play for everyone. Preview stays on your device."
      : root.deafened ? "Undeafen to use the soundboard."
      : root.bridge.effectiveMuted ? "Private preview is available. Unmute to play into voice."
      : root.bridge.voiceServerId !== root.serverId ? "Your call is on another server. Open that server's soundboard to play."
      : "Play is heard by you and everyone in your call. Mute your mic to preview privately."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
    visible: root.playOnly || text !== ""
    height: root.playOnly ? Math.max(root.theme.space(18), implicitHeight) : implicitHeight
    text: {
      var feedback = root.board.feedback[root.serverId] || ""
      return root.playOnly && ["Playback finished.", "Stopped."].indexOf(feedback) >= 0 ? "" : feedback
    }
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Row {
    width: parent.width; spacing: root.theme.spacing.sm
    TextField { id: search; objectName: "soundboardSearch"; width: parent.width-refresh.width-parent.spacing; placeholderText: "Search sounds"; Accessible.name: "Search sounds"; ThemeControlStyle {theme:root.theme;control:search} }
    ChatButton { id: refresh; theme: root.theme; text: "Refresh"; iconName: "refresh"; iconOnly: root.playOnly; forceIcon: root.playOnly; Accessible.name: "Refresh sounds"; enabled: root.bridge.daemonConnected && !root.board.loading[root.serverId]; onClicked: root.board.refresh(root.serverId,true) }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; visible: root.filteredSounds.length === 0
    text: root.board.loading[root.serverId] ? "Loading sounds…" : !root.bridge.daemonConnected ? "Connect to Wisp to load this library." : root.sounds.length ? "No sounds match your search." : root.playOnly ? "No sounds yet. Add starter sounds or open Manage sounds to upload your own." : "No sounds yet. Upload the first one for this server."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  GridView {
    id: pads; objectName: "soundboardPads"
    visible: root.playOnly
    width: parent.width
    readonly property int columns: width >= root.theme.space(360) ? 3 : 2
    cellWidth: width / columns; cellHeight: root.theme.space(72)
    height: visible ? Math.min(Math.ceil(root.sounds.length/columns)*cellHeight, root.maximumListHeight) : 0
    model: root.playOnly ? root.filteredSounds : []
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {policy:parent.contentHeight>parent.height+1 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff}
    function focusPad(index) {
      currentIndex = Math.max(0, Math.min(count-1,index))
      positionViewAtIndex(currentIndex, GridView.Contain)
      if (currentItem) currentItem.forceActiveFocus()
    }
    delegate: ChatButton {
      id: pad
      required property var modelData
      required property int index
      objectName: "soundboardQuickPlay-" + modelData.id
      width: pads.cellWidth-root.theme.space(6); height: pads.cellHeight-root.theme.space(6)
      theme: root.theme; text: modelData.name; formatLabel: false
      Binding {target:pad.background;property:"border.width";value:1;when:root.theme.tui}
      enabled: (root.previewMode ? root.canPreview : root.canPlay) && !root.board.pending && root.bridge.daemonConnected
      Accessible.name: (root.previewMode ? "Preview " : "Play ") + modelData.name + (root.previewMode ? " privately" : " into voice")
      ToolTip.visible: hovered || visualFocus
      ToolTip.text: Accessible.name + " · " + (modelData.duration_ms/1000).toFixed(1) + "s"
      onClicked: root.board.play(root.serverId,modelData.id,root.previewMode)
      Keys.onLeftPressed: pads.focusPad(index-1)
      Keys.onRightPressed: pads.focusPad(index+1)
      Keys.onUpPressed: pads.focusPad(index-pads.columns)
      Keys.onDownPressed: pads.focusPad(index+pads.columns)
      contentItem: Column {
        spacing: root.theme.space(3)
        opacity: pad.enabled ? 1 : 0.45
        Text {
          width: parent.width; height: root.theme.space(32)
          text: pad.modelData.name; textFormat: Text.PlainText
          wrapMode: Text.Wrap; maximumLineCount: 2; elide: Text.ElideRight
          color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        Text {
          width: parent.width
          text: (root.theme.tui ? "[▶] " : "▶ ") + (pad.modelData.duration_ms/1000).toFixed(1) + "s"
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
      }
    }
  }
  ListView {
    id: soundList; objectName: "soundboardList"; width: parent.width
    visible: !root.playOnly
    height: visible ? Math.min(contentHeight,root.theme.space(320)) : 0; clip: true; spacing: root.theme.spacing.sm
    model: root.playOnly ? [] : root.filteredSounds
    ScrollBar.vertical: ScrollBar {policy:parent.contentHeight>parent.height+1 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff}
    delegate: Rectangle {
      id: entry; required property var modelData; width: soundList.width; height: row.implicitHeight + root.theme.spacing.md*2
      color: root.theme.alpha(root.theme.foreground,0.035); radius: root.theme.cornerRadius
      Column {
        id: row; x: root.theme.spacing.md; y: root.theme.spacing.md; width: parent.width-x*2; spacing: root.theme.spacing.sm
        Text {
          visible: !root.playOnly
          width: parent.width; elide: Text.ElideRight; textFormat: Text.PlainText
          text: entry.modelData.name + " · " + (entry.modelData.duration_ms/1000).toFixed(1) + "s"
          color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
        }
        Text {
          visible: !root.playOnly
          width: parent.width; elide: Text.ElideRight; textFormat: Text.PlainText; text: "Added by " + entry.modelData.owner_name
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        Flow {
          width: parent.width; spacing: root.theme.spacing.sm
          ChatButton {objectName:"soundboardPreview-"+entry.modelData.id; theme:root.theme;text:"Preview";enabled:root.canPreview && !root.board.pending && root.bridge.daemonConnected;onClicked:root.board.play(root.serverId,entry.modelData.id,true)}
          ChatButton {objectName:"soundboardPlay-"+entry.modelData.id; visible:!root.playOnly; theme:root.theme;text:"Play into voice";primary:true;enabled:root.canPlay && !root.board.pending && root.bridge.daemonConnected;onClicked:root.board.play(root.serverId,entry.modelData.id,false)}
          ChatButton {objectName:"soundboardRemove-"+entry.modelData.id; theme:root.theme;text:"Remove";destructive:true;visible:!root.playOnly && (entry.modelData.owner_id===root.self.id || !!root.self.server_owner || !!root.self.server_admin);enabled:!root.board.busy[root.serverId];onClicked:{root.removing={server:root.serverId,id:entry.modelData.id,name:entry.modelData.name};removeDialog.open()}}
        }
      }
    }
  }
  Dialogs.FileDialog {
    id: filePicker; title: "Choose a sound up to 10 seconds"; nameFilters: ["Audio (*.wav *.mp3 *.ogg *.flac)"]
    onAccepted: if (root.pickerServer === root.serverId) {
      root.selectedPath=selectedFile.toString()
      if (!name.text.trim()) name.text=decodeURIComponent(root.selectedPath.split("/").pop()).replace(/\.[^.]+$/, "").slice(0,32)
    }
  }
  Dialog {
    ThemeControlStyle {theme:root.theme;control:removeDialog}
    id: removeDialog; parent: Overlay.overlay; anchors.centerIn: parent; modal: true
    width: Math.min(root.theme.space(360), parent ? parent.width-root.theme.spacing.lg*2 : root.theme.space(360))
    implicitHeight: root.theme.space(200)
    title: "Remove sound?"; standardButtons: Dialog.Yes | Dialog.Cancel
    contentItem: Item {
      implicitWidth: root.theme.space(280)
      implicitHeight: removeText.implicitHeight
      Text {
        id: removeText; width: parent.width
        text: "Remove “" + (root.removing ? root.removing.name : "") + "” for everyone on this server?"
        textFormat: Text.PlainText; wrapMode: Text.Wrap
        color: root.theme.foreground; font.family: root.theme.font.family
      }
    }
    background: Rectangle {color:root.theme.surface;border.color:root.theme.separator;border.width:1;radius:root.theme.cornerRadius}
    onAccepted: if (root.removing && root.removing.server === root.serverId) root.board.remove(root.removing.server,root.removing.id)
  }
}
