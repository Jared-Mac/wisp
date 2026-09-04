import QtQuick
import Quickshell
import Quickshell.Io
import "AppearanceLogic.js" as AppearanceLogic

Item {
  id: root
  visible: false
  property string environment: Quickshell.env("WISP_APPEARANCE_ENVIRONMENT") || "unknown"
  readonly property bool managed: environment === "omarchy"
  readonly property string profile: AppearanceLogic.selectProfile(preferences.profile, environment)
  readonly property string palette: managed ? "wisp" : validPalette(preferences.palette) ? preferences.palette : AppearanceLogic.defaultPalette(preferences.profile, environment)
  property string error: ""
  signal settingsSaved()
  signal settingsSaveFailed()
  function validPalette(value) { return ["wisp", "graphite", "violet", "ember", "performative"].indexOf(value) >= 0 }
  function setPalette(value) {
    if (managed || !validPalette(value) || value === palette) return
    error = ""
    preferences.palette = value
    settings.writeAdapter()
  }
  function setProfile(value) {
    if (managed || (value !== "terminal" && value !== "legacy") || value === profile) return
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
