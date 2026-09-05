import QtQuick
import Quickshell

ShellRoot {
  id: test
  property var onboarding
  function findObject(item, name, visited) {
    if (!item || visited.indexOf(item) >= 0) return null
    visited.push(item)
    if (item.objectName === name) return item
    if (item.contentItem) {
      var contentMatch = findObject(item.contentItem, name, visited)
      if (contentMatch) return contentMatch
    }
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var match = findObject(children[i], name, visited)
      if (match) return match
    }
    return null
  }
  function findChild(item, name) { return findObject(item, name, []) }
  Component.onCompleted: {
    var component = Qt.createComponent("onboarding/shell.qml")
    if (component.status !== Component.Ready) {
      console.error("ONBOARDING_FAILED: " + component.errorString())
      Qt.exit(1)
      return
    }
    onboarding = component.createObject(test)
  }
  Timer {
    interval: 400; running: true
    onTriggered: {
      var window = test.onboarding.accountWindow
      function check(condition, message) {
        if (!condition) {
          console.error("ONBOARDING_FAILED: " + message)
          Qt.exit(1)
          throw new Error(message)
        }
      }
      check(!!window, "account window loads")
      var expected = Quickshell.env("WISP_ONBOARDING_MODE") === "register" ? "register" : "login"
      check(window.mode === expected, "default login and explicit registration modes")
      var name = test.findChild(window, "accountDisplayName")
      var password = test.findChild(window, "accountPassword")
      var invite = test.findChild(window, "accountInvite")
      var create = test.findChild(window, "accountMode-register")
      check(create.text === "[create account]", "create account action is explicit")
      window.selectMode("login")
      check(!name.visible, "sign in does not ask for a new display name")
      window.submit()
      check(!window.busy && window.feedback.length > 0, "missing login fields do not submit")
      password.text = "fixture password"
      invite.text = "fixture invitation"
      create.clicked()
      check(window.mode === "register" && name.visible, "create account opens registration")
      check(password.text === "" && invite.text === "fixture invitation", "switching clears passwords and preserves the invite")
      window.busy = true
      window.selectMode("login")
      check(window.mode === "register", "cannot change flow during submission")
      window.busy = false
      test.findChild(window, "accountMode-login").clicked()
      check(window.mode === "login", "can return to sign in")
      // A valid invitation becomes a focused screen without losing legacy entry.
      var payload = {v:1, server:"https://friends.example.com", token:"test-only-token"}
      var encoded = Qt.btoa(JSON.stringify(payload)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")
      window.selectMode("register")
      invite.text = "wisp-invite:" + encoded
      check(window.acceptingInvite, "valid invitation is recognized")
      check(!invite.visible && !test.findChild(window, "accountServer").visible, "connection fields are hidden")
      name.text = "Alex Example"
      check(test.findChild(window, "accountUsername").text === "alex_example", "username follows chosen name")
      var username = test.findChild(window, "accountUsername")
      username.text = "my_handle"
      username.textEdited()
      name.text = "Another Name"
      check(username.text === "my_handle", "custom username is preserved")
      check(test.findChild(window, "accountSubmit").text === "Join server", "invitation has a clear join action")
      window.selectMode("login")
      check(window.acceptingInvite && !name.visible, "existing accounts can accept an invitation")
      check(window.parseInvitation("wisp-invite:broken") === null, "malformed payload is rejected")
      for (var bad of ["http://friends.example.com", "https://user:password@friends.example.com", "https://friends.example.com/redirect", "https://friends.example.com?token=secret"]) {
        payload.server = bad
        var badEncoded = Qt.btoa(JSON.stringify(payload)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")
        check(window.parseInvitation("wisp-invite:" + badEncoded) === null, "unsafe invite destination rejected")
      }
      invite.text = "wisp-invite:broken"
      window.submit()
      check(!window.busy && window.feedback.indexOf("invalid") >= 0, "invalid invite never submits")
      console.log("ONBOARDING_OK")
      window.closed()
    }
  }
  Timer { interval: 5000; running: true; onTriggered: Qt.exit(1) }
}
