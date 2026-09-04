import QtQuick

// Wisp-owned design tokens. The standalone application uses these defaults;
// shell integrations can override them without making shared controls import
// private modules from that shell.
QtObject {
  id: root

  // Hosts select the default through WispAppearance; adapters keep their styling.
  property string profile: "legacy"
  property var appearanceController: null
  readonly property bool hostManaged: !!appearanceController && appearanceController.managed
  readonly property bool terminal: performative || profile === "terminal" || profile === "terminal-experimental"
  readonly property string monospaceFamily: {
    var available = Qt.fontFamilies()
    var candidates = ["Hack", "DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono", "Adwaita Mono"]
    for (var i = 0; i < candidates.length; i++)
      if (available.indexOf(candidates[i]) >= 0) return candidates[i]
    return "monospace" // Qt/fontconfig's generic fixed-width family, not a CSS list.
  }

  readonly property string paletteName: appearanceController ? appearanceController.palette : "wisp"
  readonly property bool customPalette: paletteName !== "wisp"
  readonly property bool performative: paletteName === "performative"
  readonly property var colors: {
    switch (paletteName) {
    // Black terminal canvas, restrained olive accents, and ash inverse selections.
    case "performative": return {background:"#000000", surface:"#000000", accent:"#a2b586", muted:"#92988f"}
    case "graphite": return {background:"#191b20", surface:"#23262d", accent:"#9bb9df", muted:"#a1a8b4"}
    case "violet": return {background:"#191722", surface:"#24202f", accent:"#b79aff", muted:"#a49bb6"}
    case "ember": return {background:"#211a18", surface:"#2c2421", accent:"#eeb17b", muted:"#b2a299"}
    default: return {background:"#151821", surface:"#1c202b", accent:"#2f8cff", muted:"#8d96a8"}
    }
  }
  property color foreground: performative ? "#d3d5cf" : "#e8ecf3"
  property color background: colors.background
  property color surface: colors.surface
  property color accent: colors.accent
  property color muted: colors.muted
  readonly property color accentText: customPalette ? background : "white"
  readonly property color selectionBackground: performative ? "#b7baad" : accent
  readonly property color selectionText: performative ? background : accentText
  readonly property color statusBackground: performative ? "#171914" : accent
  readonly property color statusText: performative ? foreground : background
  readonly property color onlineIndicator: performative ? "#79b88a" : "#4bd38a"
  property color danger: performative ? "#d56b75" : "#ff7777"
  property color warning: performative ? "#c9b458" : "#f5b94c"
  readonly property color secondaryAccent: performative ? "#a291d4" : foreground
  readonly property color roomBorder: performative ? "#68613b" : separator
  readonly property color conversationBorder: performative ? "#70464c" : separator

  property int cornerRadius: performative ? 0 : terminal ? 2 : 9
  property real spacingScale: 1.0
  property string fontFamily: terminal ? monospaceFamily : "sans-serif"
  property int captionSize: 12
  property int bodySize: terminal ? 13 : 14
  property int titleSize: performative ? 14 : terminal ? 16 : 18
  readonly property color separator: performative ? "#34382f" : alpha(foreground, 0.10)
  readonly property color focusBorder: alpha(accent, 0.85)
  readonly property color surfaceBorder: performative ? "#505747" : alpha(muted, 0.72)

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
