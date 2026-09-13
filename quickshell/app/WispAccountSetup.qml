import QtQuick

Item {
  id: root
  required property var bridge
  readonly property string serverId: String(bridge.profileServerId || "")
  readonly property string scope: JSON.stringify([serverId, String((bridge.activeServerState.self || {}).id || "")])
  readonly property bool ready: bridge.daemonConnected && bridge.privacySnapshotReady && !!(bridge.activeServerState.self || {}).id
  readonly property var status: bridge.accountBackup || ({})
  readonly property bool needsAttention: !!status.pending || !!status.installation_pending
    || !status.error && (status.mode === "classic" || status.mode === "secure" && (!status.unlocked || !!status.repair_needed))
  readonly property bool offered: ready && needsAttention && !deferred[scope]
  property var host: null
  property var deferred: ({})
  property string requestId: ""
  property int generation: 0
  readonly property bool busy: !!requestId || bridge.profileBusy
  property string error: ""
  signal invalidated()
  function reset() {
    generation++; requestId = ""; error = ""; host = null
    bridge.accountBackup = ({})
    bridge.profileRequestId = ""; bridge.profileBusy = false; bridge.profileReady = false
    invalidated()
    Qt.callLater(refresh)
  }
  onScopeChanged: reset()
  onReadyChanged: { if (!ready) reset(); else Qt.callLater(refresh) }
  Component.onCompleted: Qt.callLater(refresh)
  function refresh() { if (ready && !busy) act("backup_status", {}) }
  function request() {
    var next = Object.assign({}, deferred); delete next[scope]; deferred = next
    refresh()
  }
  function claim(surface) {
    if (!offered || host && host !== surface) return false
    host = surface; return true
  }
  function dismiss(surface, openedScope) {
    if (host !== surface) return
    host = null
    if (scope === openedScope) deferred = Object.assign({}, deferred, {[scope]:true})
  }
  function act(name, args) {
    if (!ready || busy) return false
    var id = bridge.send(name, Object.assign({}, args || {}, {server_id:serverId}))
    if (!id) return false
    // Keep no password or command arguments in the UI request journal.
    bridge.requests[id] = {kind:"accountSetup", action:name, scope:scope, generation:generation}
    requestId = id
    if (name !== "backup_status") error = ""
    return true
  }
  function finish(message, action) {
    if (action.scope !== scope || action.generation !== generation || message.id !== requestId) return
    requestId = ""
    if (message.ok) {
      bridge.accountBackup = message.value || ({})
      var value = message.value || ({})
      if (value.error || value.sync_error) error = String(value.error || value.sync_error)
      else if (action.action !== "backup_status") error = ""
    } else {
      error = message.error ? String(message.error.message || "Account action failed") : "Account action failed"
      if (action.action !== "backup_status") Qt.callLater(refresh)
    }
  }
  Timer { interval:60000; repeat:true; running:root.ready; onTriggered:root.refresh() }
}
