import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  Wisp.WispAppearance { id: appearance }
  Wisp.WispTheme { id: theme; profile: appearance.profile }
  Timer {
    running: true; interval: 200
    onTriggered: {
      var expected = Quickshell.env("WISP_EXPECT_APPEARANCE")
      if (theme.profile !== expected || theme.background != "#151821" || theme.accent != "#2f8cff"
          || theme.cornerRadius !== (expected === "legacy" ? 9 : 4)
          || theme.bodySize !== (expected === "legacy" ? 14 : 13))
        console.error("APPEARANCE_FAILED " + theme.profile)
      else console.log("APPEARANCE_OK " + theme.profile + " " + theme.font.family)
      Qt.quit()
    }
  }
}
