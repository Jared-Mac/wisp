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
      : "Interface styles change presentation only. All have the same features, chats, and controls. Applies to the tray popup and full app."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width; spacing: root.theme.spacing.lg
    Repeater {
      model: [{profile:"performative",label:"Performative · default"},{profile:"clean_tui",label:"Clean TUI"},{profile:"herdr",label:"Herdr"},{profile:"terminal",label:"Terminal Grid"},{profile:"legacy",label:"Classic"}]
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
    width: parent.width; wrapMode: Text.WordWrap
    visible: !!root.appearance && root.appearance.profile === "clean_tui"
    text: "Clean TUI uses a narrow activity rail, quiet rules, restrained selections, and a calmer transcript. Palettes and color accents are independent of the interface style."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    text: "Color palette"
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width; spacing: root.theme.spacing.lg
    Repeater {
      model: [{key:"wisp",label:"Wisp blue"},{key:"graphite",label:"Graphite"},{key:"violet",label:"Violet"},{key:"ember",label:"Ember"},{key:"ash_olive",label:"Ash & Olive"},{key:"herdr",label:"Solarized Japan"}]
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
    text: "Ash & Olive contains the original Performative colors. Solarized Japan contains the original Herdr colors. Changing colors never changes your interface style."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    text: "Color accents"
    color: root.theme.foreground
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    text: "Choose where distinct colors appear, in any style or palette. Chat colors stay attached to the conversation across tiles. Disabled areas use neutral colors; focus, unread, and safety indicators remain visible."
    color: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Repeater {
    model: [{key:"chatBorders",label:"Distinct chat borders / rules"},{key:"chatHeadings",label:"Distinct chat headings"},{key:"roomSections",label:"Room section accents"},{key:"friendSections",label:"Friends section accents"},{key:"friendNames",label:"Online friend name accents"},{key:"senderNames",label:"Message sender name accents"}]
    ChatButton {
      required property var modelData
      objectName: "color-option-" + modelData.key
      theme: root.theme
      text: modelData.label + (root.theme.colorEnabled(modelData.key) ? " · On" : " · Off")
      primary: root.theme.colorEnabled(modelData.key)
      enabled: !!root.appearance && !root.appearance.managed
      onClicked: root.appearance.setColorOption(modelData.key, !root.theme.colorEnabled(modelData.key))
    }
  }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    visible: !!root.appearance && root.appearance.error !== ""
    text: root.appearance ? root.appearance.error : ""
    color: root.theme.danger
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
