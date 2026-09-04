.pragma library

function defaultPalette(requestedProfile, environment) {
  // Preserve older saved style-only choices and host/unknown appearances.
  if (["legacy", "classic", "terminal", "terminal-experimental"].indexOf(requestedProfile) >= 0) return "wisp"
  return environment === "cachyos" || environment === "desktop" ? "performative" : "wisp"
}

// Themes select presentation only; no permissions, features, or data depend on them.
function selectProfile(requested, environment) {
  if (environment === "omarchy") return "legacy"
  if (requested === "legacy" || requested === "classic") return "legacy"
  if (requested === "terminal" || requested === "terminal-experimental") return "terminal"
  return environment === "cachyos" || environment === "desktop" ? "terminal" : "legacy"
}
