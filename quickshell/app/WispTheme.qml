import QtQuick

// Wisp-owned design tokens. The standalone application uses these defaults;
// shell integrations can override them without making shared controls import
// private modules from that shell.
QtObject {
  id: root

  // Hosts select the default through WispAppearance; adapters keep their styling.
  property string profile: "legacy"
  property var appearanceController: null
  // Host adapters can opt into the compact TUI structure while continuing to
  // supply their own palette, typography, scale, and corner treatment.
  property bool tuiTreatment: false
  readonly property bool hostManaged: !!appearanceController && appearanceController.managed
  readonly property bool cleanTui: profile === "clean_tui" || profile === "clean-tui"
  readonly property bool terminal: tui || profile === "terminal" || profile === "terminal-experimental"
  readonly property string monospaceFamily: {
    var available = Qt.fontFamilies()
    var candidates = ["Hack", "DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono", "Adwaita Mono"]
    for (var i = 0; i < candidates.length; i++)
      if (available.indexOf(candidates[i]) >= 0) return candidates[i]
    return "monospace" // Qt/fontconfig's generic fixed-width family, not a CSS list.
  }
  readonly property string herdrMonospaceFamily: {
    var available = Qt.fontFamilies()
    var candidates = ["JetBrainsMono Nerd Font", "JetBrains Mono", monospaceFamily]
    for (var i = 0; i < candidates.length; i++)
      if (available.indexOf(candidates[i]) >= 0) return candidates[i]
    return monospaceFamily
  }

  readonly property string paletteName: appearanceController ? appearanceController.palette : "wisp"
  readonly property bool customPalette: paletteName !== "wisp"
  readonly property bool performative: profile === "performative"
  readonly property bool herdr: profile === "herdr"
  readonly property bool olivePalette: paletteName === "ash_olive" || paletteName === "performative"
  readonly property bool herdrPalette: paletteName === "herdr"
  function colorEnabled(key) {
    return appearanceController && "colorOptions" in appearanceController
      ? appearanceController.colorOptions[key] : key === "senderNames" || !cleanTui
  }
  readonly property bool chatBordersColored: colorEnabled("chatBorders")
  readonly property bool chatHeadingsColored: colorEnabled("chatHeadings")
  readonly property color roomSectionColor: colorEnabled("roomSections") ? warning : muted
  readonly property color friendSectionColor: colorEnabled("friendSections") ? secondaryAccent : muted
  readonly property bool tui: cleanTui || performative || herdr || tuiTreatment
  readonly property var colors: {
    switch (paletteName) {
    // Black terminal canvas, restrained olive accents, and ash inverse selections.
    case "performative":
    case "ash_olive": return {background:"#000000", surface:"#000000", accent:"#a2b586", muted:"#92988f"}
    // Herdr's Terminal theme over Owner's current Solarized Japan palette.
    case "herdr": return {background:"#001419", surface:"#001419", accent:"#29a298", muted:"#637981"}
    case "graphite": return {background:"#191b20", surface:"#23262d", accent:"#9bb9df", muted:"#a1a8b4"}
    case "violet": return {background:"#191722", surface:"#24202f", accent:"#b79aff", muted:"#a49bb6"}
    case "ember": return {background:"#211a18", surface:"#2c2421", accent:"#eeb17b", muted:"#b2a299"}
    default: return {background:"#151821", surface:"#1c202b", accent:"#2f8cff", muted:"#8d96a8"}
    }
  }
  property color foreground: herdrPalette ? "#adb7b7" : olivePalette ? "#d3d5cf" : "#e8ecf3"
  property color background: colors.background
  property color surface: colors.surface
  property color accent: colors.accent
  property color muted: colors.muted
  readonly property color accentText: customPalette ? background : "white"
  readonly property color selectionBackground: cleanTui ? alpha(accent, 0.18) : herdrPalette ? "#002c38" : olivePalette ? "#b7baad" : accent
  readonly property color selectionText: cleanTui ? foreground : herdrPalette ? "#fdf5e2" : olivePalette ? background : accentText
  readonly property color statusBackground: cleanTui ? surface : herdrPalette ? "#002c38" : olivePalette ? "#171914" : accent
  readonly property color statusText: cleanTui || herdrPalette || olivePalette ? foreground : background
  readonly property color onlineIndicator: herdrPalette ? "#849900" : olivePalette ? "#79b88a" : "#4bd38a"
  property color danger: herdrPalette ? "#db302d" : olivePalette ? "#d56b75" : "#ff7777"
  property color warning: herdrPalette ? "#b28500" : olivePalette ? "#c9b458" : "#f5b94c"
  readonly property color secondaryAccent: herdrPalette ? "#d23681" : olivePalette ? "#a291d4" : foreground
  readonly property color roomBorder: !colorEnabled("roomSections") ? separator : herdrPalette ? "#b28500" : olivePalette ? "#68613b" : separator
  readonly property color conversationBorder: !chatBordersColored ? separator : herdrPalette ? "#d23681" : olivePalette ? "#70464c" : separator

  property int cornerRadius: cleanTui ? 2 : tui ? 0 : terminal ? 2 : 9
  property real spacingScale: 1.0
  property string fontFamily: terminal ? (herdr ? herdrMonospaceFamily : monospaceFamily) : "sans-serif"
  property int captionSize: 12
  property int bodySize: terminal ? 13 : 14
  property int titleSize: tui ? 14 : terminal ? 16 : 18
  readonly property color separator: cleanTui ? alpha(foreground, 0.14) : herdrPalette ? "#23434a" : olivePalette ? "#34382f" : alpha(foreground, 0.10)
  readonly property color focusBorder: alpha(accent, 0.85)
  readonly property color surfaceBorder: cleanTui ? alpha(foreground, 0.24) : herdrPalette ? "#46636a" : olivePalette ? "#505747" : alpha(muted, 0.72)

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
