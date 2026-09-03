import QtQuick

// Conditional overrides restore the control's original bindings when disabled.
// No global Qt style, application font, or host-provided Omarchy palette changes.
QtObject {
  id: root
  required property var theme
  required property var control
  property list<Binding> overrides: [
    Binding { target: root.control; property: "palette.window"; value: root.theme.surface; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.base"; value: root.theme.background; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.button"; value: root.theme.surface; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.text"; value: root.theme.foreground; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.windowText"; value: root.theme.foreground; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.buttonText"; value: root.theme.foreground; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlight"; value: root.theme.accent; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.highlightedText"; value: root.theme.foreground; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue },
    Binding { target: root.control; property: "palette.placeholderText"; value: root.theme.muted; when: root.theme.terminal; restoreMode: Binding.RestoreBindingOrValue }
  ]
}
