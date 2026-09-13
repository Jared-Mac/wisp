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
  onServerIdChanged: { clearPasswords(); if (visible) {bridge.refreshProfile();if(bridge.accountActions)bridge.accountActions.act("account_overview",{},serverId)} }
  onVisibleChanged: { clearPasswords(); if (visible) {bridge.refreshProfile();if(bridge.accountActions)bridge.accountActions.act("account_overview",{},serverId)} }
  function clearPasswords() { currentPassword.text = ""; newPassword.text = ""; confirmPassword.text = ""; recoveryPassword.text = "" }
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
    id: styledControl1
    width: root.width; color: root.theme.foreground
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    placeholderTextColor: root.theme.muted
    enabled: root.bridge.profileReady && !root.bridge.profileBusy
    selectByMouse: true
    ThemeControlStyle { theme: root.theme; control: styledControl1 }
    background: Rectangle {
      color: root.theme.background; radius: root.theme.cornerRadius
      border.width: 1; border.color: parent.activeFocus ? root.theme.focusBorder : root.theme.separator
    }
  }
  Label { text: "Profile · " + String(root.bridge.activeServer.name || "Server"); color: root.theme.foreground; font.bold: true }
  Label { text: "Manage your account and profile." }
  Label { visible: root.bridge.profileReady; text: "Sign-in username: " + String(root.bridge.accountProfile.username || "Development account") }
  ChatButton {
    objectName: "profileRefresh"; theme: root.theme
    text: root.bridge.profileBusy ? "loading…" : "refresh"
    enabled: root.bridge.daemonConnected && !root.bridge.profileBusy
    onClicked: root.bridge.refreshProfile()
  }
  Label { text: root.bridge.profileFeedback; visible: text !== ""; color: root.theme.foreground }
  ProfilePicture { width: parent.width; bridge: root.bridge; theme: root.theme; serverId: root.serverId }
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
  SettingsSection {
    id:handleSection;theme:root.theme;title:"Public username";summary:"Let friends find you";expanded:false
    readonly property var account:root.bridge.accountActions ? root.bridge.accountActions.state(root.serverId) : ({overview:{}})
    Label {text:"Share this username so people can send you a friend request. Leave it empty to stay out of username search."}
    Field {id:publicHandle;objectName:"publicFriendHandle";maximumLength:32;placeholderText:"username";text:handleSection.account.overview.handle || ""}
    ChatButton {theme:root.theme;text:"Save public username";enabled:!handleSection.account.busy;onClicked:root.bridge.accountActions.act("set_public_handle",{handle:publicHandle.text.trim()},root.serverId)}
    ChatButton {theme:root.theme;text:"Copy @username";visible:!!handleSection.account.overview.handle;onClicked:root.bridge.copyChatText("@"+handleSection.account.overview.handle)}
    Label {text:handleSection.account.error || handleSection.account.feedback || "";visible:!!text}
  }
  SettingsSection {
    id:blockedSection;theme:root.theme;title:"Blocked accounts";summary:"Manage who can contact you";expanded:false
    readonly property var account:root.bridge.accountActions ? root.bridge.accountActions.state(root.serverId) : ({overview:{}})
    Label {text:"Blocking removes friendship and stops new direct messages and friend requests. Unblocking does not add the friend again."}
    Repeater {
      model:blockedSection.account.overview.blocked || []
      Row {
        required property var modelData
        width:root.width;spacing:root.theme.spacing.sm
        Text {width:Math.max(0,parent.width-unblock.width-parent.spacing);text:parent.modelData.display_name;textFormat:Text.PlainText;elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
        ChatButton {id:unblock;theme:root.theme;text:"Unblock";enabled:!blockedSection.account.busy;onClicked:root.bridge.accountActions.act("unblock_person",{user_id:parent.modelData.id},root.serverId)}
      }
    }
    Label {visible:!(blockedSection.account.overview.blocked || []).length;text:"No blocked accounts"}
    Label {text:blockedSection.account.error || "";visible:!!text}
  }
  ChatButton {
    theme:root.theme; text:"Finish account update"
    visible:!!root.bridge.accountSetup && root.bridge.accountSetup.needsAttention
    enabled:!!root.bridge.accountSetup && !root.bridge.accountSetup.busy
    onClicked:root.bridge.accountSetup.request()
  }
  SettingsSection {
    id: recoverySection; theme: root.theme; title: "Recovery email"; summary: "Recover access if you forget your password"
    objectName: "profileRecoverySection"; expanded: false
    readonly property var recovery: root.bridge.recoveryEmail || ({})
    Label { text: recoverySection.recovery.verified ? "Verified: " + recoverySection.recovery.email : "No verified recovery email yet."; textFormat: Text.PlainText }
    Label { visible: !!recoverySection.recovery.pending_email; text: "Awaiting verification: " + (recoverySection.recovery.pending_email || ""); textFormat: Text.PlainText }
    Label { visible: !recoverySection.recovery.delivery_available; text: "Recovery email is not available on this server yet." }
    Field { id: recoveryAddress; objectName: "profileRecoveryEmail"; placeholderText: "Email address"; Accessible.name: "Recovery email"; maximumLength: 254; inputMethodHints: Qt.ImhEmailCharactersOnly }
    Field { id: recoveryPassword; objectName: "profileRecoveryPassword"; placeholderText: "Current password"; Accessible.name: "Current password for recovery email"; maximumLength: 1024; echoMode: TextInput.Password }
    ChatButton {
      theme: root.theme; text: "Send verification email"; objectName: "profileVerifyEmail"
      enabled: root.bridge.profileReady && !root.bridge.profileBusy && !root.bridge.accountBackup.pending && !!(root.bridge.recoveryEmail || {}).delivery_available && !!recoveryAddress.text.trim() && !!recoveryPassword.text
      onClicked: if (root.bridge.profileAction("set_recovery_email", {email:recoveryAddress.text.trim(),current_password:recoveryPassword.text})) recoveryPassword.text = ""
    }
    Label { text: "Verify the link from support@wisp.you. After an email password reset, use a trusted device to restore encrypted account access." }
  }
  SettingsSection {
    theme: root.theme; title: "Change password"; summary: "Keep your account secure"
    objectName: "profilePasswordSection"; expanded: false
    Label { text: "Password"; color: root.theme.foreground; font.bold: true }
    Label { text: "At least 12 characters. Other devices stay signed in." }
    ChatButton { theme: root.theme; text: "Forgot password?"; onClicked: Qt.openUrlExternally("https://wisp.you/account/forgot-password") }
    Field {
      id: currentPassword; objectName: "profileCurrentPassword"
      echoMode: TextInput.Password; maximumLength: 1024
      placeholderText: "Current password"; Accessible.name: "Current password"
      enabled: root.bridge.profileReady && !root.bridge.profileBusy && !!root.bridge.accountProfile.password_available && !root.bridge.accountBackup.pending
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
  }
  SettingsSection {
    theme: root.theme; title: "Two-factor authentication"; summary: "Planned for a future update"
    objectName: "profileSecuritySection"; expanded: false
    Label { text: "Two-factor authentication"; color: root.theme.foreground; font.bold: true }
    Label { objectName: "profileTwoFactorStatus"; text: "Planned for a future update. 2FA is not available yet." }
  }
  SettingsSection {
    theme: root.theme; title: "My emojis"; summary: "Upload and manage your custom emojis"
    objectName: "profileEmojiSection"; expanded: false
    EmojiLibrary {objectName:"accountEmojiLibrary";width:parent.width;bridge:root.bridge;theme:root.theme;serverId:root.serverId;scope:"account"}
  }

}
