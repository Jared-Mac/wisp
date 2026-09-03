.pragma library

// Themes select presentation only; no permissions, features, or data depend on them.
function selectProfile(requested, environment) {
  if (environment === "omarchy") return "legacy"
  if (requested === "legacy" || requested === "classic") return "legacy"
  if (requested === "terminal" || requested === "terminal-experimental") return "terminal"
  return environment === "cachyos" || environment === "desktop" ? "terminal" : "legacy"
}
