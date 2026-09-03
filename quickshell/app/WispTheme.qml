import QtQuick

// Wisp-owned design tokens. The standalone application uses these defaults;
// shell integrations can override them without making shared controls import
// private modules from that shell.
QtObject {
  id: root

  // Hosts select the default through WispAppearance; adapters keep their styling.
  property string profile: "legacy"
  property var appearanceController: null
  readonly property bool terminal: profile === "terminal" || profile === "terminal-experimental"
  readonly property string monospaceFamily: {
    var available = Qt.fontFamilies()
    var candidates = ["Hack", "DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono", "Adwaita Mono"]
    for (var i = 0; i < candidates.length; i++)
      if (available.indexOf(candidates[i]) >= 0) return candidates[i]
    return "monospace" // Qt/fontconfig's generic fixed-width family, not a CSS list.
  }

  readonly property string paletteName: appearanceController ? appearanceController.palette : "wisp"
  readonly property bool customPalette: paletteName !== "wisp"
  readonly property var colors: {
    switch (paletteName) {
    case "graphite": return {background:"#191b20", surface:"#23262d", accent:"#9bb9df", muted:"#a1a8b4"}
    case "violet": return {background:"#191722", surface:"#24202f", accent:"#b79aff", muted:"#a49bb6"}
    case "ember": return {background:"#211a18", surface:"#2c2421", accent:"#eeb17b", muted:"#b2a299"}
    default: return {background:"#151821", surface:"#1c202b", accent:"#2f8cff", muted:"#8d96a8"}
    }
  }
  property color foreground: "#e8ecf3"
  property color background: colors.background
  property color surface: colors.surface
  property color accent: colors.accent
  property color muted: colors.muted
  readonly property color accentText: customPalette ? background : "white"
  property color danger: "#ff7777"
  property color warning: "#f5b94c"

  property int cornerRadius: terminal ? 2 : 9
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
