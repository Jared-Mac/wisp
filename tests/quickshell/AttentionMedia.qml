import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ChatTiles.js" as Tiles

ShellRoot {
  id: test
  property bool failed: false
  property var tile: null
  property string videoKey: ""
  property var volumePopup: null
  function findObject(item, name, visited) {
    if (!item || visited.indexOf(item)>=0) return null
    visited.push(item)
    if (item.objectName===name) return item
    for(var child of item.data || item.children || []) { var found=findObject(child,name,visited); if(found) return found }
    return null
  }
  function check(ok, message) { if (!ok) { failed=true; console.error("LOCAL_TEST_FAILED: " + message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName===name) return item
    for(var child of item.children || []) { var found=find(child,name); if(found) return found }
    return null
  }
  function changeMedia(speakers,level) {
    var next=JSON.parse(JSON.stringify(bridge.snapshot))
    next.self.media.active_speakers=speakers; next.self.media.remote_audio_levels={Jared:level}
    bridge.snapshot=next
  }
  Wisp.WispAppearance { id: appearance; environment: "desktop"; Component.onCompleted: setPalette("performative") }
  Wisp.WispTheme { id: theme; profile: "terminal"; appearanceController: appearance }
  Wisp.WispBridge {
    id: bridge
    mainWindowOpen: true
    property var sent: []
    property int soundCount: 0
    notificationSoundsEnabled: true
    function playNotificationSound() { soundCount++ }
    function send(name,args) { sent.push({name:name,args:args}); return "test-"+(++requestId) }
  }
  Wisp.WispWindow { id: window; bridge: bridge; theme: theme; visible: true; implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || 1180; implicitHeight: 800 }
  FloatingWindow {
    id: panel; visible: true; implicitWidth: 460; implicitHeight: 800
    Wisp.WispContent { id: tray; anchors.fill: parent; bridge: bridge; theme: theme; presentation: "panel" }
  }
  TestCase { id: keys; parent: window.contentItem; when: false }
  Components.ParticipantVolumeMenu { id: roomVolumes; parent: window.contentItem; bridge: bridge; theme: theme; people: bridge.hangouts.length ? bridge.hangouts[0].members : [] }
  Component.onCompleted: {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id="self"; data.self.display_name="Tyler"; data.self.hangout_id="porch"; data.self.muted=false
    data.self.media.remote_audio_participants=["Jared"]
    data.friends=[{id:"jared",display_name:"Jared",online:true,presence:"open"},{id:"charlie",display_name:"Charlie",online:false,presence:"away"}]
    data.hangouts=[{id:"porch",label:"Porch",members:[{id:"self",display_name:"Tyler"},{id:"jared",display_name:"Jared"},{id:"charlie",display_name:"Charlie"}]}]
    data.conversations=[{id:"porch",label:"Porch",kind:"hangout",unread_count:0},{id:"dm",label:"Jared",kind:"direct",unread_count:12}]
    data.messages=[]
    data.self.media.remote_videos=[{participant:"Jared",source:"screen_share",subscribed:true}]
    bridge.applySnapshot(data); bridge.selectConversation("porch")
  }
  Timer {
    interval: 200; running: true
    onTriggered: {
      test.tile=test.find(window.contentItem,"conversationPane")
      test.check(!!test.tile,"main tile host exists")
      test.check(!!test.find(tray,"unreadChat-dm"),"clickable unread chat shows its count and name")
      test.check(test.find(tray,"trayChatHeading").text.indexOf("/Porch")>=0,"tray heading names current chat")
      test.find(tray,"unreadChat-dm").clicked()
      test.check(bridge.activeConversationId==="dm" && bridge.lastConversationId==="porch","unread shortcut remembers prior chat")
      test.find(tray,"returnLastChat").clicked()
      test.check(bridge.activeConversationId==="porch","return shortcut opens prior chat")
      bridge.closeConversation()
      test.check(!!test.find(tray,"returnLastChat"),"return remains available from all chats")
      bridge.selectConversation("porch")
      bridge.participantVolumes.setVolume({id:"charlie"},50)
      bridge.participantVolumes.setVolume({id:"jared"},200)
      test.check(bridge.participantVolumes.volumeFor({id:"charlie"})===50,"per-person volume")
      test.check(bridge.participantVolumes.volumeFor({id:"jared"})===200,"independent boost")
      test.changeMedia(["Jared"],45)
      test.check(bridge.activeSpeakers.indexOf("Jared")>=0,"audio activity visible")
      test.tile.openVideo({participant:"Jared",source:"screen_share"})
      test.videoKey=Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)})[0].key
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)<0,"main-open watch docks by default")
      test.check(bridge.workspaceLayout.chatTiles.indexOf("video:")<0,"stream subscriptions never persisted")
    }
  }
  Timer {
    interval: 1000; running: true
    onTriggered: {
      var renderer=test.find(window.contentItem,"remoteVideoRenderer")
      test.check(renderer && renderer.item && renderer.item.ready,"native RGBA stream renders")
      if(renderer && renderer.item) test.check(renderer.item.frameSize.width===640 && renderer.item.frameSize.height===240,"ultrawide source dimensions preserved")
      test.changeMedia([],45)
      test.check(bridge.activeSpeakers.indexOf("Jared")>=0,"ongoing PCM level keeps highlight despite speaker-list omission")
      test.changeMedia([],0)
      test.check(bridge.activeSpeakers.indexOf("Jared")>=0,"brief silence has release hold")
      test.tile.detach(test.videoKey)
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"stream pops out")
    }
  }
  Timer {
    interval: 1400; running: true
    onTriggered: {
      test.tile.attach(test.videoKey)
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)<0,"anchor returns stream to tile")
      var next=JSON.parse(JSON.stringify(bridge.snapshot)); next.self.media.remote_muted_participants=["Jared"]; bridge.snapshot=next
      test.check(bridge.activeSpeakers.indexOf("Jared")<0,"mute overrides release immediately")
      next.self.media.remote_muted_participants=[]; bridge.snapshot=JSON.parse(JSON.stringify(next))
      var friend=test.find(window.contentItem,"favorite-jared").parent
      keys.mouseClick(friend,friend.width/2,friend.height/2,Qt.RightButton)
      test.volumePopup=test.findObject(friend,"participantVolumeMenu",[])
      test.check(test.volumePopup && test.volumePopup.visible,"right click friend opens local volume menu")
      test.volumePopup.close()
      var room=test.find(window.contentItem,"roomCard")
      keys.mouseClick(room,room.width/2,room.height/2,Qt.RightButton)
      test.volumePopup=test.findObject(room,"participantVolumeMenu",[])
      test.check(test.volumePopup && test.volumePopup.visible && test.volumePopup.participants.length===2,"right click room exposes all other participants")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path) window.contentItem.children[0].grabToImage(function(r){r.saveToFile(path+"-tiles.png")})
    }
  }
  Timer {
    interval: 1500; running: true
    onTriggered: {
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path && test.volumePopup) test.volumePopup.contentItem.parent.grabToImage(function(r){r.saveToFile(path+"-volumes.png")})
    }
  }
  Timer {
    interval: 1800; running: true
    onTriggered: {
      if(test.volumePopup) test.volumePopup.close()
      var renderer=test.find(window.contentItem,"remoteVideoRenderer")
      test.check(renderer && renderer.item && renderer.item.ready,"stream survives reparenting")
      test.tile.closePane(test.videoKey)
      test.check(bridge.sent[bridge.sent.length-1].name==="watch_video" && bridge.sent[bridge.sent.length-1].args.open===false,"closing stream tile stops watching locally")
      bridge.mainWindowOpen=false
      test.tile.openVideo({participant:"Jared",source:"screen_share"})
      test.videoKey=Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)})[0].key
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"main-closed watch opens popout")
      test.tile.closePane(test.videoKey)
      bridge.mainWindowOpen=true; bridge.workspaceLayout.streamsAsTiles=false
      test.tile.openVideo({participant:"Jared",source:"screen_share"})
      test.videoKey=Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)})[0].key
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"always-popout preference respected")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path) tray.grabToImage(function(r){r.saveToFile(path+"-tray.png")})
    }
  }
  Timer {
    interval: 2500; running: true
    onTriggered: {
      test.check(bridge.activeSpeakers.indexOf("Jared")<0,"silence releases highlight")
      var next=JSON.parse(JSON.stringify(bridge.snapshot)); next.self.media.remote_videos=[]; bridge.snapshot=next
      test.check(Tiles.leaves(test.tile.tree).every(function(n){return !test.tile.videoFor(n.id)}),"ended stream closes ephemeral tiles")
      test.check(!bridge.sent.some(function(c){return ["join_hangout","camera","share","set_conversation_tab"].indexOf(c.name)>=0}),"no publish/join or server-side chat close")
      bridge.focusedChats={fixture:"porch"}; bridge.appFocused=true
      bridge.receivedSnapshot=true // The mock deliberately has no daemon socket.
      var count=bridge.soundCount
      next=JSON.parse(JSON.stringify(next))
      next.messages=[{id:"new1",conversation_id:"dm",sender:{id:"jared"},content_type:"text/plain",payload:"test"}]
      bridge.applySnapshot(next,"message_created")
      test.check(bridge.soundCount===count+1,"other chat sounds while app focused")
      next=JSON.parse(JSON.stringify(next)); next.messages.push({id:"new2",conversation_id:"porch",sender:{id:"jared"},content_type:"text/plain",payload:"test"})
      bridge.applySnapshot(next,"message_created")
      test.check(bridge.soundCount===count+1,"focused chat does not sound")
      bridge.toggleChatNotifications("dm")
      next=JSON.parse(JSON.stringify(next)); next.messages.push({id:"new3",conversation_id:"dm",sender:{id:"jared"},content_type:"text/plain",payload:"test"})
      bridge.applySnapshot(next,"message_created")
      test.check(bridge.soundCount===count+1,"muted chat does not sound")
      for(var kind of ["member_join","member_leave","self_join","self_leave"]) {
        bridge.setEventSound(kind,"file:///tmp/"+kind+".wav")
        test.check(bridge.eventSoundPaths[kind]==="file:///tmp/"+kind+".wav","independent custom sound for "+kind)
      }
      var content=test.find(window.contentItem,"wispContent")
      content.toggleSettings()
      var settings=test.find(window.contentItem,"settingsMenu")
      for(var section of ["media","appearance","notifications","devices"]) {
        test.find(window.contentItem,"settingsTab-"+section).clicked()
        test.check(settings.section===section,"settings tab selects "+section)
      }
      test.find(window.contentItem,"settingsTab-notifications").clicked()
      tray.toggleSettings()
      test.find(tray,"settingsTab-notifications").clicked()
    }
  }
  Timer {
    interval: 2700; running: true
    onTriggered: {
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path) {
        window.contentItem.children[0].grabToImage(function(r){r.saveToFile(path+"-settings.png")})
        tray.grabToImage(function(r){r.saveToFile(path+"-tray-settings.png")})
      }
    }
  }
  Timer {
    interval: 3000; running: true
    onTriggered: {
      console.log(test.failed ? "LOCAL_TEST_FAILED" : "LOCAL_CONTROLS_OK")
      Qt.quit()
    }
  }
}
