import QtQuick

Item {
  id: root
  required property var bridge
  readonly property string server: String(bridge.activeServer.id || "")
  readonly property bool available: !!server && bridge.daemonConnected && bridge.serverMember && bridge.activeServer.connected !== false
  property var catalog: []
  readonly property var enabledPlugins: catalog.filter(function(p){return p.enabled})
  property bool canManage: false
  property bool busy: false
  property string feedback: ""
  property int epoch: 0
  onServerChanged: { reset(); bridge.plugins.reset(); refresh() }
  onAvailableChanged: { reset(); if(available) refresh() }
  Component.onCompleted: refresh()
  function reset() { epoch++; catalog=[];canManage=false;busy=false;feedback="" }
  function request(command,args) {
    if(!available || busy)return false
    var id=bridge.send(command,Object.assign({server_id:server},args || {}))
    if(!id)return false
    bridge.requests[id]={kind:"serverPlugins",epoch:epoch,server:server,command:command}
    busy=true;return true
  }
  function refresh() { if(!bridge.plugins.busy)request("plugin_server_catalog",{}) }
  function setEnabled(entry,enabled) {
    if(!canManage)return
    if(request("plugin_server_enable",{plugin_id:entry.manifest.id,digest:entry.digest,enabled:enabled}))feedback=enabled ? "Adding plugin to this server…" : "Removing server shortcut…"
  }
  function finish(message,action) {
    if(action.epoch!==epoch || action.server!==server)return
    busy=false
    if(!message.ok){feedback=String((message.error || {}).message || "Could not load server plugins.");return}
    catalog=(message.value || {}).catalog || [];canManage=!!(message.value || {}).can_manage
    feedback=action.command==="plugin_server_enable" ? "Server plugins updated. Members can open them from the main view." : ""
    if(bridge.plugins.serverMode && bridge.plugins.selected && !enabledPlugins.some(function(p){return p.manifest.id===bridge.plugins.selected}))bridge.plugins.reset()
  }
  Timer { interval:30000;repeat:true;running:root.available;onTriggered:root.refresh() }
}
