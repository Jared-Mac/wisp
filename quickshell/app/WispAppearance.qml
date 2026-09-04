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
  readonly property string profile: AppearanceLogic.selectProfile(preferences.profile, environment, managed)
  readonly property string palette: managed ? "wisp" : validPalette(preferences.palette) ? preferences.palette : AppearanceLogic.defaultPalette(preferences.profile, environment, managed)
  property string error: ""
  signal settingsSaved()
  signal settingsSaveFailed()
  function validPalette(value) { return ["wisp", "graphite", "violet", "ember", "performative", "herdr"].indexOf(value) >= 0 }
  function setPalette(value) {
    if (managed || !validPalette(value) || value === palette) return
    error = ""
    preferences.palette = value
    settings.writeAdapter()
  }
  function setProfile(value) {
    if (managed || ["terminal", "legacy", "clean_tui"].indexOf(value) < 0 || value === profile) return
    error = ""
    preferences.profile = value
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
    }
  }
}
