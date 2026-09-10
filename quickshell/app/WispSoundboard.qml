import QtQuick
import "SoundboardDefaults.js" as Defaults

Item {
  id: root
  required property var bridge
  property var catalogs: ({})
  property var loading: ({})
  property var busy: ({})
  property var feedback: ({})
  property var starterQueues: ({})
  property var starterAdded: ({})
  property int epoch: 0
  property int revision: 0
  property int playSerial: 0
  property int volume: 70
  property string playServer: ""
  property bool pending: false
  property bool playing: false
  property bool previewing: false
  property bool statusPending: false
  signal uploaded(string serverId)

  function request(command, args, action) {
    var id = bridge.send(command, args)
    if (id) bridge.requests[id] = Object.assign({kind:"soundboard", epoch:epoch, revision:revision}, action)
    return id
  }
  function syncPlayback() {
    if (!bridge.daemonConnected || statusPending) return
    statusPending=!!request("soundboard_status", {}, {action:"status", serial:playSerial})
  }
  function reset() {
    epoch++; playSerial++; pending=false; playing=false; previewing=false; statusPending=false
    busy=({}); feedback=({}); starterQueues=({}); starterAdded=({}); invalidate()
  }
  function invalidate() { catalogs=({}); loading=({}); revision++ }
  function refresh(server, force) {
    if (!server || !bridge.daemonConnected || loading[server] || catalogs[server] && !force) return
    loading=bridge.replaceEntry(loading, server, true)
    if (!request("soundboard_list", {server_id:server}, {action:"catalog", serverId:server}))
      loading=bridge.replaceEntry(loading, server, false)
  }
  function mutate(server, command, args) {
    if (busy[server]) return
    busy=bridge.replaceEntry(busy, server, true)
    feedback=bridge.replaceEntry(feedback, server, command === "soundboard_upload" ? "Uploading sound…" : "Removing sound…")
    if (!request(command, Object.assign({server_id:server},args), {action:command, serverId:server}))
      busy=bridge.replaceEntry(busy, server, false)
  }
  function addDefaults(server) {
    if (!bridge.daemonConnected || busy[server] || loading[server] || !catalogs[server]) return
    var existing=catalogs[server].map(function(sound){return sound.name.toLowerCase()})
    var missing=Defaults.sounds.filter(function(sound){return existing.indexOf(sound.name.toLowerCase())<0})
    if (existing.length+missing.length>64) {
      feedback=bridge.replaceEntry(feedback,server,"Make room for " + missing.length + " starter sounds first (64 sounds per server).")
      return
    }
    starterQueues=bridge.replaceEntry(starterQueues,server,missing)
    starterAdded=bridge.replaceEntry(starterAdded,server,0)
    busy=bridge.replaceEntry(busy,server,true)
    nextDefault(server)
  }
  function nextDefault(server) {
    var remaining=starterQueues[server] || []
    if (!remaining.length) {
      busy=bridge.replaceEntry(busy,server,false)
      feedback=bridge.replaceEntry(feedback,server,starterAdded[server] ? "Added " + starterAdded[server] + " starter sounds." : "All starter sound names are already in this library.")
      starterQueues=bridge.replaceEntry(starterQueues,server,undefined)
      refresh(server,true)
      return
    }
    var sound=remaining[0]
    feedback=bridge.replaceEntry(feedback,server,"Adding " + sound.name + "…")
    var id=request("soundboard_upload",{server_id:server,name:sound.name,path:String(Qt.resolvedUrl("assets/soundboard/"+sound.file))},{action:"starter",serverId:server})
    if (!id) {
      busy=bridge.replaceEntry(busy,server,false)
      starterQueues=bridge.replaceEntry(starterQueues,server,undefined)
      feedback=bridge.replaceEntry(feedback,server,"Disconnected. Reconnect and add starter sounds again to finish.")
    }
  }
  function upload(server, name, path) { mutate(server, "soundboard_upload", {name:name, path:path}) }
  function remove(server, id) { mutate(server, "soundboard_remove", {sound_id:id}) }
  function play(server, id, preview) {
    if (pending) return
    playSerial++; pending=true; playServer=server
    feedback=bridge.replaceEntry(feedback, server, preview ? "Loading preview…" : "Loading sound…")
    if (!request(preview ? "soundboard_preview" : "soundboard_play", {server_id:server, sound_id:id, volume:volume}, {action:"play", serverId:server, serial:playSerial})) pending=false
  }
  function stop() {
    playSerial++; pending=false
    if (playServer) feedback=bridge.replaceEntry(feedback, playServer, "Stopped.")
    request("soundboard_stop", {}, {action:"stop", serial:playSerial})
  }
  function reply(message, action) {
    if (action.epoch !== epoch) return
    var server=action.serverId, value=message.value || {}
    var error=String((message.error || {}).message || "Soundboard unavailable. Check this server's connection.")
    if (action.action === "starter") {
      if (!starterQueues[server]) return
      if (message.ok) {
        starterAdded=bridge.replaceEntry(starterAdded,server,starterAdded[server]+1)
        starterQueues=bridge.replaceEntry(starterQueues,server,starterQueues[server].slice(1))
        nextDefault(server)
      } else {
        busy=bridge.replaceEntry(busy,server,false)
        starterQueues=bridge.replaceEntry(starterQueues,server,undefined)
        feedback=bridge.replaceEntry(feedback,server,"Added " + starterAdded[server] + " starter sounds. " + error)
        refresh(server,true)
      }
    } else if (action.action === "catalog") {
      if (action.revision !== revision) return
      loading=bridge.replaceEntry(loading, server, false)
      if (message.ok) catalogs=bridge.replaceEntry(catalogs, server, value.sounds || [])
      else feedback=bridge.replaceEntry(feedback, server, error)
    } else if (action.action === "soundboard_upload" || action.action === "soundboard_remove") {
      busy=bridge.replaceEntry(busy, server, false)
      feedback=bridge.replaceEntry(feedback, server, message.ok ? "Sound library updated." : error)
      if (message.ok) {
        if (action.action === "soundboard_upload") uploaded(server)
        refresh(server, true)
      }
    } else {
      if (action.action === "status") statusPending=false
      if (action.serial !== playSerial) return
      if (action.action === "play") {
        pending=false
        feedback=bridge.replaceEntry(feedback, server, message.ok ? (value.previewing ? "Playing privately on your device." : "Playing for you and your voice room.") : error)
      }
      if (message.ok) {
        if (value.error && playServer) feedback=bridge.replaceEntry(feedback,playServer,String(value.error))
        else if (action.action === "status" && (playing || previewing) && !value.playing && !value.previewing && playServer)
          feedback=bridge.replaceEntry(feedback,playServer,"Playback finished.")
        playing=!!value.playing; previewing=!!value.previewing
      }
    }
  }
  Timer {
    interval: 250; repeat: true
    running: root.bridge.daemonConnected && (root.playing || root.previewing || root.pending)
    onTriggered: root.syncPlayback()
  }
}
