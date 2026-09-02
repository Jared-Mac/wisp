import QtQuick

Column {
  id: root

  required property var bridge
  required property var theme

  width: parent ? parent.width : 0
  spacing: root.theme.spacing.lg

  Column {
    width: parent.width
    spacing: root.theme.spacing.xs

    Text {
      text: "Settings"
      color: root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.title
      font.weight: Font.DemiBold
    }

    Text {
      width: parent.width
      text: "Choose how Wisp captures and plays voice. Changes apply to the active call."
      color: root.theme.muted
      wrapMode: Text.WordWrap
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Rectangle {
    width: parent.width
    height: audioSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.035)

    AudioSettingsView {
      id: audioSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge
      theme: root.theme
    }
  }
}
