import QtQuick
import QtQuick.Controls

SettingsSection {
  id: root
  objectName: "accountBackupSettings"
  required property var bridge
  title: "Account backup"
  summary: status.pending ? "An account action needs attention" : status.error || status.sync_error ? "Backup needs attention" : status.mode === "secure" ? (status.unlocked ? "Encrypted backup enabled" : "Unlock your encrypted backup") : "Restore your account on another device"
  expanded: false
  readonly property var status: bridge.accountBackup || ({})
  readonly property bool busy: bridge.profileBusy || !bridge.daemonConnected
  function clearPasswords() { original.text = ""; fresh.text = ""; confirm.text = ""; unlockPassword.text = "" }
  function act(name, args) { if (bridge.profileAction(name, args || {})) clearPasswords() }
  onVisibleChanged: if (!visible) clearPasswords()
  onExpandedChanged: { if (!expanded) clearPasswords(); else if (!busy) bridge.profileAction("backup_status", {}) }
  component Label: Text {
    width: root.width; wrapMode: Text.WordWrap; textFormat: Text.PlainText
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  component Password: TextField {
    id: backupField
    width: root.width; maximumLength: 1024; echoMode: TextInput.Password
    enabled: !root.busy; color: root.theme.foreground; placeholderTextColor: root.theme.muted
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    ThemeControlStyle { theme: root.theme; control: backupField }
    background: Rectangle { color: root.theme.background; radius: root.theme.cornerRadius; border.color: backupField.activeFocus ? root.theme.focusBorder : root.theme.separator }
  }
  Label { text: "Your encryption keys and trust settings are backed up privately. Sign in on another device to restore them." }
  Label { visible: !!root.status.error || !!root.status.sync_error; text: root.status.error || root.status.sync_error || ""; color: root.theme.danger }
  Label {
    visible: root.status.mode === "secure"
    text: root.status.repair_needed ? "Password reset detected. Restore backup access from a trusted device below."
      : !root.status.unlocked ? "Enter your password to unlock this device."
      : root.status.last_synced_at ? "Last synced " + new Date(root.status.last_synced_at * 1000).toLocaleString() + (root.status.has_changes ? " · Changes waiting" : "") : "Encrypted backup is ready."
  }
  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    visible: root.status.mode === "classic" && !root.status.pending
    Label { text: "Choose a new, different password to enable encrypted account backup. Your existing account and encryption keys are kept." }
    Password { id: original; objectName: "backupOriginalPassword"; placeholderText: "Current password"; Accessible.name: placeholderText }
    Password { id: fresh; objectName: "backupNewPassword"; placeholderText: "New secure password"; Accessible.name: placeholderText }
    Password { id: confirm; objectName: "backupConfirmPassword"; placeholderText: "Confirm new password"; Accessible.name: placeholderText }
    ChatButton {
      theme: root.theme; text: "Enable encrypted backup"; objectName: "backupEnable"
      enabled: !root.busy && !!original.text && Array.from(fresh.text).length >= 12 && fresh.text === confirm.text && fresh.text !== original.text
      onClicked: root.act("backup_enable", {current_password:original.text,new_password:fresh.text})
    }
  }
  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    visible: root.status.mode === "secure"
    CheckBox {
      text: "Sync automatically"; checked: !!root.status.auto_sync; enabled: !root.busy
      onToggled: root.act("backup_auto_sync", {enabled:checked})
      ThemeControlStyle { theme: root.theme; control: parent }
    }
    ChatButton { theme: root.theme; text: "Sync now"; enabled: !root.busy && root.status.unlocked && !root.status.pending; onClicked: root.act("backup_sync") }
  }
  Password {
    id: unlockPassword; objectName: "backupUnlockPassword"
    visible: !!root.status.pending || root.status.mode === "secure" && (!root.status.unlocked || root.status.repair_needed)
    placeholderText: root.status.pending === "migration" ? "Password for recovery" : "Current password"; Accessible.name: placeholderText
  }
  ChatButton {
    theme: root.theme; text: root.status.repair_needed && root.status.unlocked ? "Restore backup access" : "Unlock backup"
    visible: root.status.mode === "secure" && !root.status.pending && (!root.status.unlocked || root.status.repair_needed)
    enabled: !root.busy && !!unlockPassword.text
    onClicked: root.act(root.status.repair_needed && root.status.unlocked ? "backup_repair" : "backup_unlock", {password:unlockPassword.text})
  }
  Column {
    width: parent.width; spacing: root.theme.spacing.sm; visible: !!root.status.pending
    Label { text: "An account action was interrupted. Resume it, or sign in again to recover safely." }
    ChatButton { theme: root.theme; text: "Resume interrupted action"; enabled: !root.busy; onClicked: root.act("backup_resume", {password:unlockPassword.text}) }
    ChatButton { theme: root.theme; text: "Recover with secure password"; enabled: !root.busy && !!unlockPassword.text; onClicked: root.act("backup_recover_secure", {password:unlockPassword.text}) }
    ChatButton { theme: root.theme; text: "Recover with original password"; visible: root.status.pending === "migration"; enabled: !root.busy && !!unlockPassword.text; onClicked: root.act("backup_recover_classic", {password:unlockPassword.text}) }
    Label { visible: root.status.pending === "migration"; text: "If migration has not completed, use your original password. If it has completed, use your new secure password." }
    ChatButton { theme: root.theme; text: "Cancel prepared action"; visible: !!root.status.can_cancel; enabled: !root.busy; onClicked: root.act("backup_cancel") }
  }
  ChatButton { theme: root.theme; text: "Finish account setup"; visible: !!root.status.installation_pending; enabled: !root.busy; onClicked: root.act("backup_finish_install") }
  Timer { interval: 30000; repeat: true; running: root.visible && root.expanded; onTriggered: if (!root.busy) root.bridge.profileAction("backup_status", {}) }
}
