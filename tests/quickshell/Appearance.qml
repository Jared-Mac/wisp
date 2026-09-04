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
      var defaultPerformative = Quickshell.env("WISP_EXPECT_PALETTE") === "performative"
      if (theme.profile !== expected || theme.paletteName !== (defaultPerformative ? "performative" : "wisp")
          || theme.background != (defaultPerformative ? "#000000" : "#151821") || theme.accent != (defaultPerformative ? "#a2b586" : "#2f8cff")
          || theme.cornerRadius !== (defaultPerformative ? 0 : expected === "legacy" ? 9 : 2)
          || theme.bodySize !== (expected === "legacy" ? 14 : 13))
        console.error("APPEARANCE_FAILED " + theme.profile)
      else console.log("APPEARANCE_OK " + theme.profile + " " + theme.font.family)
      var originalProfile = appearance.profile
      for (var i = 0; i < 5; i++) {
        var name = ["wisp", "graphite", "violet", "ember", "performative"][i]
        appearance.setPalette(name)
        if (appearance.profile !== originalProfile || theme.paletteName !== (appearance.managed ? "wisp" : name)
            || popupTheme.accent !== theme.accent || theme.danger != (name === "performative" && !appearance.managed ? "#d56b75" : "#ff7777"))
          console.error("APPEARANCE_FAILED palette " + name)
        if (name === "performative" && !appearance.managed
            && (theme.background != "#000000" || theme.accent != "#a2b586"
                || theme.foreground != "#d3d5cf" || theme.warning != "#c9b458"
                || !theme.terminal || theme.cornerRadius !== 0 || theme.fontFamily !== theme.monospaceFamily
                || theme.surface != "#000000" || theme.titleSize !== 14
                || theme.roomBorder != "#68613b" || theme.conversationBorder != "#70464c"
                || theme.secondaryAccent != "#a291d4" || popupTheme.surfaceBorder != "#505747"
                || theme.selectionBackground != "#b7baad" || theme.statusBackground != "#171914"
                || theme.onlineIndicator != "#79b88a"))
          console.error("APPEARANCE_FAILED performative tokens")
      }
      appearance.setPalette("wisp")
      if (theme.cornerRadius !== (originalProfile === "legacy" ? 9 : 2)
          || theme.terminal !== (originalProfile !== "legacy"))
        console.error("APPEARANCE_FAILED base style restoration")
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
