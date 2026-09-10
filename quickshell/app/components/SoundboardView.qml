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
    text: "Soundboard · " + String((root.serverState.server || {}).name || "This server")
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: root.playOnly ? "Click a sound to play it into your voice room." : "Shared with this server. Add a short sound, preview it, or play it into your voice room."
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
    text: !root.inCall ? "Join this server's voice room to play for everyone. Preview stays on your device."
      : root.deafened ? "Undeafen to use the soundboard."
      : root.bridge.effectiveMuted ? "Private preview is available. Unmute to play into voice."
      : root.bridge.voiceServerId !== root.serverId ? "Your call is on another server. Open that server's soundboard to play."
      : "Play sends the sound to everyone in your call. Mute your mic to preview privately."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
    visible: text !== ""; text: root.board.feedback[root.serverId] || ""
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Row {
    width: parent.width; spacing: root.theme.spacing.sm
    TextField { id: search; objectName: "soundboardSearch"; width: parent.width-refresh.width-parent.spacing; placeholderText: "Search sounds"; Accessible.name: "Search sounds"; ThemeControlStyle {theme:root.theme;control:search} }
    ChatButton { id: refresh; theme: root.theme; text: "Refresh"; enabled: root.bridge.daemonConnected && !root.board.loading[root.serverId]; onClicked: root.board.refresh(root.serverId,true) }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap; visible: soundList.count === 0
    text: root.board.loading[root.serverId] ? "Loading sounds…" : !root.bridge.daemonConnected ? "Connect to Wisp to load this library." : root.sounds.length ? "No sounds match your search." : root.playOnly ? "No sounds yet. Add starter sounds or open Manage sounds to upload your own." : "No sounds yet. Upload the first one for this server."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  ListView {
    id: soundList; objectName: "soundboardList"; width: parent.width
    height: Math.min(contentHeight,root.theme.space(320)); clip: true; spacing: root.theme.spacing.sm
    model: root.sounds.filter(function(s){return s.name.toLowerCase().indexOf(search.text.toLowerCase().trim()) >= 0})
    ScrollBar.vertical: ScrollBar {}
    delegate: Rectangle {
      id: entry; required property var modelData; width: soundList.width; height: row.implicitHeight + root.theme.spacing.md*2
      color: root.theme.alpha(root.theme.foreground,0.035); radius: root.theme.cornerRadius
      Column {
        id: row; x: root.theme.spacing.md; y: root.theme.spacing.md; width: parent.width-x*2; spacing: root.theme.spacing.sm
        ChatButton {
          objectName: "soundboardQuickPlay-" + entry.modelData.id
          visible: root.playOnly; width: parent.width; height: root.theme.space(44)
          theme: root.theme; primary: true; textAlignment: Text.AlignLeft
          text: "▶ " + entry.modelData.name + " · " + (entry.modelData.duration_ms/1000).toFixed(1) + "s"
          Accessible.name: "Play " + entry.modelData.name + " into voice"
          enabled: root.canPlay && !root.board.pending && root.bridge.daemonConnected
          onClicked: root.board.play(root.serverId,entry.modelData.id,false)
        }
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
    title: "Remove sound?"; standardButtons: Dialog.Yes | Dialog.Cancel
    contentItem: Text {text: "Remove “" + (root.removing ? root.removing.name : "") + "” for everyone on this server?";textFormat:Text.PlainText;wrapMode:Text.Wrap;color:root.theme.foreground; font.family:root.theme.font.family; width:root.theme.space(280)}
    background: Rectangle {color:root.theme.surface;border.color:root.theme.separator;border.width:1;radius:root.theme.cornerRadius}
    onAccepted: if (root.removing && root.removing.server === root.serverId) root.board.remove(root.removing.server,root.removing.id)
  }
}
