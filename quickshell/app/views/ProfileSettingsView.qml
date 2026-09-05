import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  objectName: "profileSettingsView"
  required property var bridge
  required property var theme
  spacing: root.theme.spacing.lg
  readonly property string serverId: bridge.profileServerId
  readonly property bool inVoice: !!(bridge.activeServerState.self || {}).hangout_id
  onServerIdChanged: { clearPasswords(); if (visible) bridge.refreshProfile() }
  onVisibleChanged: { clearPasswords(); if (visible) bridge.refreshProfile() }
  function clearPasswords() { currentPassword.text = ""; newPassword.text = ""; confirmPassword.text = "" }
  Connections {
    target: root.bridge
    function onAccountProfileChanged() { displayName.text = String(root.bridge.accountProfile.display_name || root.bridge.selfState.display_name || "") }
    function onDaemonConnectedChanged() {
      root.clearPasswords()
      if (root.visible && root.bridge.daemonConnected) root.bridge.refreshProfile()
    }
  }
  component Label: Text {
    width: root.width; wrapMode: Text.WordWrap
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  component Field: TextField {
    width: root.width; color: root.theme.foreground
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    placeholderTextColor: root.theme.muted
    enabled: root.bridge.profileReady && !root.bridge.profileBusy
    selectByMouse: true
    ThemeControlStyle { theme: root.theme; control: parent }
    background: Rectangle {
      color: root.theme.background; radius: root.theme.cornerRadius
      border.width: 1; border.color: parent.activeFocus ? root.theme.focusBorder : root.theme.separator
    }
  }
  Label { text: "Profile · " + String(root.bridge.activeServer.name || "Server"); color: root.theme.foreground; font.bold: true }
  Label { text: "These settings apply to your account on this server." }
  Label { visible: root.bridge.profileReady; text: "Username: " + String(root.bridge.accountProfile.username || "Development account") }
  ChatButton {
    objectName: "profileRefresh"; theme: root.theme
    text: root.bridge.profileBusy ? "loading…" : "refresh"
    enabled: root.bridge.daemonConnected && !root.bridge.profileBusy
    onClicked: root.bridge.refreshProfile()
  }
  Label { text: root.bridge.profileFeedback; visible: text !== ""; color: root.theme.foreground }
  Label { text: "Display name"; color: root.theme.foreground; font.bold: true }
  Field {
    id: displayName; objectName: "profileDisplayName"
    maximumLength: 80; placeholderText: "Display name"
    Accessible.name: "Display name"
    enabled: root.bridge.profileReady && !root.bridge.profileBusy && !root.inVoice
    onAccepted: if (saveName.enabled) saveName.clicked()
  }
  Label { visible: root.inVoice; text: "Leave voice before changing your display name." }
  ChatButton {
    id: saveName; objectName: "profileSaveName"; theme: root.theme; text: "save name"
    enabled: displayName.enabled && !!displayName.text.trim()
      && displayName.text.trim() !== String(root.bridge.accountProfile.display_name || "")
    onClicked: root.bridge.profileAction("update_account_profile", {display_name:displayName.text.trim(),revision:root.bridge.accountProfile.revision})
  }
  Label { text: "Password"; color: root.theme.foreground; font.bold: true }
  Label { text: "Use at least 12 characters. Your other devices stay signed in; you can revoke them in Devices." }
  Field {
    id: currentPassword; objectName: "profileCurrentPassword"
    echoMode: TextInput.Password; maximumLength: 1024
    placeholderText: "Current password"; Accessible.name: "Current password"
    enabled: root.bridge.profileReady && !root.bridge.profileBusy && !!root.bridge.accountProfile.password_available
  }
  Field {
    id: newPassword; objectName: "profileNewPassword"
    echoMode: TextInput.Password; maximumLength: 1024
    placeholderText: "New password"; Accessible.name: "New password"
    enabled: currentPassword.enabled
  }
  Field {
    id: confirmPassword; objectName: "profileConfirmPassword"
    echoMode: TextInput.Password; maximumLength: 1024
    placeholderText: "Confirm new password"; Accessible.name: "Confirm new password"
    enabled: currentPassword.enabled
    onAccepted: if (savePassword.enabled) savePassword.clicked()
  }
  Label { visible: confirmPassword.text !== "" && confirmPassword.text !== newPassword.text; text: "Passwords do not match."; color: root.theme.danger }
  ChatButton {
    id: savePassword; objectName: "profileSavePassword"; theme: root.theme; text: "change password"
    enabled: currentPassword.enabled && currentPassword.text !== "" && Array.from(newPassword.text).length >= 12
      && newPassword.text === confirmPassword.text
    onClicked: {
      if (root.bridge.profileAction("change_account_password", {current_password:currentPassword.text,new_password:newPassword.text})) root.clearPasswords()
    }
  }
  Label { text: "Two-factor authentication"; color: root.theme.foreground; font.bold: true }
  Label { objectName: "profileTwoFactorStatus"; text: "Planned for a future update. 2FA is not available yet." }
}
