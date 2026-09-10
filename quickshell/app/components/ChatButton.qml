import QtQuick
import QtQuick.Controls
import "../ActionIcons.js" as Icons

Button {
  Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
  Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
  id: root
  required property var theme
  property string iconName: Icons.icon(text)
  property bool forceIcon: false
  readonly property bool iconTreatment: theme.friendly || forceIcon
  property bool iconOnly: Icons.symbolOnly(text)
  property bool formatLabel: true
  readonly property string displayText: theme.friendly && formatLabel ? Icons.label(text) : text
  readonly property bool showIcon: iconTreatment && iconName !== ""
  readonly property color labelColor: (theme.comfortable && primary) || (theme.tui && (primary || down)) ? theme.selectionText : primary ? (destructive ? (theme.friendly ? "white" : "#151821") : theme.terminal ? theme.accent : theme.accentText) : destructive ? theme.danger : theme.cleanTui ? (hovered ? theme.foreground : theme.muted) : theme.foreground
  Accessible.name: Icons.accessible(text)
  ToolTip.visible: iconTreatment && iconOnly && (hovered || visualFocus)
  ToolTip.text: Accessible.name
  Binding { target: root; property: "padding"; value: 2; when: root.theme.tui && !root.forceIcon; restoreMode: Binding.RestoreBindingOrValue }
  Binding {target:root;property:"padding";when:root.iconTreatment && Math.min(root.width,root.height)<32;value:Math.max(0,(Math.min(root.width,root.height)-18)/2);restoreMode:Binding.RestoreBindingOrValue}
  property bool primary: false
  property bool quiet: false
  property bool destructive: false
  property int textAlignment: Text.AlignHCenter
  implicitHeight: theme.space(theme.comfortable ? 36 : theme.tui ? 28 : 34)
  implicitWidth: (iconTreatment && iconOnly ? 0 : labelMetrics.advanceWidth) + (showIcon ? theme.space(iconOnly ? 18 : 26) : 0) + theme.space(theme.comfortable ? 20 : theme.tui ? 12 : 24)
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.theme.comfortable ? (root.primary ? root.theme.selectionBackground : root.theme.alpha(root.destructive ? root.theme.danger : root.theme.foreground, root.down ? 0.16 : root.hovered ? 0.10 : root.quiet ? 0 : 0.04)) : root.theme.tui ? (root.primary || root.down ? (root.destructive ? root.theme.danger : root.theme.selectionBackground) : root.hovered ? root.theme.alpha(root.theme.accent, 0.16) : "transparent") : root.primary ? (root.destructive ? root.theme.danger : root.theme.terminal ? root.theme.alpha(root.theme.accent, root.hovered ? 0.24 : 0.12) : root.theme.accent) : root.theme.alpha(root.destructive
      ? root.theme.danger : root.theme.foreground, root.hovered ? 0.14 : 0.06)
    opacity: root.enabled ? 1 : 0.4
    border.width: root.theme.comfortable ? (root.quiet && !root.visualFocus ? 0 : 1) : root.theme.friendly ? 1 : root.theme.tui ? (root.visualFocus ? 1 : 0) : root.theme.terminal ? 1 : 0
    border.color: root.visualFocus || root.primary ? root.theme.focusBorder : root.hovered ? root.theme.muted : root.theme.separator
    Rectangle {
      anchors.fill: parent; radius: parent.radius
      visible: root.theme.terminal && root.down
      color: root.theme.alpha(root.theme.foreground, 0.10)
    }
  }
  TextMetrics {id: labelMetrics; font: label.font; text: label.text}
  contentItem: Item {
    readonly property alias text: label.text
    readonly property alias color: label.color
    readonly property alias horizontalAlignment: label.horizontalAlignment
    WispIcon {
      id: actionIcon; theme: root.theme; name: root.iconName; ink: root.labelColor
      visible: root.showIcon
      anchors.verticalCenter: parent.verticalCenter
      x: root.iconOnly ? (parent.width-width)/2 : root.textAlignment === Text.AlignLeft ? 0 : Math.max(0,(parent.width-width-root.theme.space(8)-labelMetrics.advanceWidth)/2)
      opacity: root.enabled ? 1 : 0.4
    }
    Text {
    id: label
    textFormat: Text.PlainText
    visible: !(root.iconTreatment && root.iconOnly)
    x: root.showIcon ? actionIcon.x + actionIcon.width + root.theme.space(8) : 0
    width: Math.max(0,parent.width-x); height: parent.height
    text: root.theme.tui ? "[" + (["···", "⋯", "…"].indexOf(root.text) >= 0 ? ":" : root.text) + "]" : root.displayText
    color: root.labelColor
    opacity: !root.enabled ? 0.4 : 1
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    elide: Text.ElideRight
    horizontalAlignment: root.showIcon ? Text.AlignLeft : root.textAlignment
    verticalAlignment: Text.AlignVCenter
  }
  }
}
