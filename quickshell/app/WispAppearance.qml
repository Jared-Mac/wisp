import QtQuick
import Quickshell
import Quickshell.Io
import "AppearanceLogic.js" as AppearanceLogic

Item {
  id: root
  visible: false
  property string environment: Quickshell.env("WISP_APPEARANCE_ENVIRONMENT") || "unknown"
  // Appearance ownership belongs to the surface. Embedded host adapters keep
  // this enabled; Wisp-owned standalone windows disable it even when launched
  // from an Omarchy desktop.
  property bool managed: environment === "omarchy"
  readonly property var resolved: AppearanceLogic.resolve(preferences.profile, preferences.palette, preferences.version, environment, managed)
  readonly property string profile: resolved.profile
  readonly property string palette: resolved.palette
  readonly property var colorOptions: {
    var result = {}, saved = preferences.colorOptions || {}
    ;["chatBorders","chatHeadings","roomSections","friendSections","friendNames","senderNames"].forEach(function(key) {
      result[key] = !managed && typeof saved[key] === "boolean" ? saved[key] : key === "senderNames" || (profile !== "clean_tui" && !AppearanceLogic.isFriendly(profile))
    })
    return result
  }
  readonly property string chatLayout: ["grouped","compact","soft_groups"].indexOf(preferences.chatLayout) >= 0 ? preferences.chatLayout : "grouped"
  function setChatLayout(value) {
    if (["grouped","compact","soft_groups"].indexOf(value) < 0) return
    preferences.chatLayout=value; settings.writeAdapter()
  }
  readonly property bool showAvatars: preferences.showAvatars
  function setShowAvatars(value) { preferences.showAvatars = !!value; settings.writeAdapter() }
  property string error: ""
  signal settingsSaved()
  signal settingsSaveFailed()
  function validPalette(value) { return ["wisp", "graphite", "violet", "ember", "ash_olive", "herdr", "soft_graphite", "daylight", "hearth", "astra"].indexOf(value) >= 0 }
  function materialize() {
    // Snapshot the old effective look before changing either independent axis.
    var style = profile, color = palette, options = Object.assign({}, colorOptions)
    preferences.profile = style; preferences.palette = color
    preferences.colorOptions = options; preferences.version = 2
  }
  function setPalette(value) {
    if (value === "performative") value = "ash_olive" // old launch/config alias
    if (managed || !validPalette(value) || value === palette) return
    error = ""
    materialize()
    preferences.palette = value
    settings.writeAdapter()
  }
  function setProfile(value) {
    if (managed || ["terminal", "legacy", "clean_tui", "performative", "herdr", "soft_graphite", "daylight", "hearth"].indexOf(value) < 0 || value === profile) return
    error = ""
    var previous = profile, previousPalette = palette
    materialize()
    // Each concept remembers its palette; returning to a terminal style restores
    // its previous colors instead of inheriting the light concept's palette.
    var saved = Object.assign({}, preferences.stylePalettes)
    saved[AppearanceLogic.isFriendly(previous) ? previous : "traditional"] = previousPalette
    preferences.stylePalettes = saved
    preferences.profile = value
    if (AppearanceLogic.isFriendly(value)) preferences.palette = saved[value] || AppearanceLogic.presetPalette(value)
    else if (AppearanceLogic.isFriendly(previous)) preferences.palette = saved.traditional || "wisp"
    settings.writeAdapter()
  }
  function setColorOption(key, enabled) {
    if (managed || !(key in colorOptions) || colorOptions[key] === enabled) return
    error = ""
    materialize()
    var next = Object.assign({}, preferences.colorOptions); next[key] = !!enabled
    preferences.colorOptions = next
    settings.writeAdapter()
  }
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/appearance.json"
    blockLoading: true
    blockWrites: true
    atomicWrites: true
    printErrors: false
    watchChanges: true
    onFileChanged: reload()
    onSaved: root.settingsSaved()
    onSaveFailed: {
      root.error = "Couldn't save the theme. Check your local configuration permissions."
      root.settingsSaveFailed()
    }
    JsonAdapter {
      id: preferences
      property string profile: ""
      property string palette: ""
      property int version: 0
      property var colorOptions: ({})
      property var stylePalettes: ({})
      property bool showAvatars: true
      property string chatLayout: "grouped"
    }
  }
}
