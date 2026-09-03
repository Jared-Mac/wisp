.pragma library

// A saved opt-in alone is insufficient. Unknown and Omarchy launches stay legacy.
function selectProfile(requested, environment) {
  return requested === "terminal-experimental" && environment === "cachyos"
    ? "terminal-experimental" : "legacy"
}
