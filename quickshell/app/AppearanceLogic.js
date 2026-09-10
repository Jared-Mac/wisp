.pragma library

function isFriendly(profile) { return ["soft_graphite", "daylight", "hearth"].indexOf(profile) >= 0 }
function presetPalette(profile) { return isFriendly(profile) ? profile : "" }

function defaultPalette(requestedProfile, environment, managed) {
  if (managed) return "wisp"
  if (isFriendly(requestedProfile)) return presetPalette(requestedProfile)
  if (requestedProfile === "performative") return "ash_olive"
  if (requestedProfile === "herdr") return "herdr"
  return "wisp"
}

// Themes select presentation only; no permissions, features, or data depend on them.
function selectProfile(requested, environment, managed) {
  if (managed) return "legacy"
  if (isFriendly(requested)) return requested
  if (requested === "performative" || requested === "herdr") return requested
  if (requested === "legacy" || requested === "classic") return "legacy"
  if (requested === "terminal" || requested === "terminal-experimental") return "terminal"
  if (requested === "clean_tui" || requested === "clean-tui") return "clean_tui"
  return "legacy"
}

function resolve(profile, palette, version, environment, managed) {
  if (managed) return {profile:"legacy", palette:"wisp"}
  var style = selectProfile(profile, environment, false)
  var color = ["wisp","graphite","violet","ember","performative","ash_olive","herdr","soft_graphite","daylight","hearth","astra"].indexOf(palette) >= 0
    ? palette : defaultPalette(profile, environment, false)
  // Old palettes also selected a layout, except that Clean TUI took precedence.
  if (version < 2 && !isFriendly(style) && style !== "clean_tui" && (color === "performative" || color === "herdr")) style = color
  if (color === "performative") color = "ash_olive"
  return {profile:style, palette:color}
}
