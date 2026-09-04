import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  required property string account
  property bool ready: false
  property string error: ""
  readonly property var volumes: preferences.accounts[account] || ({})
  signal saved()
  function volumeFor(person) {
    var value = volumes[String(person.id || "")]
    return value === undefined ? 100 : Math.max(0, Math.min(200, Number(value)))
  }
  function setVolume(person, value) {
    if (!ready || !account || !person.id) return
    var next = Object.assign({}, volumes), accounts = Object.assign({}, preferences.accounts)
    next[String(person.id)] = Math.max(0, Math.min(200, Math.round(value)))
    accounts[account] = next; preferences.accounts = accounts
    error = ""; saveDelay.restart()
  }
  Timer { id: saveDelay; interval: 150; onTriggered: storage.writeAdapter() }
  FileView {
    id: storage
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/participant-volumes.json"
    blockLoading: true; blockWrites: true; atomicWrites: true; watchChanges: true; printErrors: false
    onLoaded: root.ready = true
    onLoadFailed: root.ready = true
    onFileChanged: reload()
    onSaved: root.saved()
    onSaveFailed: root.error = "Couldn't save local participant volumes."
    JsonAdapter { id: preferences; property var accounts: ({}) }
  }
}
