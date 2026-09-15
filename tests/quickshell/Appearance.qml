import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  function check(ok, message) { if (!ok) console.error("APPEARANCE_FAILED " + message) }
  Wisp.WispAppearance { id: appearance; managed: Quickshell.env("WISP_TEST_MANAGED") === "1" }
  Wisp.WispTheme { id: theme; profile: appearance.profile; appearanceController: appearance }
  Wisp.WispTheme { id: popup; profile: appearance.profile; appearanceController: appearance }
  Timer {
    running: true; interval: 150
    onTriggered: {
      var expected = Quickshell.env("WISP_EXPECT_APPEARANCE")
      test.check(appearance.chatLayout === (Quickshell.env("WISP_TEST_RELOAD")==="1" ? "soft_groups" : "grouped"), "chat layout defaults and persistence")
      test.check(appearance.profile === expected, "migration profile " + appearance.profile + " expected " + expected)
      test.check(appearance.palette === Quickshell.env("WISP_EXPECT_PALETTE"), "migration palette " + appearance.palette)
      if (Quickshell.env("WISP_TEST_RELOAD") === "1") {
        test.check(appearance.colorOptions.chatBorders && !appearance.colorOptions.chatHeadings
          && !appearance.colorOptions.roomSections && !appearance.colorOptions.friendSections
          && !appearance.colorOptions.friendNames && !appearance.colorOptions.senderNames, "color preferences persisted")
      } else {
        ;["grouped","compact","soft_groups"].forEach(function(layout) {
          appearance.setChatLayout(layout)
          test.check(theme.chatLayout===layout && popup.chatLayout===layout,"chat layouts apply on both surfaces")
        })
        appearance.setChatLayout("invalid")
        test.check(appearance.chatLayout==="soft_groups","invalid chat layout is ignored")
        var profiles = ["soft_graphite","daylight","hearth","performative","clean_tui","herdr","terminal","legacy"]
        var palettes = ["soft_graphite","daylight","hearth","ash_olive","herdr","wisp","graphite","violet","ember","astra"]
        profiles.forEach(function(style) {
          appearance.setProfile(style)
          var geometry = [theme.cornerRadius,theme.fontFamily,theme.bodySize,theme.titleSize,theme.tui,theme.cleanTui,theme.performative].join("|")
          palettes.forEach(function(palette) {
            appearance.setPalette(palette)
            test.check(theme.profile === (appearance.managed ? "legacy" : style), "independent profile " + style + "/" + palette)
            test.check(theme.paletteName === (appearance.managed ? "wisp" : palette), "independent palette " + palette)
            test.check([theme.cornerRadius,theme.fontFamily,theme.bodySize,theme.titleSize,theme.tui,theme.cleanTui,theme.performative].join("|") === geometry, "palette must not change structure or font")
            test.check(popup.background === theme.background && popup.profile === theme.profile, "both surfaces agree")
            if (!appearance.managed && palette === "ash_olive") test.check(theme.background == "#000000" && theme.foreground == "#d3d5cf" && theme.accent == "#a2b586", "original colors retained")
          })
        })
        if (!appearance.managed) {
          appearance.setProfile("terminal"); appearance.setPalette("herdr")
          appearance.setProfile("soft_graphite"); appearance.setPalette("violet")
          appearance.setProfile("daylight"); appearance.setPalette("daylight")
          appearance.setProfile("hearth"); appearance.setPalette("hearth")
          appearance.setProfile("soft_graphite")
          test.check(appearance.palette === "violet", "concept remembers customized palette")
          appearance.setProfile("terminal")
          test.check(appearance.palette === "herdr", "returning to terminal restores previous palette")
        }
        appearance.setProfile("clean_tui"); appearance.setPalette("ash_olive")
        var keys = ["chatBorders","chatHeadings","roomSections","friendSections","friendNames","senderNames"]
        if (!appearance.managed) {
          keys.forEach(function(key) { appearance.setColorOption(key,false) })
          test.check(!theme.chatBordersColored && !theme.chatHeadingsColored && theme.roomSectionColor === theme.muted && theme.friendSectionColor === theme.muted, "all neutral")
          keys.forEach(function(key) {
            appearance.setColorOption(key,true)
            keys.forEach(function(other) { test.check(theme.colorEnabled(other) === (other === key), "independent color toggle " + key + "/" + other) })
            appearance.setColorOption(key,false)
          })
          appearance.setColorOption("chatBorders",true)
          test.check(theme.cleanTui && theme.chatBordersColored && !theme.chatHeadingsColored && theme.roomSectionColor === theme.muted && theme.friendSectionColor === theme.muted, "Clean TUI supports chat-only colors")
        }
      }
      test.check(appearance.error === "", "preferences saved")
      console.log("APPEARANCE_OK")
      Qt.quit()
    }
  }
}
