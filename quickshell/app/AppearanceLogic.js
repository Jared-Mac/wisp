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
  if (requested === "legacy" || requested === "classic") return "legacy"
  if (requested === "terminal" || requested === "terminal-experimental") return "terminal"
  if (requested === "clean_tui" || requested === "clean-tui") return "clean_tui"
  return environment === "cachyos" || environment === "desktop" || environment === "omarchy"
    ? "terminal" : "legacy"
}
