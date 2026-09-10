import QtQuick
import Quickshell
import "components"

// Normal application host. Compact anchored surfaces live in
// WispPanelWindow.qml and the optional Omarchy Panel.qml adapter.
FloatingWindow {
  id: root

  required property var bridge
  required property var theme
  property bool localPreviewsPoppedOut: false
  readonly property bool chatVisible: visible && content.showingChats
  signal hideRequested()
  signal popOutLocalPreviewsRequested()

  title: "Wisp"
  implicitWidth: theme.space(1180)
  implicitHeight: theme.space(900)
  minimumSize: Qt.size(theme.space(360), theme.space(520))
  color: theme.background

  function reveal() {
    visible = true
    minimized = false
    Qt.callLater(function() {
      content.forceActiveFocus()
      if (root.contentItem && root.contentItem.Window.window)
        root.contentItem.Window.window.requestActivate()
    })
  }

  onVisibleChanged: {
    if (visible) Qt.callLater(function() { content.forceActiveFocus() })
    else content.resetNavigation()
  }
  onClosed: hideRequested()

  WispTheme {
    id: readingTheme
    comfortable: root.theme.profile === "legacy" && !root.theme.hostManaged && !root.theme.tuiTreatment
    profile: root.theme.profile
    appearanceController: root.theme.appearanceController
    tuiTreatment: root.theme.tuiTreatment
    fontFamily: root.theme.font.family
    spacingScale: root.theme.spacingScale
    captionSize: comfortable ? 14 : root.theme.font.caption
    bodySize: comfortable ? 16 : root.theme.font.body
    titleSize: comfortable ? 18 : root.theme.font.title
    foreground: root.theme.foreground
    background: root.theme.background
    surface: root.theme.astraPalette ? root.theme.surface : comfortable ? Qt.tint(root.theme.background, root.theme.alpha(root.theme.foreground, 0.035)) : root.theme.surface
    muted: comfortable && root.theme.herdrPalette ? "#8a9da3" : root.theme.muted
    accent: root.theme.accent
    danger: comfortable && root.theme.herdrPalette ? "#ff7974" : root.theme.danger
    warning: comfortable && root.theme.herdrPalette ? "#d3b556" : root.theme.warning
    cornerRadius: comfortable ? 5 : root.theme.cornerRadius
  }

  Rectangle {
    anchors.fill: parent
    color: root.theme.background
    SurfaceOutline { theme: root.theme; radius: 0 }

    WispContent {
      id: content
      anchors.fill: parent
      bridge: root.bridge
      theme: readingTheme
      logoSource: Qt.resolvedUrl("assets/waveform.svg")
      presentation: "app"
      showCloseButton: true
      dismissOnNavigate: false
      localPreviewsPoppedOut: root.localPreviewsPoppedOut
      onPopOutLocalPreviewsRequested: root.popOutLocalPreviewsRequested()
      onCloseRequested: root.hideRequested()
      onAppRequested: { content.goHome(); root.reveal() }
    }
  }
}
