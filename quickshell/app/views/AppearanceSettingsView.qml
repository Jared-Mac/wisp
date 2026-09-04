import QtQuick
import "../components"

Column {
  id: root
  required property var theme
  readonly property var appearance: theme.appearanceController
  width: parent ? parent.width : 0
  spacing: theme.spacing.lg

  Text {
    text: "Appearance · this device"
    color: root.theme.foreground
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    text: !root.appearance || root.appearance.managed
      ? "Appearance follows your Omarchy shell. All Wisp features are available."
      : "Themes change appearance only. Both have the same features, chats, and controls. Applies to the tray popup and full app."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width; spacing: root.theme.spacing.lg
    Repeater {
      model: [{profile:"terminal",label:"Terminal · default"},{profile:"legacy",label:"Classic · original"}]
      ChatButton {
        required property var modelData
        objectName: "theme-" + modelData.profile
        theme: root.theme; text: modelData.label
        primary: !!root.appearance && root.appearance.profile === modelData.profile
        enabled: !!root.appearance && !root.appearance.managed
        onClicked: root.appearance.setProfile(modelData.profile)
      }
    }
  }
  Text {
    text: "Color palette"
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width; spacing: root.theme.spacing.lg
    Repeater {
      model: [{key:"wisp",label:"Wisp blue"},{key:"graphite",label:"Graphite"},{key:"violet",label:"Violet"},{key:"ember",label:"Ember"},{key:"performative",label:"Performative"}]
      ChatButton {
        required property var modelData
        objectName: "palette-" + modelData.key
        theme: root.theme; text: modelData.label
        primary: !!root.appearance && root.appearance.palette === modelData.key
        enabled: !!root.appearance && !root.appearance.managed
        onClicked: root.appearance.setPalette(modelData.key)
      }
    }
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    visible: !!root.appearance && root.appearance.palette === "performative"
    text: "Performative · a Linux-terminal interface: monospace, numbered frames, bracketed controls, prompt editor, and a live status line. Overrides the base style while selected."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    visible: !!root.appearance && root.appearance.error !== ""
    text: root.appearance ? root.appearance.error : ""
    color: root.theme.danger
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
