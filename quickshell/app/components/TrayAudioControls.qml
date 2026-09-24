import QtQuick
import QtQuick.Controls
import "../views"

Item {
  id:root
  objectName:"trayAudioFooter"
  required property var bridge
  required property var theme
  signal cameraRequested()
  signal serverSettingsRequested()
  signal createRoomRequested()
  property bool navigationPeeks:false
  readonly property int buttonCount:6+(navigationPeeks ? 2 : 0)+(inVoice && bridge.pushToTalkState.enabled ? 1 : 0)
  readonly property bool inVoice:!!bridge.currentVoiceRoom
  readonly property var voiceSpot: {
    var spot=(bridge.voiceServerState.spots || []).filter(function(room){return room.active_hangout_id===root.bridge.selfState.hangout_id})[0]
    return spot ? Object.assign({},spot,{server_id:bridge.voiceServerId}) : null
  }
  readonly property real buttonSize:Math.min(theme.space(24),Math.max(theme.space(20),(width-theme.space(40)-buttonCount*gap)/buttonCount))
  readonly property real gap:theme.space(3)
  implicitHeight:buttonSize
  RoomInvitePicker {id:invite;bridge:root.bridge;theme:root.theme}
  Row {
    id:controls;objectName:"trayAudioRow"
    anchors {left:parent.left;right:parent.right;bottom:parent.bottom}
    height:root.buttonSize;spacing:root.gap
    TrayPeekButton {
      id:voice;objectName:"trayVoicePeek"
      theme:root.theme;width:Math.max(root.theme.space(24),controls.width-root.buttonCount*(root.buttonSize+root.gap))
      height:root.buttonSize
      text:root.inVoice ? root.bridge.currentVoiceLabel+" - "+(root.bridge.mediaState.livekit_connected ? "connected" : "connecting…") : String(root.bridge.activeServer.name || "Voice")
      formatLabel:false;textAlignment:Text.AlignLeft
      Accessible.name:root.inVoice ? text+". Show voice participants" : "Not in voice. Show voice status"
      panelContent:Component {
        Column {
          spacing:root.theme.spacing.sm
          RoomCard {width:parent.width;visible:root.inVoice && !!root.voiceSpot;room:root.voiceSpot || {id:"",name:"Voice",members:[]};bridge:root.bridge;theme:root.theme;mainApp:true;adaptive:true}
          HangoutCard {width:parent.width;visible:root.inVoice && !root.voiceSpot;hangout:root.bridge.currentVoiceRoom || {id:"",label:"Voice",members:[]};bridge:root.bridge;theme:root.theme;adaptive:true}
          ChatButton {visible:root.inVoice && !root.voiceSpot;theme:root.theme;text:"Invite";iconName:"invite";onClicked:invite.showAt(this)}
          Text {visible:!root.inVoice;text:"Not connected to voice";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        }
      }
    }
    AudioStateIndicator {
      objectName:"globalAudioControls"
      bridge:root.bridge;theme:root.theme;adaptive:true;forceIcons:true
      compactButtonSize:root.buttonSize;spacing:root.gap
      availableWidth:root.buttonSize*3+root.gap*2
      tooltipAbove:true
      muted:root.bridge.effectiveMuted;deafened:!!root.bridge.selfState.deafened
    }
    Repeater {
      model:[
        {action:"share",icon:root.bridge.sharing ? "screen-off" : "screen",label:root.bridge.sharing ? "Stop sharing screen" : "Share screen",active:root.bridge.sharing},
        {action:"camera",icon:root.bridge.cameraActive ? "camera-off" : "camera",label:root.bridge.cameraActive ? "Stop camera" : "Share camera",active:root.bridge.cameraActive},
        {action:"leave",icon:"disconnect",label:"Disconnect from voice"}
      ]
      ChatButton {
        id:action;required property var modelData
        objectName:modelData.action==="leave" ? "currentCallDisconnect" : "mediaAction-"+modelData.action
        theme:root.theme;width:root.buttonSize;height:width
        text:modelData.label;iconName:modelData.icon;iconOnly:true;forceIcon:true
        primary:!!modelData.active;destructive:modelData.action==="leave" || !!modelData.active
        enabled:["share","camera","leave"].indexOf(modelData.action)<0 || root.inVoice
          && (modelData.action!=="share" || !root.bridge.shareStarting)
          && (modelData.action!=="camera" || root.bridge.cameraActive || !root.bridge.cameraStarting && root.bridge.cameraState.devices.length>0)
        ToolTip.visible:hovered || visualFocus;ToolTip.text:text
        onClicked:{
          if(modelData.action==="share")root.bridge.toggleShare()
          else if(modelData.action==="camera")root.cameraRequested()
          else if(modelData.action==="leave")root.bridge.leave()
        }
      }
    }
    ChatButton {
      id:talk;objectName:"trayPushToTalk";theme:root.theme
      visible:root.inVoice && root.bridge.pushToTalkState.enabled
      width:root.buttonSize;height:width;text:root.bridge.selfState.muted ? "Unmute before talking" : "Hold to talk"
      iconName:"microphone";iconOnly:true;forceIcon:true;enabled:!root.bridge.selfState.muted
      primary:!!root.bridge.pushToTalkState.active
      ToolTip.visible:hovered || visualFocus;ToolTip.text:text
      onPressed:root.bridge.pushToTalkPress()
      onReleased:root.bridge.pushToTalkRelease()
      onCanceled:root.bridge.pushToTalkRelease()
      Timer {interval:1000;repeat:true;running:talk.down;onTriggered:root.bridge.pushToTalkPress()}
    }
    TrayPeekButton {
      visible:root.navigationPeeks
      objectName:"trayFriendsPeek";theme:root.theme;width:root.buttonSize
      height:width
      text:"Friends and members";iconName:"people";iconOnly:true;forceIcon:true
      panelContent:Component {
        Column {
          spacing:root.theme.spacing.sm
          PeopleView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:"panel"}
        }
      }
    }
    TrayPeekButton {
      visible:root.navigationPeeks
      objectName:"trayRoomsPeek";theme:root.theme;width:root.buttonSize
      height:width
      text:"Server and rooms";iconName:"room";iconOnly:true;forceIcon:true
      panelContent:Component {
        Column {
          spacing:root.theme.spacing.sm
          ServerSelector {width:parent.width;bridge:root.bridge;theme:root.theme;compact:true;showInvite:false;onSettingsRequested:root.serverSettingsRequested()}
          RoomsHeader {width:parent.width;bridge:root.bridge;theme:root.theme;onCreateRequested:root.createRoomRequested()}
          SpotsView {width:parent.width;bridge:root.bridge;theme:root.theme}
          ServerChannelsView {width:parent.width;bridge:root.bridge;theme:root.theme}
        }
      }
    }
  }
}
