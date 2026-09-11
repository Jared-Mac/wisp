function key(account, server, room) { return JSON.stringify([String(account),String(server),String(room)]) }
function states(snapshot) {
  if (!snapshot) return []
  return snapshot.server_states && snapshot.server_states.length ? snapshot.server_states
    : [Object.assign({},snapshot,{server:{id:String(snapshot.selected_server_id || "local"),name:"Wisp",connected:true}})]
}
function joins(previous, next, eventName, settings, now, lastAlerts) {
  if (!previous || !next || eventName!=="hangout_changed" || !settings.enabled) return []
  var self=next.self || {}, voiceServer=String(next.voice_server_id || next.selected_server_id || "")
  if (settings.timing==="not_in_voice" && self.hangout_id || settings.timing==="background" && settings.focused) return []
  var before=states(previous), results=[]
  states(next).forEach(function(state) {
    var server=state.server || {}, account=String((state.self || {}).id || "")
    var old=before.filter(function(s){return String((s.server || {}).id)===String(server.id) && String((s.self || {}).id || "")===account})[0]
    if (!old || server.connected===false || (old.server || {}).connected===false || settings.homes.indexOf(String(server.id))<0) return
    var friends=(state.friends || []).map(function(p){return String(p.id)}), oldFriends=(old.friends || []).map(function(p){return String(p.id)})
    ;(state.spots || []).forEach(function(room) {
      var oldRoom=(old.spots || []).filter(function(r){return String(r.id)===String(room.id)})[0]
      if (!oldRoom || !room.active_hangout_id) return
      if (String(server.id)===voiceServer && String(room.active_hangout_id)===String(self.hangout_id || "")) return
      var roomKey=key(account,server.id,room.id)
      if (settings.onlyEmpty && (oldRoom.members || []).length) return
      var oldMembers=(oldRoom.members || []).map(function(p){return String(p.id)})
      var joined=(room.members || []).filter(function(p) {
        var id=String(p.id)
        return id!==account && oldMembers.indexOf(id)<0 && friends.indexOf(id)>=0 && oldFriends.indexOf(id)>=0
      })
      if (!joined.length) return
      var cooldown=Math.max(0,Math.min(120,Number(settings.cooldown)||0))*60000
      if (lastAlerts[roomKey]!==undefined && now-Number(lastAlerts[roomKey])<cooldown) return
      var conversation=(state.conversations || []).filter(function(c){return String(c.spot_id || "")===String(room.id)})[0]
      if (!conversation) return
      results.push({key:roomKey,serverId:String(server.id),accountId:account,roomId:String(room.id),
        conversationId:String(server.id)+"::"+String(conversation.id),roomName:String(room.name || "Room"),serverName:String(server.name || "Wisp"),
        names:joined.map(function(p){return String(p.display_name || "Friend")})})
    })
  })
  return results
}
function escapeMarkup(text) {
  return String(text).replace(/[\u0000-\u001f\u007f]/g," ").replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")
}
function command(notice) {
  var who=notice.names.slice(0,3).join(", ")+(notice.names.length>3 ? " + "+(notice.names.length-3)+" more" : "")
  return ["notify-send","--app-name=Wisp","--icon=dev.wisp","--urgency=normal","--expire-time=8000",
    "--hint=boolean:suppress-sound:true","--action=default=Open room","--",
    "Friend"+(notice.names.length===1 ? "" : "s")+" joined a room",
    escapeMarkup(who.slice(0,200))+" joined "+escapeMarkup(notice.roomName.slice(0,120))+" · "+escapeMarkup(notice.serverName.slice(0,120))]
}
