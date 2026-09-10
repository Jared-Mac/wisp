import QtQuick

Item {
  id: root
  required property var bridge
  property var replies: ({})
  property var pinStates: ({})
  property var loadedMessages: ({})
  property string feedback: ""
  signal replyStarted(string conversationId)
  signal messageLocated(string conversationId, string messageId)
  signal forwardFinished(string requestId, bool ok, string error)
  function canonical(id) { return String((bridge.conversationById(id) || {}).id || id) }
  function replyFor(id) { return replies[canonical(id)] || null }
  function preview(message) {
    if (!message) return "Message unavailable"
    if (message.content_type === "text/plain") return String(message.payload || "")
    var payload=message.payload || {}, caption=String(payload.caption || "")
    return (message.content_type === "image/png" ? "Image" : String(payload.file_name || "File")) + (caption ? " · " + caption : "")
  }
  function beginReply(id, message) {
    id=canonical(id)
    if (bridge.conversationValue(bridge.sendingConversations,id,false) || (bridge.conversationById(id) || {}).pending_access) return
    replies=bridge.replaceEntry(replies,id,{message_id:String(message.id),sender_name:String(message.sender.display_name || "Unknown"),preview:preview(message).slice(0,240)})
    replyStarted(id)
  }
  function cancelReply(id) { replies=bridge.replaceEntry(replies,canonical(id),undefined) }
  function pinsFor(id) { return pinStates[canonical(id)] || {messages:[],loading:false,ready:false,error:""} }
  function isPinned(id, messageId) { return pinsFor(id).messages.some(function(m) { return String(m.id)===String(messageId) }) }
  function canPin(id) {
    var c=bridge.conversationById(id)
    if (!c || c.pending_access) return false
    if (!c.spot_id && !c.server_channel) return true
    var state=bridge.serverStates.filter(function(s){return String(s.server.id)===String(c.server_id)})[0]
    return !!state && !!(state.self.server_admin || state.self.server_owner)
  }
  function request(action, name, args) {
    var id=bridge.send(name,args)
    if (id) bridge.requests[id]=Object.assign({kind:"messageAction",action:action},args)
    return id || ""
  }
  function loadPins(id) {
    id=canonical(id)
    if (!id || (bridge.conversationById(id) || {}).pending_access || pinsFor(id).loading) return
    var requestId=request("pins","list_pins",Object.assign(bridge.withConversationScope(id),{canonicalId:id}))
    if (requestId) pinStates=bridge.replaceEntry(pinStates,id,Object.assign({},pinsFor(id),{loading:true,error:"",requestId:requestId}))
  }
  function disconnected() {
    pinStates=({}); loadedMessages=({})
    Object.keys(bridge.requests).forEach(function(id) {
      if (bridge.requests[id].kind==="messageAction") bridge.finishRequest({id:id,ok:false,error:{message:"Connection lost. Check the chat before retrying."}})
    })
  }
  function invalidate() {
    loadedMessages=({})
    var ids=Object.keys(pinStates)
    pinStates=({})
    ids.forEach(loadPins)
  }
  function setPin(id, messageId, pinned) {
    id=canonical(id)
    var requestId=request("pin","set_message_pin",Object.assign(bridge.withConversationScope(id),{message_id:String(messageId),pinned:pinned,canonicalId:id}))
    return requestId
  }
  function locate(id, messageId) {
    id=canonical(id)
    feedback=""
    request("locate","load_message",Object.assign(bridge.withConversationScope(id),{message_id:String(messageId),canonicalId:id}))
  }
  function forward(message, destination) {
    return request("forward","forward_message",Object.assign({},destination,{source_server_id:String(message.server_id),message_id:String(message.id)}))
  }
  function remember(serverId, message) {
    var server=bridge.servers.filter(function(s){return String(s.id)===String(serverId)})[0] || {id:serverId,name:""}
    var scoped=bridge.scopedMessage(server,message)
    loadedMessages=bridge.replaceEntry(loadedMessages,String(serverId)+"::"+String(message.id),scoped)
    return scoped
  }
  function finish(message, action) {
    var error=message.error ? String(message.error.message || "Message action failed") : ""
    if (action.action === "pins") {
      if (pinsFor(action.canonicalId).requestId!==String(message.id)) return
      var value=message.value || {}, server=bridge.servers.filter(function(s){return String(s.id)===String(action.server_id)})[0] || {id:action.server_id,name:""}
      pinStates=bridge.replaceEntry(pinStates,action.canonicalId,{ready:!!message.ok,loading:false,error:error,can_manage:!!value.can_manage,
        messages:message.ok ? (value.messages || []).map(function(m){return bridge.scopedMessage(server,m)}) : []})
    } else if (action.action === "pin") {
      if (message.ok) { feedback=action.pinned ? "Message pinned" : "Message unpinned"; pinStates=bridge.replaceEntry(pinStates,action.canonicalId,undefined); loadPins(action.canonicalId) }
      else feedback=error
    } else if (action.action === "locate") {
      if (message.ok) { remember(action.server_id,message.value); messageLocated(action.canonicalId,String(message.value.id)) }
      else feedback=error || "Original message is unavailable"
    } else if (action.action === "forward") {
      if (message.ok) feedback="Message forwarded"
      forwardFinished(String(message.id),!!message.ok,error)
    }
    if (feedback) feedbackTimer.restart()
  }
  Timer { id: feedbackTimer; interval: 5000; onTriggered: root.feedback="" }
}
