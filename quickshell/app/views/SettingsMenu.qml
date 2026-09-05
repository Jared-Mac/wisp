import QtQuick
import "../components"

Column {
  id: root
  objectName: "settingsMenu"
  property string section: "media"

  required property var bridge
  required property var theme
  property var anchorController: null

  Connections {
    target: root.bridge
    function onCanManageServerChanged() {
      if (!root.bridge.canManageServer && root.section === "server") root.section = "media"
    }
  }

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
      text: "Manage your profile, devices, privacy, and Wisp preferences."
      color: root.theme.muted
      wrapMode: Text.WordWrap
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Flow {
    width: parent.width
    spacing: root.theme.spacing.sm
    Repeater {
      model: {
        var tabs = [{id:"profile",label:"Profile"},{id:"media",label:"Audio / Video"},{id:"appearance",label:"Appearance"},{id:"notifications",label:"Notifications & Chat"},{id:"privacy",label:"Privacy"},{id:"devices",label:"Devices"}]
        if (root.bridge.canManageServer) tabs.push({id:"server",label:"Server"})
        return tabs
      }
      SettingsTab {
        required property var modelData
        theme: root.theme; text: modelData.label
        objectName: "settingsTab-" + modelData.id
        primary: root.section === modelData.id
        onClicked: root.section = modelData.id
      }
    }
  }

  Rectangle {
    visible: root.section === "profile"
    width: parent.width
    height: visible ? profileSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    ProfileSettingsView {
      id: profileSettings
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge; theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "server" && root.bridge.canManageServer
    width: parent.width
    height: visible ? serverSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    ServerSettingsView {
      id: serverSettings
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge; theme: root.theme
    }
  }

  Rectangle {
    // Tab content stays instantiated so changing tabs never resets controls.
    visible: root.section === "appearance"
    width: parent.width
    height: appearanceSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator
    AppearanceSettingsView {
      id: appearanceSettings
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "privacy"
    width: parent.width
    height: privacySettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    PrivacySettingsView {
      id: privacySettings
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge; theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "notifications"
    width: parent.width
    height: notificationSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator
    NotificationSettingsView {
      id: notificationSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge
      theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "devices"
    width: parent.width
    height: deviceSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator

    DeviceSettingsView {
      id: deviceSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge
      theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "media"
    width: parent.width
    height: videoSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator

    VideoSettingsView {
      id: videoSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      bridge: root.bridge
      theme: root.theme
    }
  }

  Rectangle {
    visible: root.section === "media"
    width: parent.width
    height: audioSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator

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
    visible: root.section === "appearance" && !!root.anchorController
    width: parent.width
    height: visible ? desktopSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui ? 1 : 0
    border.color: root.theme.separator

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
        text: "Auto follows the tray icon's display and edge when the desktop provides its position. Otherwise Wisp uses the current system display and the bottom-right corner."
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
        text: root.anchorController && root.anchorController.screen
          ? "Panel display: " + root.anchorController.screen.name
          : "Panel display unavailable"
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }
}
