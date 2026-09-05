import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("RECOVERY_FAILED: " + message) } }
  property var snapshot: ({})
  function observe(connected, available, event, room) {
    var state = {server:{id:"home",connected:available},spots:[{id:"lounge",name:"Lounge",active_hangout_id:"voice"}],hangouts:[{id:"voice"}]}
    snapshot = {voice_server_id:"home",self:{hangout_id:room === undefined ? "voice" : room,media:{livekit_connected:connected}},servers:[state.server],server_states:[state]}
    bridge.serverStates = snapshot.server_states; bridge.servers = snapshot.servers; bridge.selfState = snapshot.self
    recovery.observe(snapshot,event)
  }
  function drop() { observe(true,true,"media_connected"); observe(false,false,"voice_connection_lost") }
  QtObject {
    id: bridge
    property string configHome: Quickshell.env("XDG_CONFIG_HOME")
    property bool notificationSoundsEnabled: true
    property bool daemonConnected: true
    property var serverStates: []
    property var servers: []
    property var selfState: ({})
    property var requests: ({})
    property var commands: []
    property var sounds: []
    property string lastError: ""
    signal settingsSaved()
    signal settingsSaveFailed()
    function send(name,args) { recovery.manualCommand(name,args); commands.push({name:name,args:args}); return "test-" + commands.length }
    function playNotificationSound(kind) { sounds.push(kind) }
    function playExitDisconnectSound() { sounds.push("exit_leave") }
  }
  Wisp.WispVoiceRecovery { id: recovery; bridge: bridge }
  Timer {
    interval:100; running:true
    onTriggered: {
      test.check(recovery.enabledSetting,"automatic reconnect defaults on")
      test.observe(false,true,"snapshot",null)
      test.observe(false,true,"hangout_changed")
      test.check(bridge.sounds.length === 0,"joining membership alone does not play connected")
      test.observe(true,true,"media_connected")
      test.observe(true,true,"active_speakers")
      test.check(JSON.stringify(bridge.sounds) === '["self_join"]',"successful voice connection plays one join cue")
      test.observe(false,false,"voice_connection_lost")
      test.observe(false,false,"server_connection_changed")
      test.check(recovery.pending && bridge.sounds.length === 2 && bridge.sounds[1] === "self_leave","server outage starts one recovery episode and one leave cue")
      var before = bridge.commands.length
      recovery.tick(Date.now()+3000)
      test.check(bridge.commands.length === before,"offline servers are not sent join requests")
      test.observe(false,true,"server_reconnected",null)
      recovery.tick(Date.now()+3000)
      test.check(bridge.commands.slice(-1)[0].name === "join_spot" && bridge.commands.slice(-1)[0].args.spot_id === "lounge","server restart rejoins the saved room, not a stale call id")
      var request = recovery.requestId
      recovery.reply({id:request,ok:true})
      test.check(bridge.sounds.length === 2,"HTTP acknowledgement is not a successful media connection")
      test.observe(true,true,"media_connected")
      test.check(!recovery.pending && bridge.sounds.length === 3 && bridge.sounds[2] === "self_join","restored voice plays its connection cue once")

      test.drop(); test.observe(false,true,"server_reconnected",null)
      for (var i=0; i<6; i++) {
        recovery.tick(recovery.nextAttempt)
        recovery.reply({id:recovery.requestId,ok:false,error:{message:"Network timeout"}})
      }
      test.check(!recovery.pending && recovery.attempts === 6 && recovery.statusText.indexOf("6 attempts") >= 0,"six failures stop retries with an explanation")
      before = bridge.commands.length; recovery.tick(Date.now()+90000)
      test.check(bridge.commands.length === before,"exhausted retries stay stopped")
      test.drop(); recovery.tick(recovery.deadline)
      test.check(!recovery.pending && recovery.statusText.indexOf("timed out") >= 0,"two-minute deadline expires even while offline")
      test.drop(); bridge.send("leave",{})
      test.check(!recovery.pending,"manual disconnect cancels recovery")
      test.drop(); bridge.send("join_spot",{spot_id:"other",server_id:"elsewhere"})
      test.check(!recovery.pending,"explicit room or server join cancels the old target")
      test.drop(); recovery.enabledSetting = false
      test.check(!recovery.pending,"turning the setting off cancels pending retries")
      test.drop(); test.check(!recovery.pending,"disabled preference does not create retries")
      recovery.enabledSetting = true
      test.drop(); test.observe(false,true,"server_reconnected",null)
      recovery.tick(recovery.nextAttempt)
      recovery.reply({id:recovery.requestId,ok:false,error:{message:"403 Forbidden"}})
      test.check(!recovery.pending,"lost permissions stop retrying immediately")
      test.drop(); bridge.serverStates = [{server:{id:"home",connected:true},spots:[],hangouts:[]}]; bridge.servers = [{id:"home",connected:true}]
      recovery.tick(recovery.nextAttempt)
      test.check(!recovery.pending,"deleted rooms are never recreated by retrying")
      test.observe(true,true,"media_connected")
      before = bridge.sounds.length; recovery.daemonLost(); recovery.daemonLost()
      test.check(bridge.sounds.length === before+1 && !recovery.pending,"daemon loss plays one disconnect and never carries a retry across restart")
      test.observe(false,true,"snapshot",null)
      test.observe(true,true,"media_connected")
      before = bridge.sounds.length; recovery.beginShutdown()
      test.observe(false,true,"voice_left",null)
      test.check(bridge.sounds.length === before+1 && bridge.sounds.slice(-1)[0] === "exit_leave" && !recovery.pending,"normal exit starts its cue before teardown without a duplicate leave")
      test.check(!bridge.commands.some(function(c) { return c.name === "share" || c.name === "camera" }),"recovery never restarts video publishing")
      console.log(test.failed ? "RECOVERY_FAILED" : "RECOVERY_OK")
      Qt.quit()
    }
  }
}
