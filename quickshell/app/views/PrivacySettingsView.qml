import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "../components"

Column {
  id: root
  objectName: "privacySettingsView"
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: theme.spacing.lg
  readonly property var status: bridge.privacyStatus || ({configured:false})
  property string recoveryFile: ""
  Component.onCompleted: { if (typeof bridge.refreshPrivacy === "function" && bridge.daemonConnected) bridge.refreshPrivacy() }
  onVisibleChanged: { if (visible && typeof bridge.refreshPrivacy === "function" && bridge.daemonConnected) bridge.refreshPrivacy() }
  Connections {
    target: root.bridge
    function onDaemonConnectedChanged() {
      if (root.visible && root.bridge.daemonConnected && typeof root.bridge.refreshPrivacy === "function") root.bridge.refreshPrivacy()
    }
  }
  Text {
    text: "Chat encryption"
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "Encryption is set up automatically when your account connects. Private keys are saved only on this device."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: root.status.error || (root.status.configured ? "New chat content is encrypted on this device before upload." : "Waiting for automatic encryption setup. Chat is blocked until encryption is ready.")
    color: root.status.error ? root.theme.danger : root.status.configured ? root.theme.accent : root.theme.warning
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "Trust on first connection: Wisp remembers each friend's initial key across rooms. Unexpected key changes stop sending. Encrypted room membership requires a signed update from an authorized client. All participants need an updated, configured client."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "Existing plaintext history and backups are not encrypted retroactively. Recovery files can unlock your history: keep them off the server and never send them to friends. Losing every device and the recovery file loses access. This archive design does not provide forward secrecy."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width; spacing: root.theme.spacing.sm
    ChatButton {
      theme: root.theme; text: "Refresh status"
      enabled: !!root.bridge.daemonConnected && !root.bridge.privacyBusy
      onClicked: root.bridge.refreshPrivacy()
    }
    ChatButton {
      theme: root.theme; text: "Restore recovery file…"
      visible: !root.status.configured || !!root.status.error
      enabled: !!root.bridge.daemonConnected && !root.bridge.privacyBusy
      onClicked: recoveryPicker.open()
    }
    ChatButton {
      theme: root.theme; text: "Back up recovery file…"
      visible: !!root.status.configured && !root.status.error
      enabled: !root.bridge.privacyBusy
      onClicked: {root.recoveryFile=""; backupPicker.open()}
    }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "Back up your recovery file to another safe location to keep access if this device is lost. On a new device, restore that file if this account already has encryption keys."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.WrapAnywhere
    visible: !!root.status.fingerprint
    text: "Your key fingerprint (optional verification):\n" + (root.status.fingerprint || "")
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    visible: !!root.bridge.privacyFeedback || !!root.bridge.privacyBusy
    text: root.bridge.privacyBusy ? "Working…" : String(root.bridge.privacyFeedback || "")
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "The server still sees membership, room names, voice-invite metadata, timestamps and traffic sizes. Encryption does not hide those."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  FileDialog {
    id: recoveryPicker; title: "Choose your private Wisp recovery file"
    nameFilters: ["Recovery files (*.key)","All files (*)"]
    onAccepted: {root.recoveryFile=String(selectedFile); backupPicker.open()}
  }
  FileDialog {
    id: backupPicker; title: "Save a private recovery backup — never upload this to the server"
    fileMode: FileDialog.SaveFile; defaultSuffix: "key"
    onAccepted: {
      if (root.status.configured && !root.status.error && !root.recoveryFile) root.bridge.exportPrivacy(String(selectedFile))
      else root.bridge.configurePrivacy(String(selectedFile),root.recoveryFile)
    }
  }
}
