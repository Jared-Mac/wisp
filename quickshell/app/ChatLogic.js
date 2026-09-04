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

function incomingConversationIds(previous, next, eventName) {
  if (!previous || eventName !== "message_created") return []
  var known = {}, result = [], selfId = (next.self || {}).id
  ;(previous.messages || []).forEach(function(m) { known[String(m.id)] = true })
  ;(next.messages || []).forEach(function(m) {
    var id = String(m.conversation_id || "")
    if (!known[String(m.id)] && m.sender.id !== selfId && id && result.indexOf(id) < 0) result.push(id)
  })
  return result
}
function shouldNotifyChat(id, focusedId, appFocused, policy, mutedChats, muted, volume) {
  if (muted || Number(volume) <= 0 || mutedChats.indexOf(String(id)) >= 0) return false
  if (policy === "always") return true
  if (policy === "unfocused") return !appFocused
  return String(id) !== focusedId
}

function roomSoundEvents(previous, next, eventName) {
  if (!previous || !eventName || ["snapshot", "server_reconnected"].indexOf(eventName) >= 0) return []
  var before = String((previous.self || {}).hangout_id || ""), after = String((next.self || {}).hangout_id || "")
  if (before !== after) {
    var result = []
    if (before) result.push("self_leave")
    if (after) result.push("self_join")
    return result
  }
  if (!after) return []
  function members(snapshot) {
    var room = (snapshot.hangouts || []).filter(function(h) { return String(h.id) === after })[0]
    return room ? (room.members || []).map(function(m) { return String(m.id) }).filter(function(id) { return id !== String((next.self || {}).id) }) : []
  }
  var oldMembers = members(previous), newMembers = members(next), events = []
  if (oldMembers.some(function(id) { return newMembers.indexOf(id) < 0 })) events.push("member_leave")
  if (newMembers.some(function(id) { return oldMembers.indexOf(id) < 0 })) events.push("member_join")
  return events
}
