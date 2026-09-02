import QtQuick

Column {
  id: root

  required property var bridge
  required property var theme
  property var anchorController: null

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

  Rectangle {
    visible: !!root.anchorController
    width: parent.width
    height: visible ? desktopSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.035)

    Column {
      id: desktopSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      spacing: root.theme.spacing.lg

      Text {
        text: "Desktop position"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        font.weight: Font.DemiBold
      }

      Text {
        width: parent.width
        text: "Wisp always opens on the primary display. Auto follows the tray edge when the tray is on that display; otherwise it uses the bottom-right corner."
        color: root.theme.muted
        wrapMode: Text.WordWrap
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      Flow {
        width: parent.width
        height: childrenRect.height
        spacing: root.theme.spacing.sm

        Repeater {
          model: [
            { "value": "auto", "label": "Auto" },
            { "value": "bottom-right", "label": "Bottom right" },
            { "value": "bottom-left", "label": "Bottom left" },
            { "value": "top-right", "label": "Top right" },
            { "value": "top-left", "label": "Top left" }
          ]

          delegate: Rectangle {
            required property var modelData
            readonly property bool selected: root.anchorController
              && root.anchorController.anchor === modelData.value
            width: anchorLabel.implicitWidth + root.theme.spacing.xl * 2
            height: root.theme.space(30)
            radius: root.theme.cornerRadius
            color: selected
              ? root.theme.alpha(root.theme.accent, 0.24)
              : anchorMouse.containsMouse
                ? root.theme.alpha(root.theme.foreground, 0.11)
                : root.theme.alpha(root.theme.foreground, 0.055)
            border.width: selected ? 1 : 0
            border.color: root.theme.alpha(root.theme.accent, 0.75)

            Text {
              id: anchorLabel
              anchors.centerIn: parent
              text: modelData.label
              color: selected ? root.theme.foreground : root.theme.muted
              font.family: root.theme.font.family
              font.pixelSize: root.theme.font.caption
            }

            MouseArea {
              id: anchorMouse
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.anchorController.setAnchor(modelData.value)
            }
          }
        }
      }

      Text {
        text: root.anchorController && root.anchorController.primaryScreen
          ? "Primary display: " + root.anchorController.primaryScreen.name
          : "Primary display unavailable"
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }
}
