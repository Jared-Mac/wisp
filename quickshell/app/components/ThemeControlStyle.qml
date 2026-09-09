import QtQuick
import "../ActionIcons.js" as Icons

// Native controls follow the selected style and restore their original bindings when disabled.
// No global Qt style, application font, or host-provided Omarchy palette changes.
QtObject {
  id: root
  required property var theme
  required property var control
  property bool outline: false
  property bool menuOutline: false
  readonly property bool themed: root.theme.terminal || root.theme.customPalette || root.theme.friendly
  property bool supportsIcon: false
  function inspectControl() { supportsIcon = !!control && ("icon" in control) && ("text" in control) }
  Component.onCompleted: inspectControl()
  onControlChanged: inspectControl()
  readonly property bool hasIcon: root.theme.friendly && supportsIcon
  // Lazily inspect native backgrounds only for Wisp-managed popups. Merely
  // reading a host style's background can instantiate it and change its layout.
  property Loader outlineLoader: Loader {
    active: root.outline && !root.theme.hostManaged
    sourceComponent: SurfaceOutline {
      parent: root.control.background
      theme: root.theme
    }
  }
  property list<Binding> overrides: [
    Binding { target: root.control; property: "font.family"; value: root.theme.font.family; when: root.theme.friendly; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "font.pixelSize"; value: root.theme.font.body; when: root.theme.friendly; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.hasIcon ? root.control : null; property: "icon.source"; value: root.hasIcon ? Icons.source(Icons.icon(root.control.text),root.theme.foreground) : ""; when: root.hasIcon; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.hasIcon ? root.control : null; property: "icon.color"; value: root.theme.foreground; when: root.hasIcon; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "padding"; value: root.theme.space(4); when: root.menuOutline && !root.theme.hostManaged; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.window"; value: root.theme.surface; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.base"; value: root.theme.background; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.button"; value: root.theme.surface; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.text"; value: root.theme.foreground; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.windowText"; value: root.theme.foreground; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.buttonText"; value: root.theme.foreground; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlight"; value: root.theme.accent; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlightedText"; value: root.theme.accentText; when: root.themed; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.placeholderText"; value: root.theme.muted; when: root.themed; restoreMode: Binding.RestoreBindingOrValue }
  ]
}
