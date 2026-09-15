import QtQuick
import "TypingLogic.js" as Logic

Item {
  id: root
  required property var bridge
  property var incoming: ({})
  property var outgoing: ({})

  function send(id, active) {
    if (!bridge.daemonConnected || bridge.updates.preparing) return
    var scope = bridge.scopeForConversation(id)
    var server = bridge.serverStates.filter(function(s) { return String(s.server.id) === scope.server_id })[0]
    if (!server || !server.server.connected) return
    var request = bridge.send("chat_typing", Object.assign(scope,{active:active}))
    if (request) bridge.requests[request] = {kind:"typing"}
  }
  function edited(id, text) {
    if (!id || (bridge.conversationById(id) || {}).pending_access) return
    if (!String(text || "").trim()) { stop(id); return }
    var now = Date.now(), previous = outgoing[id]
    var next = {lastEdit:now,lastSent:previous ? previous.lastSent : 0}
    if (!previous || now - previous.lastSent >= Logic.pulseMs) {
      send(id,true); next.lastSent=now
    }
    outgoing=bridge.replaceEntry(outgoing,id,next)
  }
  function stop(id) {
    if (!outgoing[id]) return
    outgoing=bridge.replaceEntry(outgoing,id,undefined)
    send(id,false)
  }
  function reset() { incoming=({}); outgoing=({}) }
  function receive(event) {
    var server = bridge.serverStates.filter(function(s) { return String(s.server.id) === String(event.server_id) })[0]
    if (!server || !server.server.connected) return
    var id = bridge.scopedConversationId(event.server_id,event.conversation_id)
    if (!bridge.conversationById(id)) return
    incoming=bridge.replaceEntry(incoming,id,Logic.receive(incoming[id] || {},event,Date.now(),(server.self || {}).id))
  }
  function reconcile() {
    var next={}
    Object.keys(incoming).forEach(function(id) {
      var scope=bridge.scopeForConversation(id)
      var server=bridge.serverStates.filter(function(s) { return String(s.server.id)===scope.server_id })[0]
      if (server && server.server.connected && bridge.conversationById(id)) next[id]=incoming[id]
    })
    incoming=next
    Object.keys(outgoing).forEach(function(id) {
      var scope=bridge.scopeForConversation(id)
      var server=bridge.serverStates.filter(function(s) { return String(s.server.id)===scope.server_id })[0]
      if (!server || !server.server.connected || !bridge.conversationById(id)) outgoing=bridge.replaceEntry(outgoing,id,undefined)
    })
  }
  function label(id) { return Logic.label(incoming[id] || {}) }
  Timer {
    interval: 500; repeat: true
    running: Object.keys(root.incoming).length > 0 || Object.keys(root.outgoing).length > 0
    onTriggered: {
      var now=Date.now(), next={}
      Object.keys(root.incoming).forEach(function(id) {
        var entries=Logic.prune(root.incoming[id],now)
        if (Object.keys(entries).length) next[id]=entries
      })
      root.incoming=next
      Object.keys(root.outgoing).forEach(function(id) {
        if (now-root.outgoing[id].lastEdit>=Logic.timeoutMs) root.stop(id)
      })
    }
  }
}
