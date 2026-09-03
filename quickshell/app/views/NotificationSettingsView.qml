import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  spacing: theme.spacing.lg

  Text { text: "Notifications · this device"; color: root.theme.foreground; font.bold: true }
  Text {
    width: parent.width
    text: "Message sounds play only while Wisp is unfocused. Unread messages stay on the tray badge until reviewed."
    wrapMode: Text.Wrap
    color: root.theme.muted
    font.pixelSize: root.theme.font.caption
  }
  ChatButton {
    theme: root.theme
    text: root.bridge.notificationMuted ? "Sound muted · Enable" : "Sound enabled · Mute"
    onClicked: root.bridge.notificationMuted = !root.bridge.notificationMuted
  }
  Row {
    width: parent.width
    spacing: root.theme.spacing.lg
    Text { text: "Volume"; color: root.theme.foreground; anchors.verticalCenter: parent.verticalCenter }
    Slider {
      width: Math.min(root.width - 120, root.theme.space(300))
      from: 0; to: 100; stepSize: 1
      value: root.bridge.notificationVolume
      onMoved: root.bridge.notificationVolume = Math.round(value)
    }
    Text { text: root.bridge.notificationVolume + "%"; color: root.theme.muted; anchors.verticalCenter: parent.verticalCenter }
  }
  Text {
    width: parent.width
    text: root.bridge.notificationSoundPath || "Default: Wisp chime"
    elide: Text.ElideMiddle
    color: root.theme.muted
    font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width
    spacing: root.theme.spacing.lg
    ChatButton { theme: root.theme; text: "Choose sound…"; onClicked: soundPicker.open() }
    ChatButton { theme: root.theme; text: "Use default"; onClicked: root.bridge.notificationSoundPath = "" }
    ChatButton {
      theme: root.theme; text: "Test sound"
      enabled: !root.bridge.notificationMuted && root.bridge.notificationVolume > 0
      onClicked: root.bridge.playNotificationSound()
    }
  }
  Text {
    width: parent.width
    visible: text !== ""
    text: root.bridge.notificationError
    wrapMode: Text.Wrap
    color: root.theme.danger
    font.pixelSize: root.theme.font.caption
  }
  FileDialog {
    id: soundPicker
    title: "Choose a notification sound"
    nameFilters: ["Audio files (*.wav *.ogg *.flac *.mp3)", "All files (*)"]
    onAccepted: root.bridge.notificationSoundPath = String(selectedFile)
  }
}
