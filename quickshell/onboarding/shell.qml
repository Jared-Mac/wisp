//@ pragma AppId dev.wisp.onboarding

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io

ShellRoot {
  id: app
  property alias accountWindow: window

  FloatingWindow {
    id: window
    objectName: "accountWindow"
    visible: true
    title: mode === "reset" ? "Reset Wisp password" : acceptingInvite ? "Accept invitation" : mode === "login" ? "Sign in to Wisp" : mode === "register" ? "Create a Wisp account" : "Set up a Wisp server"
    implicitWidth: 560
    implicitHeight: acceptingInvite ? 500 : 650
    minimumSize: Qt.size(440, acceptingInvite ? 460 : 540)
    color: "#151821"
    onClosed: Qt.exit(2)

    property string mode: {
      var requested = Quickshell.env("WISP_ONBOARDING_MODE") || "login"
      return ["register", "login", "bootstrap", "reset"].indexOf(requested) >= 0 ? requested : "login"
    }
    property bool advanced: false
    property bool classicSignIn: false
    property var setupState: ({})
    property string requestedAction: ""
    readonly property bool resetting: mode === "reset"
    function refreshSetup() { if (!setupProcess.running && !busy && !resetting) setupProcess.running = true }
    Component.onCompleted: Qt.callLater(refreshSetup)
    property bool invitationEntry: Quickshell.env("WISP_ONBOARDING_JOIN") === "1"
    property var resolvedInvitation: null
    property string resolvedInput: ""
    property bool useSavedAccount: true
    readonly property var invitation: resolvedInput === invite.text.trim() && resolvedInvitation ? resolvedInvitation : parseInvitation(invite.text.trim())
    readonly property bool existingAccount: !!(acceptingInvite && invitation.saved_account && useSavedAccount)
    function modernLink(value) { return /^(https?:\/\/|wisp-invite:v2\.|wisp\.you\/|[a-z]+(?:-[a-z]+){0,3}[0-9]{12}$)/.test(value) }
    function invitationInputChanged() {
      resolvedInvitation = null; resolvedInput = ""; feedback = ""; useSavedAccount = true
      if (modernLink(invite.text.trim())) previewTimer.restart()
    }
    readonly property bool acceptingInvite: mode !== "bootstrap" && mode !== "reset" && invitation !== null
    property bool usernameEdited: false
    function parseInvitation(value) {
      if (!/^wisp-invite:[A-Za-z0-9_-]+$/.test(value) || value.length > 16384) return null
      try {
        var encoded = value.slice(12).replace(/-/g, "+").replace(/_/g, "/")
        while (encoded.length % 4) encoded += "="
        var bytes = Qt.atob(encoded), escaped = ""
        for (var i = 0; i < bytes.length; i++) escaped += "%" + ("0" + bytes.charCodeAt(i).toString(16)).slice(-2)
        var payload = JSON.parse(decodeURIComponent(escaped))
        if (payload.v !== 1 || typeof payload.token !== "string" || !payload.token.length
            || typeof payload.server !== "string" || !/^https:\/\/[a-z0-9.-]+(?::[0-9]+)?\/?$/i.test(payload.server)) return null
        return {server: payload.server, server_name:payload.server.replace(/^https:\/\//,""), legacy:true}
      } catch (error) { return null }
    }
    function suggestUsername(value) {
      var result = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "_").replace(/^_+|_+$/g, "").slice(0, 32)
      return result.length >= 3 ? result : result ? (result + "_friend") : ""
    }
    property bool busy: false
    property string feedback: ""

    function selectMode(value) {
      if (busy) return
      mode = value
      classicSignIn = false
      requestedAction = ""
      resetLink.text = ""
      useSavedAccount = false
      feedback = ""
      password.text = ""
      confirmPassword.text = ""
      bootstrapToken.text = ""
    }

    function perform(action) {
      if (busy) return
      requestedAction = action
      busy = true
      accountProcess.running = true
    }
    function submit() {
      if (busy) return
      requestedAction = ""
      feedback = ""
      if (resetting) {
        if (!resetLink.text.trim() || Array.from(password.text).length < 12 || password.text !== confirmPassword.text) {
          feedback = "Paste your reset link and enter matching passwords of at least 12 characters."
          return
        }
        perform("reset"); return
      }
      if (mode !== "bootstrap" && invite.text.trim().indexOf("wisp-invite:") === 0 && !modernLink(invite.text.trim()) && !invitation) {
        feedback = "This invitation is invalid or needs a newer Wisp version. Ask your friend for a new invite."
        return
      }
      if (modernLink(invite.text.trim()) && !invitation) {
        feedback = previewProcess.running ? "Checking invitation…" : "Check the invitation before continuing."
        previewTimer.restart(); return
      }
      if (existingAccount) { busy = true; accountProcess.running = true; return }
      if ((mode === "bootstrap" && !server.text.trim()) || !username.text.trim() || !password.text) {
        feedback = "Username and password are required."
        return
      }
      if (mode !== "login" && !displayName.text.trim()) {
        feedback = "Choose a display name."
        return
      }
      if (mode !== "login" && password.text !== confirmPassword.text) {
        feedback = "Passwords do not match."
        return
      }
      if (mode === "bootstrap" && !bootstrapToken.text.trim()) {
        feedback = "Paste the one-time server setup token."
        return
      }
      busy = true
      accountProcess.running = true
    }

    Rectangle {
      anchors.fill: parent
      color: "#151821"
      border.color: "#3b4353"
      border.width: 1

      ScrollView {
        anchors.fill: parent
        anchors.margins: 28
        contentWidth: availableWidth

        ColumnLayout {
          width: parent.width
          spacing: 14

          Text {
            text: window.resetting ? "Reset your password" : window.acceptingInvite ? "Accept invitation" : window.mode === "login" ? "Sign in to Wisp" : window.mode === "register" ? "Create your account" : "Set up your server"
            color: "#e8ecf3"
            font.family: "Hack"
            font.pixelSize: 22
            font.weight: Font.DemiBold
          }
          Text {
            text: window.resetting ? "Paste the reset link from your email. This changes your password; an existing trusted device is needed to restore backup access afterward." : window.acceptingInvite ? "Join " + (window.invitation.server_name || "this server") + ". " + (window.existingAccount ? "Continue as " + window.invitation.account_name + "." : window.mode === "login" ? "Sign in to continue." : "Create an account to continue.") : "Create an account or sign in. You can join a server later."
            textFormat: Text.PlainText
            color: "#8d96a8"
            font.family: "Hack"
            font.pixelSize: 12
            wrapMode: Text.Wrap
            Layout.fillWidth: true
          }

          RowLayout {
            Layout.fillWidth: true
            spacing: 7
            Repeater {
              model: (window.acceptingInvite ? [
                {key:"register", label:"[new account]"},
                {key:"login", label:"[already have an account]"}
              ] : [
                {key:"login", label:"[sign in]"},
                {key:"register", label:"[create account]"},
                {key:"bootstrap", label:"[new server]"}
              ])
              delegate: Button {
                required property var modelData
                objectName: "accountMode-" + modelData.key
                text: modelData.label
                enabled: !window.busy
                font.family: "Hack"
                font.pixelSize: 12
                onClicked: window.selectMode(modelData.key)
                background: Rectangle {
                  radius: 3
                  color: window.mode === modelData.key ? "#2f8cff" : "#1c202b"
                  border.color: window.mode === modelData.key ? "#72afff" : "#3b4353"
                }
                contentItem: Text {
                  text: parent.text
                  color: "#e8ecf3"
                  font: parent.font
                  horizontalAlignment: Text.AlignHCenter
                  verticalAlignment: Text.AlignVCenter
                }
              }
            }
          }

          RowLayout {
            visible: !window.acceptingInvite && !window.resetting
            Button { text: window.invitationEntry ? "Hide invitation" : "Use an invitation"; onClicked: window.invitationEntry = !window.invitationEntry }
            Button { text: window.advanced ? "Hide advanced" : "Advanced"; onClicked: window.advanced = !window.advanced }
          }
          Text {
            objectName:"invitationAccountService"
            visible:window.acceptingInvite && !window.existingAccount
            Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText
            text:"Use an account for " + String((window.invitation || {}).server || "").replace(/^https?:\/\//, "") + "."
            color:"#8d96a8";font.pixelSize:12
          }
          Text { visible: window.acceptingInvite && !!window.invitation && window.invitation.legacy; Layout.fillWidth:true; wrapMode:Text.Wrap; text:"Legacy invitation: joining also adds the inviter as a friend."; color:"#8d96a8"; font.pixelSize:12 }
          Text { visible: window.acceptingInvite && !!window.invitation && !window.invitation.legacy; Layout.fillWidth:true; wrapMode:Text.Wrap; textFormat:Text.PlainText; text:"Invited by " + String((window.invitation || {}).inviter || "a server member") + " · Expires " + new Date((window.invitation || {}).expires_at).toLocaleTimeString(); color:"#8d96a8"; font.pixelSize:12 }
          Text { visible: !window.acceptingInvite && (window.advanced || window.mode === "bootstrap"); text: "server"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: server
            visible: !window.acceptingInvite && (window.advanced || window.mode === "bootstrap")
            objectName: "accountServer"
            Layout.fillWidth: true
            text: Quickshell.env("WISP_ONBOARDING_SERVER") || ""
            placeholderText: window.mode === "register" ? "optional when encoded in invite" : "https://wisp.example.com"
            color: "#e8ecf3"
            placeholderTextColor: "#667085"
            font.family: "Hack"
            font.pixelSize: 13
            onEditingFinished: window.refreshSetup()
            background: Rectangle { color: "#1c202b"; border.color: server.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text { visible: window.mode !== "bootstrap" && !window.resetting && !window.acceptingInvite && (window.invitationEntry || window.advanced); text: "Invitation link"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: invite
            onTextChanged: window.invitationInputChanged()
            text: Quickshell.env("WISP_ONBOARDING_INVITE") || ""
            objectName: "accountInvite"
            visible: window.mode !== "bootstrap" && !window.resetting && !window.acceptingInvite && (window.invitationEntry || window.advanced)
            Layout.fillWidth: true
            placeholderText: "Invite link or word-and-number code"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            background: Rectangle { color: "#1c202b"; border.color: invite.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text { visible: window.mode === "bootstrap"; text: "one-time setup token"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: bootstrapToken
            visible: window.mode === "bootstrap"
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: "server bootstrap token"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            background: Rectangle { color: "#1c202b"; border.color: bootstrapToken.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text { visible: window.mode !== "login" && !window.resetting && !window.existingAccount; text: "Your name"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: displayName
            onTextChanged: if (window.mode === "register" && !window.usernameEdited) username.text = window.suggestUsername(text)
            objectName: "accountDisplayName"
            visible: window.mode !== "login" && !window.resetting && !window.existingAccount
            Layout.fillWidth: true
            placeholderText: "name shown to friends"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            background: Rectangle { color: "#1c202b"; border.color: displayName.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text { visible: !window.existingAccount && !window.resetting; text: window.mode === "register" && (!window.invitation || !window.invitation.legacy) ? "Username · friends can find you with this" : "username"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: username
            visible: !window.existingAccount && !window.resetting
            maximumLength: 32
            onTextEdited: window.usernameEdited = true
            objectName: "accountUsername"
            Layout.fillWidth: true
            placeholderText: "username"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            background: Rectangle { color: "#1c202b"; border.color: username.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          TextField {
            id: resetLink; objectName: "accountResetLink"; visible: window.resetting
            Layout.fillWidth: true; maximumLength: 2048; echoMode: TextInput.Password
            placeholderText: "Paste reset link or code"; Accessible.name: "Reset link from email"
            color: "#e8ecf3"; placeholderTextColor: "#667085"
            background: Rectangle { color: "#1c202b"; border.color: "#3b4353"; radius: 3 }
          }
          CheckBox {
            id: classicToggle; objectName: "classicSignIn"; visible: window.mode === "login" && !window.existingAccount
            text: "Classic account sign-in"; checked: window.classicSignIn
            enabled: !window.busy && !window.setupState.secure && !window.setupState.pending
            onToggled: { window.classicSignIn = checked; password.text = "" }
          }
          Text {
            Layout.fillWidth: true; wrapMode: Text.Wrap; color: "#8d96a8"; font.pixelSize: 12
            visible: window.mode === "login" && !window.existingAccount
            text: window.classicSignIn ? "Only for accounts that have not enabled encrypted backup. Use your original password."
              : "Secure sign-in restores your encrypted account backup. Use classic sign-in only for an older account that has not enabled backup."
          }
          Text { visible: !window.existingAccount; text: "password"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: password
            visible: !window.existingAccount
            maximumLength: 1024
            objectName: "accountPassword"
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: window.mode === "login" ? "password" : "at least 12 characters"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            onAccepted: if (window.mode === "login") window.submit()
            background: Rectangle { color: "#1c202b"; border.color: password.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text { visible: window.mode !== "login" && !window.existingAccount; text: "confirm password"; color: "#8d96a8"; font.family: "Hack"; font.pixelSize: 12 }
          TextField {
            id: confirmPassword
            visible: window.mode !== "login" && !window.existingAccount
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: "repeat password"
            color: "#e8ecf3"; placeholderTextColor: "#667085"; font.family: "Hack"; font.pixelSize: 13
            onAccepted: window.submit()
            background: Rectangle { color: "#1c202b"; border.color: confirmPassword.activeFocus ? "#2f8cff" : "#3b4353"; radius: 3 }
          }

          Text {
            visible: window.feedback !== ""
            text: window.feedback
            color: "#ff7777"
            font.family: "Hack"
            font.pixelSize: 12
            wrapMode: Text.Wrap
            Layout.fillWidth: true
          }

          Button {
            objectName: "accountSubmit"
            Layout.fillWidth: true
            Layout.preferredHeight: 38
            enabled: !window.busy && !previewProcess.running
            text: window.busy ? "working…" : window.resetting ? "Reset password" : window.acceptingInvite ? "Accept invitation" : window.mode === "login" ? "sign in" : window.mode === "bootstrap" ? "create owner account" : "create account"
            font.family: "Hack"
            font.pixelSize: 13
            onClicked: window.submit()
            background: Rectangle { radius: 3; color: parent.enabled ? "#2f8cff" : "#273140"; border.color: parent.enabled ? "#72afff" : "#3b4353" }
            contentItem: Text { text: parent.text; color: "#e8ecf3"; font: parent.font; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
          }
          Button {
            objectName: "forgotPassword"; visible: window.mode === "login"; enabled: !window.busy
            Layout.fillWidth: true; text: "Forgot password?"
            onClicked: Qt.openUrlExternally("https://wisp.you/account/forgot-password")
          }
          Button {
            text: "Enter a password reset link"; visible: window.mode === "login"; enabled: !window.busy
            Layout.fillWidth: true; onClicked: window.selectMode("reset")
          }
          ColumnLayout {
            Layout.fillWidth: true; visible: !!window.setupState.pending && !window.resetting
            Text { Layout.fillWidth: true; wrapMode: Text.Wrap; color: "#8d96a8"; text: "An interrupted account action is saved on this device." }
            Button { Layout.fillWidth: true; text: "Resume interrupted action"; enabled: !window.busy; onClicked: window.perform("resume") }
            Button { Layout.fillWidth: true; text: "Recover with secure password"; enabled: !window.busy && !!password.text; onClicked: window.perform("recover") }
            Button { Layout.fillWidth: true; text: "Recover with original password"; visible: window.setupState.pending === "migration"; enabled: !window.busy && !!password.text; onClicked: window.perform("recover_classic") }
            Button { Layout.fillWidth: true; text: "Cancel prepared action"; visible: !!window.setupState.can_cancel; enabled: !window.busy; onClicked: window.perform("cancel") }
          }
          Button {
            text: "Resume password reset"; visible: window.resetting; enabled: !window.busy
            Layout.fillWidth: true; onClicked: window.perform("resume_reset")
          }
          Button {
            objectName: "declineInvitation"
            visible: window.invitationEntry || !!window.invitation
            Layout.fillWidth: true
            text: "Decline"
            enabled: !window.busy
            onClicked: Qt.quit()
          }
        }
      }

      Timer { id: previewTimer; interval:250; onTriggered: {
        if (!window.modernLink(invite.text.trim())) return
        if (previewProcess.running) { restart(); return }
        previewProcess.currentInput = invite.text.trim()
        previewProcess.running = true
      } }
      Process {
        id: previewProcess
        property string currentInput: ""
        command:["wisp-account"]; stdinEnabled:true
        onStarted:write(JSON.stringify({action:"preview_invite",invite_code:currentInput}) + "\n")
        stdout:StdioCollector {id:previewOutput}
        stderr:StdioCollector {id:previewError}
        onExited:function(code) {
          if (currentInput !== invite.text.trim()) return
          if (code === 0) {
            try { window.resolvedInvitation = JSON.parse(previewOutput.text); window.resolvedInput = currentInput; window.feedback = "" }
            catch(error) { window.feedback = "Could not read invitation." }
          } else window.feedback = String(previewError.text || "Could not open invitation.").trim().replace(/^Error: /, "")
        }
      }
      Process {
        id: setupProcess; command: ["wisp-account"]; stdinEnabled: true
        property string requestedServer: ""
        onStarted: {
          requestedServer = window.acceptingInvite ? window.invitation.server : server.text.trim()
          write(JSON.stringify({action:"setup_status",server_url:requestedServer}) + "\n")
        }
        stdout: StdioCollector { id: setupOutput }
        stderr: StdioCollector {}
        onExited: function(code) {
          if (requestedServer !== (window.acceptingInvite ? window.invitation.server : server.text.trim())) return
          if (code === 0) {
            try { window.setupState = JSON.parse(setupOutput.text) } catch(error) { window.setupState = ({}) }
            if (window.setupState.secure || window.setupState.pending) window.classicSignIn = false
            if (window.setupState.pending && window.setupState.username) username.text = window.setupState.username
          }
        }
      }
      Process {
        id: accountProcess
        command: ["wisp-account"]
        stdinEnabled: true
        onStarted: {
          write(JSON.stringify({
          action: window.requestedAction || (window.existingAccount ? "accept_invite" : window.mode === "login" && window.classicSignIn ? "classic_login" : window.mode),
          server_url: window.acceptingInvite ? window.invitation.server : server.text.trim(),
          username: username.text.trim(),
          display_name: displayName.text.trim(),
          password: password.text,
          invite_code: invite.text.trim(),
          bootstrap_token: bootstrapToken.text.trim(),
          device_name: "Wisp desktop",
          reset_link: resetLink.text
        }) + "\n")
          password.text = ""; confirmPassword.text = ""; bootstrapToken.text = ""; resetLink.text = ""
        }
        stdout: StdioCollector { id: output }
        stderr: StdioCollector { id: errors }
        onExited: function(exitCode) {
          window.busy = false
          password.text = ""
          confirmPassword.text = ""
          bootstrapToken.text = ""
          if (exitCode === 0) {
            var result = ({})
            try { result = JSON.parse(output.text) } catch(error) {}
            if (result.reset) {
              window.selectMode("login")
              window.feedback = "Password reset. Sign in with your new password, then use a trusted device to restore backup access."
              return
            }
            if (result.cancelled) { window.setupState = ({}); window.refreshSetup(); return }
            window.feedback = result.warning || "Account saved. Starting Wisp…"
            finishTimer.interval = result.warning ? 5000 : 500
            finishTimer.start()
          } else {
            var lines = String(errors.text || "Account setup failed").trim().split("\n")
            window.feedback = lines.length ? lines[lines.length - 1].replace(/^Error: /, "") : "Account setup failed"
            window.refreshSetup()
          }
        }
      }

      Timer { id: finishTimer; interval: 500; onTriggered: Qt.quit() }
    }
  }
}
