import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  function check(value, label) { if (!value) { failed=true; console.error("PROFILE_CUES_FAILED " + label) } }
  Wisp.WispAppearance { id: appearance; environment:"desktop" }
  Wisp.WispBridge {
    id: bridge
    notificationSoundsEnabled: true
    property var played: []
    function playNotificationSound(kind) { played.push(kind) }
    function send(name,args) { return "fixture-"+(++requestId) }
  }
  QtObject {
    id: fake
    property bool daemonConnected: true
    property var requests: ({})
    property var sent: []
    property int serial: 0
    signal settingsSaved()
    function send(name,args) { var id="avatar-"+(++serial); sent.push({name:name,args:args,id:id}); return id }
    function replaceEntry(map,key,value) { var next=Object.assign({},map); if(value===undefined) delete next[key]; else next[key]=value; return next }
  }
  Wisp.WispAvatars { id: avatars; bridge: fake }
  Timer {
    interval:100; running:true
    onTriggered: {
      test.check(bridge.audioControlSounds,"control sounds default enabled")
      var next=JSON.parse(JSON.stringify(bridge.snapshot)); next.self.id="self"; next.self.muted=false; next.self.deafened=false
      bridge.applySnapshot(next,"self_state_changed")
      test.check(bridge.played.length===0,"initial state is silent")
      next=JSON.parse(JSON.stringify(next)); next.self.muted=true; bridge.applySnapshot(next,"self_state_changed")
      bridge.applySnapshot(next,"self_state_changed")
      next=JSON.parse(JSON.stringify(next)); next.self.deafened=true; bridge.applySnapshot(next,"self_state_changed")
      next=JSON.parse(JSON.stringify(next)); next.self.deafened=false; next.self.muted=false; bridge.applySnapshot(next,"self_state_changed")
      next=JSON.parse(JSON.stringify(next)); next.self.muted=true; bridge.applySnapshot(next,"server_state_changed")
      next=JSON.parse(JSON.stringify(next)); next.self.muted=false; bridge.applySnapshot(next,"self_state_changed")
      test.check(JSON.stringify(bridge.played)==='["audio_mute","audio_deafen","audio_undeafen","audio_unmute"]',"four confirmed cues without duplicate, server or combined-state sounds")
      bridge.audioControlSounds=false
      test.check(bridge.notificationSoundCommand("audio_mute").length===0,"control sound switch is respected")
      bridge.audioControlSounds=true; bridge.notificationMuted=true
      test.check(bridge.notificationSoundCommand("audio_deafen").length===0,"global sound switch is respected")
      bridge.notificationMuted=false; bridge.notificationVolume=40
      bridge.setEventSound("audio_unmute","file:///tmp/custom-unmute.wav")
      test.check(bridge.notificationSoundCommand("audio_unmute")[3]==="/tmp/custom-unmute.wav","custom control sound is used")
      appearance.setShowAvatars(false)
      test.check(!appearance.showAvatars,"avatar preference applies")
      var user="00000000-0000-0000-0000-000000000001"
      avatars.load("a",user); avatars.load("a",user); avatars.load("b",user)
      test.check(fake.sent.length===2,"avatar loads are deduplicated and scoped by server")
      var first=fake.sent[0], action=fake.requests[first.id]
      avatars.reply({id:first.id,ok:true,value:{url:"data:image/png;base64,A"}},action)
      test.check(avatars.url("a",user)!==avatars.url("b",user),"servers cannot borrow each other's picture")
      var second=fake.sent[1], stale=fake.requests[second.id]
      avatars.invalidate()
      avatars.reply({id:second.id,ok:true,value:{url:"stale"}},stale)
      test.check(!avatars.url("b",user),"stale image replies are discarded")
      avatars.save("a","file:///tmp/picture.png")
      var mutation=fake.sent[fake.sent.length-1]
      avatars.invalidate() // The server event may beat the upload response.
      avatars.reply({id:mutation.id,ok:true,value:{}},fake.requests[mutation.id])
      test.check(!avatars.busy.a && avatars.feedback.a==="Profile picture saved","upload completion survives invalidation")
      avatars.save("a","")
      test.check(fake.sent[fake.sent.length-1].name==="remove_avatar","removal targets own account")
      console.log(test.failed ? "PROFILE_CUES_FAILED" : "PROFILE_CUES_OK")
      Qt.quit()
    }
  }
}
