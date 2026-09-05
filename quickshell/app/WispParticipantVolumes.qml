import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  required property string account
  property bool ready: false
  property string error: ""
  readonly property var volumes: preferences.accounts[account] || ({})
  readonly property var mutedPeople: preferences.mutedAccounts[account] || ({})
  signal saved()
  function personKey(person) { return String(person.server_id || "local") + "/" + String(person.id || "") }
  function isMuted(person) { return !!(preferences.mutedAccounts[String(person.account_id || account)] || {})[personKey(person)] }
  function setMuted(person, muted) {
    if (!ready || !account || !person.id) return
    var accountId = String(person.account_id || account)
    var next = Object.assign({}, preferences.mutedAccounts[accountId] || {}), accounts = Object.assign({}, preferences.mutedAccounts)
    next[personKey(person)] = !!muted; accounts[accountId] = next; preferences.mutedAccounts = accounts
    error = ""; saveDelay.restart()
  }
  function effectiveVolumeFor(person) { return isMuted(person) ? 0 : volumeFor(person) }
  function volumeFor(person) {
    var values = preferences.accounts[String(person.account_id || account)] || {}
    var value = values[personKey(person)]
    if (value === undefined) value = values[String(person.id || "")] // Existing preferences migrate on edit.
    return value === undefined ? 100 : Math.max(0, Math.min(200, Number(value)))
  }
  function setVolume(person, value) {
    if (!ready || !account || !person.id) return
    var accountId = String(person.account_id || account)
    var next = Object.assign({}, preferences.accounts[accountId] || {}), accounts = Object.assign({}, preferences.accounts)
    next[personKey(person)] = Math.max(0, Math.min(200, Math.round(value)))
    accounts[accountId] = next; preferences.accounts = accounts
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
    JsonAdapter { id: preferences; property var accounts: ({}); property var mutedAccounts: ({}) }
  }
}
