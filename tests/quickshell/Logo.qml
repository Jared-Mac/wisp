import QtQuick
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false

  function check(value, message) {
    if (!value) {
      failed = true
      console.error("LOGO_FAILED " + message)
    }
  }

  QtObject {
    id: appearance
    property string palette: "wisp"
    property bool managed: false
  }
  Wisp.WispTheme { id: theme; appearanceController: appearance }

  FloatingWindow {
    visible: true
    implicitWidth: 400
    implicitHeight: 140
    color: theme.background

    Components.WispLogo {
      id: logo
      anchors.centerIn: parent
      width: 320
      height: 70
      theme: theme
    }
  }

  Timer {
    interval: 180
    running: true
    onTriggered: {
      test.check(logo.gridRows === 9 && logo.gridColumns === 40, "stable cell grid")
      test.check(logo.wordColumns === 38, "word geometry")
      test.check(logo.waveformLevels.length === logo.gridColumns, "one amplitude per column")
      test.check(logo.letterCellCount > 120, "letter mask contains cells")
      test.check(logo.waveformCellCount > logo.gridColumns, "waveform has visible amplitude")
      test.check(logo.letterAt(1, 0) && !logo.letterAt(0, 0), "word is inset from the waveform")
      test.check(logo.waveAt(3, 2) && !logo.waveAt(3, 1), "amplitude expands around center")
      test.check(logo.letterColor === theme.foreground && logo.waveMiddleColor === theme.accent,
                 "logo consumes theme tokens")
      appearance.palette = "herdr"
    }
  }

  Timer {
    interval: 360
    running: true
    onTriggered: {
      test.check(String(logo.waveMiddleColor) === "#29a298", "palette change reaches waveform")
      logo.waveformVisible = false
      test.check(!logo.waveAt(3, logo.waveCenterRow), "waveform can be disabled for monochrome use")
      if (!test.failed) console.log("LOGO_OK")
      Qt.quit()
    }
  }
}
