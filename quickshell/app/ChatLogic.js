function visibleConversations(conversations, hangouts) {
  var activeHangouts = {}
  ;(hangouts || []).forEach(function(hangout) {
    activeHangouts["hangout:" + String(hangout.id)] = true
  })
  return (conversations || []).filter(function(conversation) {
    if (String(conversation.kind) !== "hangout" || conversation.spot_id)
      return true
    // Temporary call chats are useful while the call is live, or afterward
    // when they contain history. Empty ended calls only clutter the picker.
    return !!conversation.last_message || !!activeHangouts[String(conversation.id)]
  })
}

function reconcileTabs(previous, conversations) {
  var available = {}
  conversations.forEach(function(c) { if (!c.tab_closed) available[String(c.id)] = true })
  var result = previous.filter(function(id) { return !!available[id] })
  conversations.forEach(function(c) {
    var id = String(c.id)
    if (available[id] && result.indexOf(id) < 0) result.push(id)
  })
  return result
}

function hasIncomingMessage(previous, next, eventName) {
  // A reconnect/history load must not replay notification sounds.
  if (!previous || eventName !== "message_created") return false
  var known = {}
  ;(previous.messages || []).forEach(function(m) { known[String(m.id)] = true })
  var selfId = (next.self || {}).id
  return (next.messages || []).some(function(m) {
    return !known[String(m.id)] && m.sender.id !== selfId
  })
}

function shouldPlaySound(incoming, focused, muted, volume) {
  return incoming && !focused && !muted && Number(volume) > 0
}
