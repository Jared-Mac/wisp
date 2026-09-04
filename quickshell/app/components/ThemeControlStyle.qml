import QtQuick

// Terminal's styling restores the original Classic bindings when disabled.
// No global Qt style, application font, or host-provided Omarchy palette changes.
QtObject {
  id: root
  required property var theme
  required property var control
  property bool outline: false
  property bool menuOutline: false
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
    Binding { target: root.control; property: "padding"; value: root.theme.space(4); when: root.menuOutline && !root.theme.hostManaged; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.window"; value: root.theme.surface; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.base"; value: root.theme.background; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.button"; value: root.theme.surface; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.text"; value: root.theme.foreground; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.windowText"; value: root.theme.foreground; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.buttonText"; value: root.theme.foreground; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlight"; value: root.theme.accent; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlightedText"; value: root.theme.accentText; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.placeholderText"; value: root.theme.muted; when: root.theme.terminal || root.theme.customPalette; restoreMode: Binding.RestoreBindingOrValue }
  ]
}
