import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  required property var bridge
  property var info: ({phase:"idle",available:false})
  property var preferences: ({automatic:true,check_on_launch:true,background_checks:true,interval_minutes:5})
  property bool initialized: false
  property bool ready: false
  property string leasePath: ""
  property string processStart: ""
  property string action: ""
  property var queue: []
  property bool checking: action === "check"
  readonly property bool busy: ["downloading","preparing","installing"].indexOf(info.phase) >= 0 || action === "start"
  readonly property bool preparing: info.phase === "preparing" || info.phase === "installing"
  readonly property bool available: !!info.available
  readonly property bool foreground: bridge.appFocused
  readonly property bool safe: bridge.daemonConnected && !bridge.voiceRecovery.pending && !bridge.voiceRecovery.shuttingDown
    && ["joining","reconnecting"].indexOf(bridge.selfState.connection) < 0
    && !bridge.selfState.hangout_id && !bridge.selfState.sharing
    && !(bridge.selfState.media || {}).livekit_connected
    && !Object.keys(bridge.watchedMedia).length && !Object.keys(bridge.mediaWatchRequests).length
    && !hasValues(bridge.drafts) && !hasValues(bridge.pendingAttachments) && !hasValues(bridge.sendingConversations)
    && !hasValues(bridge.importingConversations) && !hasValues(bridge.savingFiles) && !hasValues(bridge.messageActions.replies)
    && !bridge.privacyBusy && !bridge.profileBusy && !bridge.serverSettingsBusy && !bridge.audioTestBusy
    && !bridge.friendships.busy
    && ["idle","ready","complete","error"].indexOf(bridge.audioTestState.phase) >= 0
    && !bridge.soundboard.pending && !bridge.soundboard.playing && !bridge.soundboard.previewing && !hasValues(bridge.soundboard.busy)
  readonly property string statusText: {
    if (checking) return "Checking for updates…"
    if (info.phase === "downloading") return "Downloading update…"
    if (preparing) return "Installing update… Wisp will reopen shortly."
    if (info.error) return info.error
    if (available) return "A Wisp update is available."
    if (info.phase === "updated") return "Wisp has been updated."
    return info.last_checked ? "You're up to date." : "Check for the latest Wisp release."
  }
  function hasValues(map) { return Object.keys(map || {}).some(function(k) { var v=map[k]; return Array.isArray(v) ? v.length > 0 : !!v }) }
  function enqueue(args) {
    queue=queue.concat([args]); advance()
  }
  function advance() {
    if (helper.running || action || !queue.length) return
    var args=queue[0]; queue=queue.slice(1); action=args[0]
    helper.command=[(Quickshell.env("XDG_BIN_HOME") || Quickshell.env("HOME") + "/.local/bin") + "/wisp-updater"].concat(args)
    helper.running=true
  }
  function accept(value) {
    if (value.preferences) preferences=value.preferences
    if (value.lease_path) { processStart=value.process_start; leasePath=value.lease_path; ready=true; leaseTimer.restart() }
    info=Object.assign({},info,value)
  }
  function initialize() {
    if (initialized || !bridge.daemonConnected) return
    initialized=true
    enqueue(["register",String(Quickshell.processId),bridge.clientName])
  }
  function checkNow() { if (!checking && !busy) enqueue(["check","force"]) }
  function setPreference(key,value) { enqueue(["set",key,String(value)]) }
  function install(automatic) {
    if (!ready || !available || busy || !safe || automatic && foreground) return
    writeLease(); enqueue(["start",automatic ? "automatic" : "manual"])
  }
  function writeLease() {
    if (!ready) return
    lease.setText(JSON.stringify({pid:Quickshell.processId,start:processStart,client:bridge.clientName,safe:safe,
      foreground:foreground,prepared:preparing && safe && (!info.automatic_request || !foreground) ? info.request : ""}))
  }
  function reloadState() { try { accept(JSON.parse(stateFile.text())) } catch (_) {} }
  Connections { target: root.bridge; function onDaemonConnectedChanged() { root.initialize(); leaseTimer.restart() } }
  Component.onCompleted: initialize()
  onSafeChanged: leaseTimer.restart()
  onForegroundChanged: leaseTimer.restart()
  onInfoChanged: leaseTimer.restart()
  Timer { id: leaseTimer; interval:100; onTriggered: root.writeLease() }
  // No periodic activity polling: UI state changes write a small local lease.
  // The installer requests a fresh acknowledgement after its download finishes.
  FileView { id: lease; path:root.leasePath; blockWrites:true; atomicWrites:true; printErrors:false }
  FileView {
    id: stateFile
    path: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/wisp/update-state.json"
    watchChanges:true; printErrors:false
    onFileChanged: reload()
    onLoaded: root.reloadState()
  }
  FileView {
    path:(Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/updates.json"
    watchChanges:true; printErrors:false
    onFileChanged: reload()
    onLoaded: { try { root.preferences=Object.assign({},root.preferences,JSON.parse(text())) } catch (_) {} }
  }
  Process {
    id: helper
    stdout: StdioCollector { onStreamFinished: { try { root.accept(JSON.parse(this.text)) } catch (_) {} } }
    stderr: StdioCollector {}
    onExited: function(code, status) {
      var completed=root.action; root.action=""
      if (code !== 0 && !root.info.error) root.info=Object.assign({},root.info,{error:"The update helper is unavailable. Try again later."})
      if (completed === "register" && root.ready && root.preferences.check_on_launch) root.enqueue(["check"])
      if (completed === "start") refreshAfterStart.restart()
      Qt.callLater(root.advance)
    }
  }
  Timer { id: refreshAfterStart; interval:1500; onTriggered:root.enqueue(["status"]) }
  Timer {
    interval:Math.max(1,Math.min(1440,root.preferences.interval_minutes || 5))*60000
    repeat:true; running:root.ready && root.preferences.background_checks && !root.busy
    onTriggered: if (!root.checking) root.enqueue(["check"])
  }
  Timer {
    interval:60000; running:root.ready && root.available && root.preferences.automatic && root.safe && !root.foreground && !root.busy
      && root.info.attempted_commit !== (root.info.release || {}).commit
    onTriggered:root.install(true)
  }
  Timer { interval:5000; repeat:true; running:root.busy && !helper.running; onTriggered:root.enqueue(["status"]) }
}
