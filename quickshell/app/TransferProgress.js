// Keep a fixed-size, short-lived projection of progress events. Never retain
// arbitrary payload fields or use a peer-provided timestamp for expiry.
var maxEntries = 128
var idleTimeoutMs = 5 * 60 * 1000
var completedTimeoutMs = 5000

function validCount(value) {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function prune(entries, now) {
  var next = {}
  Object.keys(entries).forEach(function(key) {
    if (entries[key].expires > now) next[key] = entries[key]
  })
  return next
}

function receive(entries, event, now) {
  var next = prune(entries, now)
  if (!event || typeof event.id !== "string" || !event.id.length || event.id.length > 256
      || (event.direction !== "upload" && event.direction !== "download")
      || !validCount(event.bytes) || !validCount(event.total) || event.bytes > event.total)
    return next

  var key = event.direction + ":" + event.id
  var previous = next[key]
  var complete = event.bytes === event.total
  var expires = now + (complete ? completedTimeoutMs : idleTimeoutMs)
  // Repeated completion events must not keep extending a completed entry.
  if (complete && previous && previous.bytes === previous.total)
    expires = Math.min(expires, previous.expires)
  delete next[key]
  // Object insertion order tracks updates even when events share a timestamp.
  var keys = Object.keys(next)
  while (keys.length >= maxEntries) delete next[keys.shift()]
  next[key] = {bytes:event.bytes, total:event.total, expires:expires}
  return next
}
