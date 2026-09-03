import QtQuick

// Wisp-owned design tokens. The standalone application uses these defaults;
// shell integrations can override them without making shared controls import
// private modules from that shell.
QtObject {
  id: root

  // Legacy is deliberately the default, including direct/unknown launches.
  property string profile: "legacy"
  readonly property bool terminal: profile === "terminal-experimental"
  readonly property string monospaceFamily: {
    var available = Qt.fontFamilies()
    var candidates = ["Hack", "DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono", "Adwaita Mono"]
    for (var i = 0; i < candidates.length; i++)
      if (available.indexOf(candidates[i]) >= 0) return candidates[i]
    return "monospace" // Qt/fontconfig's generic fixed-width family, not a CSS list.
  }

  property color foreground: "#e8ecf3"
  property color background: "#151821"
  property color surface: "#1c202b"
  property color accent: "#2f8cff"
  property color muted: "#8d96a8"
  property color danger: "#ff7777"
  property color warning: "#f5b94c"

  property int cornerRadius: terminal ? 4 : 9
  property real spacingScale: 1.0
  property string fontFamily: terminal ? monospaceFamily : "sans-serif"
  property int captionSize: 12
  property int bodySize: terminal ? 13 : 14
  property int titleSize: terminal ? 16 : 18
  readonly property color separator: alpha(foreground, 0.10)
  readonly property color focusBorder: alpha(accent, 0.85)

  function space(px) {
    var value = Number(px)
    if (!isFinite(value) || value <= 0) return 0
    return Math.max(1, Math.round(value * spacingScale))
  }

  function alpha(color, opacity) {
    return Qt.rgba(color.r, color.g, color.b, opacity)
  }

  readonly property QtObject spacing: QtObject {
    readonly property int xs: root.space(3)
    readonly property int sm: root.space(4)
    readonly property int md: root.space(6)
    readonly property int lg: root.space(8)
    readonly property int xl: root.space(10)
    readonly property int xxl: root.space(12)
    readonly property int huge: root.space(18)
  }

  readonly property QtObject font: QtObject {
    readonly property string family: root.fontFamily
    readonly property int caption: root.captionSize
    readonly property int body: root.bodySize
    readonly property int title: root.titleSize
  }
}
