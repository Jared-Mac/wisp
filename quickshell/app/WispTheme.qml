import QtQuick

// Wisp-owned design tokens. The standalone application uses these defaults;
// shell integrations can override them without making shared controls import
// private modules from that shell.
QtObject {
  id: root
  readonly property string chatLayout: appearanceController ? appearanceController.chatLayout : "grouped"
  readonly property bool showAvatars: !appearanceController || appearanceController.showAvatars !== false

  // Hosts select the default through WispAppearance; adapters keep their styling.
  property string profile: "legacy"
  // Classic window reading treatment; terminal styles and compact hosts stay compact.
  property bool comfortable: false
  property var appearanceController: null
  // Host adapters can opt into the compact TUI structure while continuing to
  // supply their own palette, typography, scale, and corner treatment.
  property bool tuiTreatment: false
  readonly property bool hostManaged: !!appearanceController && appearanceController.managed
  readonly property bool friendly: !hostManaged && ["soft_graphite", "daylight", "hearth"].indexOf(profile) >= 0
  readonly property bool hearth: friendly && profile === "hearth"
  readonly property bool light: paletteName === "daylight"
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
  // Wisp-owned terminal layouts; embedded adapters retain host styling.
  readonly property bool refinedTui: (performative || herdr) && !hostManaged && !tuiTreatment
  readonly property bool olivePalette: paletteName === "ash_olive" || paletteName === "performative"
  readonly property bool herdrPalette: paletteName === "herdr"
  readonly property bool astraPalette: paletteName === "astra"
  function colorEnabled(key) {
    return appearanceController && "colorOptions" in appearanceController
      ? appearanceController.colorOptions[key] : key === "senderNames" || (!cleanTui && !friendly)
  }
  readonly property bool chatBordersColored: colorEnabled("chatBorders")
  readonly property bool chatHeadingsColored: colorEnabled("chatHeadings")
  readonly property color roomSectionColor: colorEnabled("roomSections") ? warning : muted
  readonly property color friendSectionColor: colorEnabled("friendSections") ? secondaryAccent : muted
  readonly property bool tui: cleanTui || performative || herdr || tuiTreatment
  readonly property var colors: {
    switch (paletteName) {
    case "soft_graphite": return {background:"#181e25", surface:"#222b34", accent:"#8ec5ee", muted:"#a8b6c6"}
    case "daylight": return {background:"#f7f6f1", surface:"#fffdf8", accent:"#3e6b5a", muted:"#59665f"}
    case "hearth": return {background:"#24212d", surface:"#302b3c", accent:"#f0b898", muted:"#b8adc8"}
    // Black terminal canvas, restrained olive accents, and ash inverse selections.
    case "performative":
    case "ash_olive": return {background:"#000000", surface:"#000000", accent:"#a2b586", muted:"#92988f"}
    // Herdr's Terminal theme over Owner's current Solarized Japan palette.
    case "herdr": return {background:"#001419", surface:"#001419", accent:"#29a298", muted:"#637981"}
    // Astra: match the installed Omarchy palette, including its raised surface.
    case "astra": return {background:"#0c1224", surface:"#1c2843", accent:"#a397ec", muted:"#8795b5"}
    case "graphite": return {background:"#191b20", surface:"#23262d", accent:"#9bb9df", muted:"#a1a8b4"}
    case "violet": return {background:"#191722", surface:"#24202f", accent:"#b79aff", muted:"#a49bb6"}
    case "ember": return {background:"#211a18", surface:"#2c2421", accent:"#eeb17b", muted:"#b2a299"}
    default: return {background:"#151821", surface:"#1c202b", accent:"#2f8cff", muted:"#8d96a8"}
    }
  }
  property color foreground: light ? "#24372f" : paletteName === "hearth" ? "#f4edf6" : astraPalette ? "#bac7df" : herdrPalette ? "#adb7b7" : olivePalette ? "#d3d5cf" : "#e8ecf3"
  property color background: colors.background
  property color surface: colors.surface
  property color accent: colors.accent
  property color muted: colors.muted
  readonly property color accentText: light ? "#ffffff" : customPalette ? background : "white"
  readonly property color selectionBackground: astraPalette ? "#343e68" : cleanTui ? alpha(accent, 0.18) : herdrPalette ? "#002c38" : olivePalette ? "#b7baad" : accent
  readonly property color selectionText: astraPalette ? "#d6e2f5" : cleanTui ? foreground : herdrPalette ? "#fdf5e2" : olivePalette ? background : accentText
  readonly property color statusBackground: refinedTui ? background : astraPalette ? "#090e1d" : cleanTui ? surface : herdrPalette ? "#002c38" : olivePalette ? "#171914" : accent
  readonly property color statusText: refinedTui ? foreground : astraPalette ? foreground : cleanTui || herdrPalette || olivePalette ? foreground : background
  readonly property color onlineIndicator: light ? "#25804a" : astraPalette ? "#7ebbac" : herdrPalette ? "#849900" : olivePalette ? "#79b88a" : "#4bd38a"
  property color danger: light ? "#bc303c" : astraPalette ? "#da829c" : herdrPalette ? "#db302d" : olivePalette ? "#d56b75" : "#ff7777"
  property color warning: light ? "#88611a" : astraPalette ? "#d6bd80" : herdrPalette ? "#b28500" : olivePalette ? "#c9b458" : "#f5b94c"
  readonly property color secondaryAccent: astraPalette ? "#ba91d9" : herdrPalette ? "#d23681" : olivePalette ? "#a291d4" : foreground
  readonly property color roomBorder: !colorEnabled("roomSections") ? separator : astraPalette ? "#d6bd80" : herdrPalette ? "#b28500" : olivePalette ? "#68613b" : separator
  readonly property color conversationBorder: !chatBordersColored ? separator : astraPalette ? "#a397ec" : herdrPalette ? "#d23681" : olivePalette ? "#70464c" : separator
  readonly property color sidebar: friendly ? (light ? "#eaede5" : surface) : background
  readonly property color panel: friendly ? (paletteName === "hearth" ? "#353041" : light ? surface : paletteName === "soft_graphite" ? "#1d252e" : surface) : surface
  readonly property color controlBackground: alpha(foreground, light ? 0.045 : 0.065)
  readonly property color bubble: alpha(accent, light ? 0.09 : 0.10)

  property int cornerRadius: friendly ? (hearth ? 12 : profile === "daylight" ? 6 : 8) : cleanTui ? 2 : tui ? 0 : terminal ? 2 : 9
  property real spacingScale: 1.0
  property string fontFamily: terminal ? (herdr || refinedTui ? herdrMonospaceFamily : monospaceFamily) : friendly ? "Noto Sans" : "sans-serif"
  property int captionSize: 12
  property int bodySize: terminal ? 13 : 14
  property int titleSize: tui ? 14 : terminal ? 16 : 18
  readonly property color separator: astraPalette ? "#2b385a" : cleanTui ? alpha(foreground, 0.14) : herdrPalette ? "#23434a" : olivePalette ? "#34382f" : alpha(foreground, 0.10)
  readonly property color focusBorder: alpha(accent, 0.85)
  readonly property color surfaceBorder: astraPalette ? "#52628a" : cleanTui ? alpha(foreground, 0.24) : herdrPalette ? "#46636a" : olivePalette ? "#505747" : alpha(muted, 0.72)

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
