import QtQuick

// Session-only visual boundaries. Server read counts may change while a person
// is catching up; they must not erase the place where that visit began.
Item {
  id: root
  required property var bridge
  property var boundaries: ({})
  property var acknowledged: ({})
  function locallyRead(id) { var c=bridge.conversationById(id); return !!c && !!c.last_message && acknowledged[String(c.id)]===String(c.last_message.id) }
  property var readers: ({})
  property int serial: 0
  function canonical(id) { return String((bridge.conversationById(id) || {}).id || id) }
  function boundary(id) { return boundaries[canonical(id)] || null }
  function pending(id) { var b=boundary(id); return !!b && !b.seen }
  function isReading(id) {
    id=canonical(id)
    return Object.keys(readers).some(function(key) { var r=readers[key]; return r.id===id && r.focused && r.ready })
  }
  function report(key, id, focused, ready) {
    id=canonical(id)
    var previous=readers[key]
    readers=bridge.replaceEntry(readers,key,id ? {id:id,focused:focused,ready:ready} : undefined)
    if (previous && previous.focused && (!focused || previous.id!==id)) {
      var old=boundaries[previous.id]
      if (old && old.seen && !Object.keys(readers).some(function(k) { return readers[k].id===previous.id && readers[k].focused }))
        boundaries=bridge.replaceEntry(boundaries,previous.id,undefined)
    }
    if (focused && ready) {
      var b=boundaries[id]
      if (b && !b.seen) boundaries=bridge.replaceEntry(boundaries,id,Object.assign({},b,{seen:true}))
      bridge.markVisibleConversationRead(id)
    }
  }
  function observe(previous, next, eventName, snapshot) {
    var updated=Object.assign({},boundaries), known={}, selfIds={}
    ;(previous ? previous.messages : []).forEach(function(m) { known[String(m.server_id)+"::"+String(m.id)]=true })
    ;(snapshot.server_states || []).forEach(function(s) { selfIds[String(s.server.id)]=String((s.self || {}).id || "") })
    var available={}
    next.conversations.forEach(function(c) {
      var id=String(c.id); available[id]=true
      var selfId=selfIds[String(c.server_id)] || String((snapshot.self || {}).id || "")
      var messages=next.messages.filter(function(m) { return String(m.conversation_id)===id })
        .sort(function(a,b) { return String(a.created_at).localeCompare(String(b.created_at)) || String(a.id).localeCompare(String(b.id)) })
      var remote=messages.filter(function(m) { return String((m.sender || {}).id)!==selfId })
      var newMessages=previous && eventName==="message_created" ? remote.filter(function(m) { return !known[String(m.server_id)+"::"+String(m.id)] }) : []
      var b=updated[id]
      if (!b) {
        var candidates=[]
        if (newMessages.length && !root.isReading(id)) candidates=newMessages
        // Seed offline/reconnect history from the server's non-self unread count.
        if (!candidates.length && Number(c.unread_count)>0 && !root.isReading(id) && acknowledged[id]!==String((c.last_message || {}).id)) candidates=remote.slice(-Number(c.unread_count))
        if (candidates.length) b={firstId:String(candidates[0].id),createdAt:String(candidates[0].created_at),seen:false}
      } else if (newMessages.length && !root.isReading(id)) b=Object.assign({},b,{seen:false})
      if (b && !messages.some(function(m) { return String(m.id)===b.firstId })) {
        var following=remote.filter(function(m) { return String(m.created_at)>b.createdAt || (String(m.created_at)===b.createdAt && String(m.id)>b.firstId) })
        b=following.length ? Object.assign({},b,{firstId:String(following[0].id),createdAt:String(following[0].created_at)}) : null
      }
      // A read from another frontend/device must clear this frontend's badge,
      // while retaining the divider if someone is still visiting this chat.
      if (b && Number(c.unread_count)===0 && previous && previous.conversations.some(function(old) { return String(old.id)===id && Number(old.unread_count)>0 })) {
        var visiting=Object.keys(readers).some(function(key) { return readers[key].id===id && readers[key].focused })
        b=visiting ? Object.assign({},b,{seen:true}) : null
      }
      if (b) updated[id]=b; else delete updated[id]
    })
    Object.keys(updated).forEach(function(id) { if (!available[id]) delete updated[id] })
    if (JSON.stringify(updated)!==JSON.stringify(boundaries)) boundaries=updated
  }
}
