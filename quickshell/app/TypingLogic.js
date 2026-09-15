// Only keystroke activity renews this lease; a saved draft never does.
var timeoutMs = 8000
var pulseMs = 3000

function receive(entries, event, now, selfId) {
  var next = Object.assign({}, entries), user = String(event.user_id || "")
  if (!user || user === String(selfId || "")) return next
  if (event.active !== true) delete next[user]
  else next[user] = {name:String(event.display_name || "Someone"),expires:now + Math.max(0,Math.min(timeoutMs,Number(event.timeout_ms) || timeoutMs))}
  return prune(next, now)
}
function prune(entries, now) {
  var next = {}
  Object.keys(entries || {}).forEach(function(id) { if (entries[id].expires > now) next[id] = entries[id] })
  return next
}
function label(entries) {
  var names = Object.keys(entries || {}).map(function(id) { return entries[id].name })
  if (!names.length) return ""
  if (names.length === 1) return names[0] + " is typing…"
  if (names.length === 2) return names[0] + " and " + names[1] + " are typing…"
  return names[0] + ", " + names[1] + " and " + (names.length - 2) + " others are typing…"
}
