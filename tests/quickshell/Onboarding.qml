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
      console.log("ONBOARDING_OK")
      window.closed()
    }
  }
  Timer { interval: 5000; running: true; onTriggered: Qt.exit(1) }
}
