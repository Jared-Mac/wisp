import QtQuick

Item {
  id: root
  required property var bridge
  signal actionFinished(string requestId, bool success, string error)
  property var catalogs: ({})
  property string connectionKey: ""
  property bool transportReady: bridge.daemonConnected && bridge.receivedSnapshot && !bridge.updates.preparing
  onTransportReadyChanged: if (transportReady) Qt.callLater(sync)
  readonly property bool busy: Object.keys(catalogs).some(function(id) { return !!catalogs[id].action })
  function state(serverId) { return catalogs[String(serverId)] || {people:[],ready:false,loading:false,action:"",error:"",feedback:""} }
  function put(serverId, patch) { catalogs=bridge.replaceEntry(catalogs,String(serverId),Object.assign({},state(serverId),patch)) }
  function connected(serverId) { return bridge.serverStates.some(function(s) { return String(s.server.id)===String(serverId) && s.server.connected!==false }) }
  function relationship(person) {
    var server=bridge.participantServer(person)
    if (String((server.self || {}).id)===String(person.id)) return "self"
    if ((server.friends || []).some(function(p) { return String(p.id)===String(person.id) })) return "friend"
    var match=state(person.server_id || server.server.id).people.filter(function(p) { return String(p.id)===String(person.id) })[0]
    return match ? match.relationship : "none"
  }
  function sync(eventName) {
    if (!transportReady) return
    var ids=bridge.serverStates.filter(function(s){return s.server.connected!==false}).map(function(s){return String(s.server.id)})
    var key=bridge.serverStates.map(function(s){return String(s.server.id)+":"+String((s.self || {}).id)+":"+String(s.server.connected)+":"+String(s.server_member)}).join("|")
    if (key!==connectionKey) {
      connectionKey=key; catalogs=({}); ids.forEach(refresh)
    } else if (["account_changed","server_membership_changed","friend_requests_changed","friendship_changed","account_profile_changed","server_reconnected"].indexOf(eventName)>=0) {
      ids.forEach(function(id) { refresh(id,true) })
    } else {
      ids.forEach(function(id) { if (state(id).waiting) refresh(id) })
    }
  }
  function ensure(serverId) {
    var current=state(serverId)
    if (!current.ready || current.waiting) refresh(serverId)
  }
  function reset() {
    Object.keys(bridge.requests).forEach(function(id) { if(bridge.requests[id].kind==="friendship") delete bridge.requests[id] })
    catalogs=({}); connectionKey=""
  }
  function refresh(serverId, invalidate) {
    serverId=String(serverId)
    if (!serverId || !connected(serverId)) return
    if (state(serverId).loading || state(serverId).action) { if(invalidate) put(serverId,{dirty:true}); return }
    if (!transportReady) { put(serverId,{waiting:true,error:""}); return }
    var id=bridge.send("list_people",{server_id:serverId})
    if (!id) { put(serverId,{waiting:true,error:""}); return }
    bridge.requests[id]={kind:"friendship",server_id:serverId,action:"list",connectionKey:connectionKey}
    put(serverId,{loading:true,waiting:false,error:"",requestId:id,dirty:false})
  }
  function act(person, action) {
    person=bridge.scopedParticipant(person)
    var serverId=String(person.server_id), previous=relationship(person)
    if (state(serverId).loading || state(serverId).action || !connected(serverId) || previous==="self" || (previous==="friend" && action!=="remove")) return ""
    if (action==="send" && previous!=="none" || action==="accept" && previous!=="incoming" || action==="dismiss" && ["incoming","outgoing"].indexOf(previous)<0) return
    if (action==="remove" && previous!=="friend") return ""
    var commands={remove:"remove_friend",send:"send_friend_request",accept:"accept_friend_request",dismiss:"dismiss_friend_request"}
    if (!commands[action]) return
    var id=bridge.send(commands[action],{server_id:serverId,user_id:String(person.id)})
    if (!id) {put(serverId,{error:"Reconnect to send a friend request."});return}
    bridge.requests[id]={kind:"friendship",server_id:serverId,action:action,user_id:String(person.id),previous:previous,connectionKey:connectionKey}
    put(serverId,{action:action,error:"",feedback:"",requestId:id})
    return id
  }
  function finish(message, request) {
    var serverId=request.server_id, current=state(serverId)
    if (request.connectionKey!==connectionKey || current.requestId!==String(message.id)) return
    var dirty=!!current.dirty, patch={loading:false,action:"",requestId:"",dirty:false}
    if (message.ok) {
      patch.people=((message.value || {}).people || []).map(function(p) { return Object.assign({},p,{server_id:serverId}) })
      patch.ready=true; patch.error=""
      if (request.action!=="list") {
        patch.feedbackUser=request.user_id
        var person=patch.people.filter(function(p){return String(p.id)===request.user_id})[0] || {}
        patch.feedback=request.action==="remove" ? "Friend removed" : request.action==="accept" ? "Friend added" : request.action==="dismiss" ? (request.previous==="incoming" ? "Request declined" : "Request cancelled") : person.relationship==="incoming" ? "They've already sent you a request. Choose Accept." : person.relationship==="friend" ? "Already friends" : "Friend request sent"
      }
    } else patch.error=String((message.error || {}).message || "Couldn't load server members. Try again.")
    put(serverId,patch)
    if (request.action!=="list") actionFinished(String(message.id),!!message.ok,patch.error || "")
    if (patch.feedback) feedbackTimer.restart()
    if (dirty) refresh(serverId)
  }
  Timer {
    // Retry only requests that could not be sent; loaded catalogs never poll.
    interval:1000;repeat:true
    running:root.transportReady && Object.keys(root.catalogs).some(function(id) { return root.connected(id) && !!root.state(id).waiting })
    onTriggered:root.sync()
  }
  Timer {
    id:feedbackTimer;interval:6000
    onTriggered:Object.keys(root.catalogs).forEach(function(id) { if(root.state(id).feedback)root.put(id,{feedback:"",feedbackUser:""}) })
  }
}
