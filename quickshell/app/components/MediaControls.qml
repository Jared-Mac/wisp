import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  signal leaveRequested()
  signal cameraRequested()
  property bool compact: false
  property bool adaptive: false
  readonly property bool small: adaptive && width < theme.space(180)
  property bool showLeave: true
  property string leaveObjectName: "mediaAction-leave"
  property bool showAudio: true
  property bool showPublishing: true
  property bool showSoundboard: true
  property bool showPushToTalk: true
  property bool showInvite: true
  property bool showRemoteStreams: true
  spacing: root.theme.spacing.sm
  SoundboardPopup { id: soundboardPopup; bridge: root.bridge; theme: root.theme; hostItem: root }
  RoomInvitePicker { id: invitePicker; bridge: root.bridge; theme: root.theme }
  Flow {
    id: controls
    readonly property real cellWidth: Math.min(root.width,root.theme.space(32))
    readonly property int columns: Math.max(1,Math.floor((root.width+spacing)/(cellWidth+spacing)))
    width: root.small ? columns*cellWidth+(columns-1)*spacing : parent.width
    x: (root.width-width)/2; spacing: root.theme.spacing.sm
    Repeater {
      id: controlRepeater
      model: (root.showAudio && (root.theme.friendly || root.small) ? [
        {label:root.bridge.selfState.muted ? "Unmute" : "Mute",action:"mute",icon:root.bridge.selfState.muted ? "microphone-off" : "microphone"},
        {label:root.bridge.selfState.deafened ? "Undeafen" : "Deafen",action:"deafen",icon:root.bridge.selfState.deafened ? "headphones-off" : "headphones"}
      ] : []).concat([
        {label:root.bridge.sharing ? "Stop share" : root.bridge.shareStarting ? "Choosing…" : "Share",action:"share",icon:root.bridge.sharing ? "screen-off" : "screen"},
        {label:root.bridge.cameraActive ? "Stop cam" : root.bridge.cameraStarting ? "Starting…" : "Camera",action:"camera",icon:root.bridge.cameraActive ? "camera-off" : "camera"},
        {label:"Soundboard",action:"soundboard",icon:"soundboard"},
        {label:"Invite",action:"invite",icon:"invite"},
        {label:"d/c",action:"leave",icon:"disconnect"}
      ]).filter(function(action) {return (root.showPublishing || ["share","camera"].indexOf(action.action)<0) && (root.showSoundboard || action.action!=="soundboard") && (root.showLeave || action.action!=="leave") && (root.showInvite || action.action!=="invite")})
      ChatButton {
        id: action; required property var modelData
        objectName: modelData.action === "leave" ? root.leaveObjectName : "mediaAction-" + modelData.action
        theme: root.theme; text: modelData.label; iconName: modelData.icon; forceIcon: root.small || modelData.action === "invite"
        readonly property bool publishing: modelData.action==="share" && root.bridge.sharing || modelData.action==="camera" && root.bridge.cameraActive
        readonly property bool controlEnabled: publishing || (modelData.action!=="share" || !root.bridge.shareStarting) && (modelData.action!=="camera" || !root.bridge.cameraStarting && root.bridge.cameraState.devices.length>0)
        Binding {target:action.background;property:"border.width";value:1;when:action.publishing}
        Binding {target:action.background;property:"border.color";value:root.theme.danger;when:action.publishing}
        enabled: controlEnabled
        destructive: publishing || modelData.action==="leave"
        primary: root.theme.friendly && (modelData.action==="mute" && root.bridge.selfState.muted || modelData.action==="deafen" && root.bridge.selfState.deafened)
        width: root.small || modelData.action === "invite" ? Math.min(root.width,root.theme.space(32)) : root.theme.friendly ? (root.compact ? root.theme.space(32) : (controls.width-controls.spacing*3)/4) : Math.min(root.width,actionMetrics.advanceWidth+root.theme.space(20))
        height: root.theme.space(root.theme.friendly || root.small ? (root.compact ? 32 : 40) : root.theme.tui ? 28 : 34)
        Accessible.name: modelData.action==="share" ? (publishing ? "Stop sharing screen" : "Share screen") : modelData.action==="camera" ? (publishing ? "Stop camera" : "Start camera") : modelData.action==="leave" ? "Disconnect from voice" : modelData.label
        ToolTip.visible: hovered || visualFocus; ToolTip.text: Accessible.name
        HoverHandler { id: actionPointer }
        onClicked: {
          if(modelData.action==="share") root.bridge.toggleShare()
          else if(modelData.action==="camera") root.cameraRequested()
          else if(modelData.action==="soundboard") soundboardPopup.openAt(action, actionPointer.hovered ? actionPointer.point.position : Qt.point(width/2, height))
          else if(modelData.action==="invite") invitePicker.open()
          else if(modelData.action==="mute") root.bridge.toggleMuted()
          else if(modelData.action==="deafen") root.bridge.toggleDeafened()
          else {root.bridge.leave();root.leaveRequested()}
        }
        TextMetrics { id: actionMetrics; font: actionLabel.font; text: actionLabel.text }
        contentItem: Item {
          WispIcon {theme:root.theme;name:action.iconName;ink:action.labelColor;visible:root.theme.friendly || action.forceIcon;anchors.centerIn:parent;width:Math.min(parent.width,root.theme.space(root.compact ? 18 : 20));height:width}
          Text {
            id: actionLabel; visible:!root.theme.friendly && !action.forceIcon; width:parent.width; anchors.bottom:parent.bottom
            height:root.theme.friendly ? root.theme.space(20) : parent.height
            text:root.theme.tui ? "["+action.modelData.label.toLowerCase()+"]" : action.modelData.action==="leave" ? "Disconnect" : action.modelData.label
            color:action.labelColor;opacity:action.enabled ? 1 : 0.45
            font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
            elide:Text.ElideRight;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter
          }
        }
      }
    }
  }
  Repeater {
    model: root.showRemoteStreams ? root.bridge.remoteVideos : []
    Rectangle {
      id: remoteStream; required property var modelData
      readonly property bool watching: !!modelData.surface_open || !!modelData.subscribed
      width:root.width;height:root.theme.space(44);radius:root.theme.cornerRadius;color:root.theme.alpha(root.theme.accent,0.07)
      Text {
        anchors.left:parent.left;anchors.leftMargin:root.theme.spacing.lg;anchors.right:watchButton.left;anchors.rightMargin:root.theme.spacing.md;anchors.verticalCenter:parent.verticalCenter
        text:String(remoteStream.modelData.participant || "Friend")+(remoteStream.modelData.source==="camera" ? " · camera" : " · live")
        color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;elide:Text.ElideRight
      }
      Menu {
        id:watchMenu
        ThemeControlStyle {theme:root.theme;control:watchMenu;outline:true}
        function watch(presentation) {root.bridge.watchVideo(Object.assign({},remoteStream.modelData,{presentation:presentation}),true)}
        MenuItem {id:separateStream;text:"Watch in dedicated window";onTriggered:watchMenu.watch("window");ThemeControlStyle {theme:root.theme;control:separateStream}}
        MenuItem {id:tiledStream;text:"Watch in app";onTriggered:watchMenu.watch("tile");ThemeControlStyle {theme:root.theme;control:tiledStream}}
      }
      ChatButton {
        id:watchButton;anchors.right:parent.right;anchors.rightMargin:root.theme.spacing.sm;anchors.verticalCenter:parent.verticalCenter
        theme:root.theme;text:remoteStream.watching ? "leave" : "watch"
        onClicked:remoteStream.watching ? root.bridge.watchVideo(remoteStream.modelData,false) : watchMenu.popup()
      }
    }
  }
  ChatButton {
    id:talk;theme:root.theme;visible:root.showPushToTalk && root.bridge.pushToTalkState.enabled;width:parent.width
    text:root.bridge.selfState.muted ? "Unmute before talking" : root.bridge.pushToTalkState.active ? "Talking — release to stop" : "Hold to talk"
    iconName:"microphone";enabled:!root.bridge.selfState.muted;primary:!!root.bridge.pushToTalkState.active
    onPressed:root.bridge.pushToTalkPress()
    onReleased:root.bridge.pushToTalkRelease()
    onCanceled:root.bridge.pushToTalkRelease()
  }
  Timer {interval:1000;repeat:true;running:talk.down && root.bridge.pushToTalkState.enabled;onTriggered:root.bridge.pushToTalkPress()}
}
