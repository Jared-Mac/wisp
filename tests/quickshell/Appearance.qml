import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  Wisp.WispAppearance { id: appearance }
  Wisp.WispTheme { id: theme; profile: appearance.profile; appearanceController: appearance }
  Wisp.WispTheme { id: popupTheme; profile: appearance.profile; appearanceController: appearance }
  Timer {
    running: true; interval: 200
    onTriggered: {
      if (Quickshell.env("WISP_PALETTE_PERSIST")) {
        var color = Quickshell.env("WISP_PALETTE_PERSIST")
        if (Quickshell.env("WISP_PALETTE_RELOAD") === "1") {
          if (appearance.palette !== color || popupTheme.paletteName !== color)
            console.error("APPEARANCE_FAILED palette reload")
        } else appearance.setPalette(color)
        console.log("PALETTE_PERSIST_OK")
        Qt.quit()
        return
      }
      var expected = Quickshell.env("WISP_EXPECT_APPEARANCE")
      if (theme.profile !== expected || theme.background != "#151821" || theme.accent != "#2f8cff" || theme.accentText != "#ffffff"
          || theme.cornerRadius !== (expected === "legacy" ? 9 : 2)
          || theme.bodySize !== (expected === "legacy" ? 14 : 13))
        console.error("APPEARANCE_FAILED " + theme.profile)
      else console.log("APPEARANCE_OK " + theme.profile + " " + theme.font.family)
      var originalProfile = appearance.profile
      for (var i = 0; i < 4; i++) {
        var name = ["wisp", "graphite", "violet", "ember"][i]
        appearance.setPalette(name)
        if (appearance.profile !== originalProfile || theme.paletteName !== (appearance.managed ? "wisp" : name)
            || popupTheme.accent !== theme.accent || theme.danger != "#ff7777")
          console.error("APPEARANCE_FAILED palette " + name)
      }
      appearance.setPalette("wisp")
      if (Quickshell.env("WISP_TEST_CHANGE")) appearance.setProfile(Quickshell.env("WISP_TEST_CHANGE"))
      else Qt.quit()
    }
  }
  Timer {
    running: !!Quickshell.env("WISP_TEST_CHANGE"); interval: 450
    onTriggered: {
      var expected = Quickshell.env("WISP_EXPECT_CHANGED")
      if (theme.profile !== expected || popupTheme.profile !== expected || appearance.error !== "")
        console.error("APPEARANCE_FAILED switch " + theme.profile + " " + appearance.error)
      else console.log("APPEARANCE_SWITCH_OK")
      Qt.quit()
    }
  }
}
