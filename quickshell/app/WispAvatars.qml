import QtQuick

Item {
  id: root
  required property var bridge
  property var images: ({})
  property var loading: ({})
  property var busy: ({})
  property var feedback: ({})
  property int epoch: 0
  signal saved(string serverId, bool success)
  // A server can reconnect while the local daemon stays connected. Reload any
  // pictures that may have changed while its event stream was unavailable.
  readonly property string connections: JSON.stringify(((bridge.snapshot || {}).server_states || []).map(function(state) {
    return [String(state.server.id),!!state.server.connected]
  }).sort(function(a,b) { return a[0].localeCompare(b[0]) }))
  onConnectionsChanged: invalidate()
  function key(server,user) { return String(server) + "::" + String(user) }
  function url(server,user) { return images[key(server,user)] || "" }
  function request(name,args,action) {
    var id = bridge.send(name,args)
    if (id) bridge.requests[id] = Object.assign({kind:"avatar",epoch:epoch},action)
    return id
  }
  function load(server,user) {
    if (!bridge.daemonConnected || !server || !/^[0-9a-f-]{36}$/i.test(user)) return
    var k = key(server,user)
    if (images[k] !== undefined || loading[k]) return
    if (request("avatar_image",{server_id:server,user_id:user},{action:"image",key:k})) loading = bridge.replaceEntry(loading,k,true)
  }
  function invalidate() { images=({}); loading=({}); epoch++ }
  function reconnect() { busy=({}); invalidate() }
  function save(server,path) {
    if (busy[server]) return
    var action = path ? "upload_avatar" : "remove_avatar"
    if (request(action,{server_id:server,path:path || ""},{action:action,serverId:server})) {
      busy = bridge.replaceEntry(busy,server,true)
      feedback = bridge.replaceEntry(feedback,server,path ? "Saving picture…" : "Removing picture…")
    }
  }
  function reply(message,action) {
    // Image replies are scoped to a cache generation; mutations must still clear
    // busy state if their server event arrived before the command response.
    if (action.action === "image") {
      if (action.epoch !== epoch) return
      loading = bridge.replaceEntry(loading,action.key,undefined)
      images = bridge.replaceEntry(images,action.key,message.ok ? String((message.value || {}).url || "") : "")
    } else {
      busy = bridge.replaceEntry(busy,action.serverId,false)
      feedback = bridge.replaceEntry(feedback,action.serverId,message.ok ? (action.action === "upload_avatar" ? "Profile picture saved" : "Profile picture removed") : String((message.error || {}).message || "Could not save profile picture"))
      if (message.ok) { invalidate(); bridge.settingsSaved() }
      saved(action.serverId,!!message.ok)
    }
  }
}
