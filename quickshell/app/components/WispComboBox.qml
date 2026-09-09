import QtQuick
import QtQuick.Controls

ComboBox {
  id: root
  required property var theme
  implicitHeight: theme.space(theme.friendly ? 38 : 32)
  leftPadding: theme.space(12); rightPadding: theme.space(32)
  font.family: theme.font.family; font.pixelSize: theme.font.body
  palette.text: theme.foreground; palette.windowText: theme.foreground; palette.buttonText: theme.foreground
  palette.window: theme.surface; palette.base: theme.background; palette.button: theme.surface
  palette.highlight: theme.accent; palette.highlightedText: theme.accentText
  contentItem: Text {
    text: root.displayText; font: root.font; color: root.enabled ? root.theme.foreground : root.theme.muted
    verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
  }
  indicator: WispIcon {
    theme: root.theme; name: "chevron"; ink: root.enabled ? root.theme.foreground : root.theme.muted
    x: root.width-width-root.theme.space(10); y: (root.height-height)/2
  }
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground,root.hovered ? 0.075 : 0.035)
    border.width: 1; border.color: root.activeFocus ? root.theme.accent : root.theme.separator
  }
  delegate: ItemDelegate {
    id: option; required property int index
    width: root.popup.width; text: root.textAt(index); highlighted: root.highlightedIndex===index
    implicitHeight: root.theme.space(36)
    contentItem: Text {text:option.text;color:root.theme.foreground;font:root.font;elide:Text.ElideRight;verticalAlignment:Text.AlignVCenter}
    background: Rectangle {radius:root.theme.cornerRadius;color:option.highlighted || option.hovered ? root.theme.alpha(root.theme.accent,0.16) : "transparent"}
  }
  popup.background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
}
