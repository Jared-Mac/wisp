import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  required property var bridge
  property alias enabledSetting: preferences.enabled
  property var previous: null
  property bool wasConnected: false
  property var target: null
  property int attempts: 0
  property double deadline: 0
  property double nextAttempt: 0
  property string requestId: ""
  property bool sendingRetry: false
  property bool shuttingDown: false
  property string statusText: ""
  readonly property bool pending: !!target
  readonly property int maxAttempts: 6
  readonly property int retryWindowMs: 120000

  FileView {
    path: root.bridge.configHome + "/wisp/voice-recovery.json"
    blockLoading: true; blockWrites: true; atomicWrites: true; printErrors: false
    watchChanges: true; onFileChanged: reload()
    onAdapterUpdated: writeAdapter()
    onSaved: root.bridge.settingsSaved()
    onSaveFailed: { root.bridge.lastError = "Couldn't save voice reconnection settings."; root.bridge.settingsSaveFailed() }
    JsonAdapter { id: preferences; property bool enabled: true }
  }
  onEnabledSettingChanged: if (!enabledSetting) cancel("Automatic voice reconnection is off.", true)

  function voice(value) {
    var self = value.self || {}, media = self.media || {}
    var serverId = String(value.voice_server_id || value.selected_server_id || "local")
    var state = (value.server_states || []).filter(function(s) { return String(s.server.id) === serverId })[0] || value
    var server = (value.servers || []).filter(function(s) { return String(s.id) === serverId })[0] || state.server || {}
    return {serverId:serverId, roomId:String(self.hangout_id || ""), state:state,
      connected:!!self.hangout_id && !!media.livekit_connected && server.connected !== false,
      serverConnected:server.connected !== false, connection:String(self.connection || "")}
  }
  function cancel(reason, leave) {
    var hadTarget = !!target
    target = null; requestId = ""; statusText = reason || ""
    if (leave && hadTarget) { sendingRetry = true; bridge.send("leave", {}); sendingRetry = false }
  }
  function manualCommand(name, args) {
    if (sendingRetry) return
    if (["leave", "join_spot", "join_hangout", "join_friend", "respond_knock"].indexOf(name) >= 0
        || name === "respond_room_invitation" && args.accept)
      cancel("", false)
  }
  function observe(snapshot, eventName) {
    if (!bridge.notificationSoundsEnabled) return
    var current = voice(snapshot), old = previous
    previous = current
    if (shuttingDown) { wasConnected = false; return }
    if (!old) { wasConnected = current.connected; return }
    if (wasConnected && !current.connected) {
      bridge.playNotificationSound("self_leave")
      var unexpected = !current.serverConnected || ["media_reconnecting", "media_disconnected", "voice_connection_lost"].indexOf(eventName) >= 0
      if (unexpected && enabledSetting && !shuttingDown) {
        var room = (old.state.spots || []).filter(function(r) { return String(r.active_hangout_id || "") === old.roomId })[0]
        target = {serverId:old.serverId,roomId:old.roomId,spotId:room ? String(room.id) : "",label:room ? room.name : "voice"}
        attempts = 0; deadline = Date.now() + retryWindowMs; nextAttempt = Date.now() + 2000
        statusText = "Reconnecting voice…"
      }
    }
    if (current.connected && !wasConnected) {
      bridge.playNotificationSound("self_join")
      cancel("", false)
    }
    wasConnected = current.connected
    if (target && current.serverId !== target.serverId) cancel("", false)
    if (eventName === "voice_left") cancel("", false)
  }
  function daemonLost() {
    if (bridge.notificationSoundsEnabled && wasConnected) bridge.playNotificationSound("self_leave")
    wasConnected = false; previous = null
    cancel("", false) // A new daemon must never inherit a room to rejoin.
  }
  function beginShutdown() {
    var leaving = pending || !!bridge.selfState.hangout_id
    shuttingDown = true
    cancel("", false)
    // Start the cue before teardown, and suppress its later leave snapshot.
    if (wasConnected || (bridge.soundQueue || []).indexOf("self_leave") >= 0) { bridge.playExitDisconnectSound(); wasConnected = false }
    if (leaving) bridge.send("leave", {})
  }
  function tick(now) {
    if (!target || shuttingDown) return
    if (now >= deadline) { cancel("Voice reconnection timed out. Join again when you're ready.", true); return }
    if (requestId || now < nextAttempt || !bridge.daemonConnected) return
    var state = bridge.serverStates.filter(function(s) { return String(s.server.id) === root.target.serverId })[0]
    var server = bridge.servers.filter(function(s) { return String(s.id) === root.target.serverId })[0]
    if (!state || !server || server.connected === false) return
    if (attempts >= maxAttempts) { cancel("Couldn't reconnect voice after 6 attempts. Join again to retry.", true); return }
    var exists = target.spotId ? (state.spots || []).some(function(r) { return String(r.id) === root.target.spotId })
      : (state.hangouts || []).some(function(r) { return String(r.id) === root.target.roomId })
    if (!exists) { cancel("The voice room is no longer available.", true); return }
    attempts++
    statusText = "Reconnecting voice · " + attempts + "/" + maxAttempts
    sendingRetry = true
    var args = {server_id:target.serverId}
    if (target.spotId) args.spot_id = target.spotId; else args.hangout_id = target.roomId
    requestId = bridge.send(target.spotId ? "join_spot" : "join_hangout", args) || ""
    sendingRetry = false
    if (requestId) bridge.requests[requestId] = {kind:"voiceRecovery"}
    nextAttempt = now + [2000,5000,10000,20000,30000,30000][attempts - 1]
  }
  function reply(message) {
    if (String(message.id) !== requestId || !target) return
    requestId = ""
    var error = String((message.error || {}).message || "")
    if (!message.ok && /forbidden|permission|not.found|not.allowed|unauthorized|revoked|banned|encryption|media key/i.test(error))
      cancel("Voice reconnection stopped: " + error, true)
    else if (attempts >= maxAttempts && !message.ok)
      cancel("Couldn't reconnect voice after 6 attempts. Join again to retry.", true)
  }
  Timer { interval: 250; repeat: true; running: root.pending; onTriggered: root.tick(Date.now()) }
}
