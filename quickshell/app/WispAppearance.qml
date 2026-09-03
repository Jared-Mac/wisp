import QtQuick
import Quickshell
import Quickshell.Io
import "AppearanceLogic.js" as AppearanceLogic

Item {
  id: root
  visible: false
  property string environment: Quickshell.env("WISP_APPEARANCE_ENVIRONMENT") || "unknown"
  readonly property string profile: {
    var requested = "legacy"
    try { requested = JSON.parse(settings.text()).profile || "legacy" } catch (error) {}
    return AppearanceLogic.selectProfile(requested, environment)
  }
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/appearance.json"
    blockLoading: true
    printErrors: false
    watchChanges: true
    onFileChanged: reload()
  }
}
