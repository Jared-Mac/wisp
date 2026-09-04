.pragma library

function defaultPalette(requestedProfile, environment, managed) {
  if (managed) return "wisp"
  // Preserve older saved style-only choices and host/unknown appearances.
  if (["legacy", "classic", "terminal", "terminal-experimental", "clean_tui", "clean-tui"].indexOf(requestedProfile) >= 0) return "wisp"
  return environment === "cachyos" || environment === "desktop" || environment === "omarchy"
    ? "performative" : "wisp"
}

// Themes select presentation only; no permissions, features, or data depend on them.
function selectProfile(requested, environment, managed) {
  if (managed) return "legacy"
  if (requested === "performative" || requested === "herdr") return requested
  if (requested === "legacy" || requested === "classic") return "legacy"
  if (requested === "terminal" || requested === "terminal-experimental") return "terminal"
  if (requested === "clean_tui" || requested === "clean-tui") return "clean_tui"
  return environment === "cachyos" || environment === "desktop" || environment === "omarchy"
    ? "terminal" : "legacy"
}

function resolve(profile, palette, version, environment, managed) {
  if (managed) return {profile:"legacy", palette:"wisp"}
  var style = selectProfile(profile, environment, false)
  var color = ["wisp","graphite","violet","ember","performative","ash_olive","herdr"].indexOf(palette) >= 0
    ? palette : defaultPalette(profile, environment, false)
  // Old palettes also selected a layout, except that Clean TUI took precedence.
  if (version < 2 && style !== "clean_tui" && (color === "performative" || color === "herdr")) style = color
  if (color === "performative") color = "ash_olive"
  return {profile:style, palette:color}
}
