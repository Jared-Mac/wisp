import QtQuick
import QtQuick.Controls
import "../components"

Item {
  id:root
  objectName:"trayFocusedWorkspace"
  required property var bridge
  required property var theme
  signal cameraRequested()
  signal serverSettingsRequested()
  signal createRoomRequested()
  readonly property bool inVoice:!!bridge.currentVoiceRoom
  readonly property var voiceSpot: {
    var spot=(bridge.voiceServerState.spots || []).filter(function(room){return room.active_hangout_id===root.bridge.selfState.hangout_id})[0]
    return spot ? Object.assign({},spot,{server_id:bridge.voiceServerId}) : null
  }
  readonly property real buttonSize:Math.min(theme.space(24),Math.max(theme.space(20),(width-theme.space(40)-8*gap)/8))
  readonly property real gap:theme.space(3)
  Flickable {
    id:chat;objectName:"focusedTrayChat"
    anchors {left:parent.left;right:parent.right;top:parent.top;bottom:controls.top;bottomMargin:root.theme.space(6)}
    contentWidth:width;contentHeight:messages.implicitHeight
    clip:true;boundsBehavior:Flickable.StopAtBounds
    ScrollBar.vertical:ScrollBar {}
    MessagesView {id:messages;width:parent.width;availableHeight:chat.height;bridge:root.bridge;theme:root.theme}
  }
  SoundboardPopup {id:soundboard;bridge:root.bridge;theme:root.theme;hostItem:root}
  Row {
    id:controls;objectName:"focusedTrayControls"
    anchors {left:parent.left;right:parent.right;bottom:parent.bottom}
    height:root.buttonSize;spacing:root.gap
    TrayPeekButton {
      id:voice;objectName:"trayVoicePeek"
      theme:root.theme;width:Math.max(root.theme.space(24),controls.width-8*root.buttonSize-8*root.gap)
      height:root.buttonSize
      text:root.inVoice ? root.bridge.currentVoiceLabel+" - "+(root.bridge.mediaState.livekit_connected ? "connected" : "connecting…") : String(root.bridge.activeServer.name || "Voice")
      formatLabel:false;textAlignment:Text.AlignLeft
      Accessible.name:root.inVoice ? text+". Show voice participants" : "Not in voice. Show voice status"
      panelContent:Component {
        Column {
          spacing:root.theme.spacing.sm
          RoomCard {width:parent.width;visible:root.inVoice && !!root.voiceSpot;room:root.voiceSpot || {id:"",name:"Voice",members:[]};bridge:root.bridge;theme:root.theme;mainApp:true;adaptive:true}
          HangoutCard {width:parent.width;visible:root.inVoice && !root.voiceSpot;hangout:root.bridge.currentVoiceRoom || {id:"",label:"Voice",members:[]};bridge:root.bridge;theme:root.theme;adaptive:true}
          Text {visible:!root.inVoice;text:"Not connected to voice";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        }
      }
    }
    Repeater {
      model:[
        {action:"mute",icon:root.bridge.effectiveMuted ? "microphone-off" : "microphone",label:root.bridge.selfState.muted ? "Unmute microphone" : "Mute microphone",active:root.bridge.effectiveMuted},
        {action:"deafen",icon:root.bridge.selfState.deafened ? "headphones-off" : "headphones",label:root.bridge.selfState.deafened ? "Undeafen" : "Deafen",active:root.bridge.selfState.deafened},
        {action:"soundboard",icon:"soundboard",label:"Soundboard"},
        {action:"share",icon:root.bridge.sharing ? "screen-off" : "screen",label:root.bridge.sharing ? "Stop sharing screen" : "Share screen",active:root.bridge.sharing},
        {action:"camera",icon:root.bridge.cameraActive ? "camera-off" : "camera",label:root.bridge.cameraActive ? "Stop camera" : "Share camera",active:root.bridge.cameraActive},
        {action:"leave",icon:"disconnect",label:"Disconnect from voice"}
      ]
      ChatButton {
        id:action;required property var modelData
        objectName:"focusedAudio-"+modelData.action
        theme:root.theme;width:root.buttonSize;height:width
        text:modelData.label;iconName:modelData.icon;iconOnly:true;forceIcon:true
        primary:!!modelData.active;destructive:modelData.action==="leave"
        enabled:["share","camera","leave"].indexOf(modelData.action)<0 || root.inVoice
          && (modelData.action!=="share" || !root.bridge.shareStarting)
          && (modelData.action!=="camera" || root.bridge.cameraActive || !root.bridge.cameraStarting && root.bridge.cameraState.devices.length>0)
        ToolTip.visible:(hovered || visualFocus) && !soundboard.visible;ToolTip.text:text
        onClicked:{
          if(modelData.action==="mute")root.bridge.toggleMuted()
          else if(modelData.action==="deafen")root.bridge.toggleDeafened()
          else if(modelData.action==="share")root.bridge.toggleShare()
          else if(modelData.action==="camera")root.cameraRequested()
          else if(modelData.action==="leave")root.bridge.leave()
          else soundboard.openAt(action,Qt.point(width/2,0))
        }
      }
    }
    TrayPeekButton {
      objectName:"trayFriendsPeek";theme:root.theme;width:root.buttonSize
      height:width
      text:"Friends and members";iconName:"people";iconOnly:true;forceIcon:true
      panelContent:Component {
        Column {
          spacing:root.theme.spacing.sm
          FriendsView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:"panel"}
          ServerMembersView {width:parent.width;bridge:root.bridge;theme:root.theme;presentation:"panel";collapsible:false}
        }
      }
    }
    TrayPeekButton {
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
