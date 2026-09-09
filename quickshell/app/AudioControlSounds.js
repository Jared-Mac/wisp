function event(previous, next, eventName) {
  if (!previous || !next || eventName !== "self_state_changed" || previous.id !== next.id) return ""
  if (!!previous.deafened !== !!next.deafened) return next.deafened ? "audio_deafen" : "audio_undeafen"
  if (!!previous.muted !== !!next.muted) return next.muted ? "audio_mute" : "audio_unmute"
  return ""
}
