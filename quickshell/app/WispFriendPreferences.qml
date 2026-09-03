import QtQuick
import Quickshell
import Quickshell.Io
import "FriendLogic.js" as FriendLogic

Item {
  id: root
  visible: false
  required property string account
  // JsonAdapter reloads arrays as Qt sequences; normalize into JS arrays so
  // favorites have the same behavior immediately after editing and on restart.
  readonly property var accountSettings: JSON.parse(JSON.stringify(preferences.accounts[account] || ({})))
  readonly property var favorites: Array.isArray(accountSettings.favorites) ? accountSettings.favorites : []
  readonly property bool collapsed: accountSettings.collapsed === true
  property string error: ""
  function save(favorites, collapsed) {
    if (!account) return
    var accounts = Object.assign({}, preferences.accounts)
    accounts[account] = {favorites: favorites, collapsed: collapsed}
    preferences.accounts = accounts
    error = ""
    settings.writeAdapter()
  }
  function isFavorite(friend) { return favorites.indexOf(FriendLogic.key(friend)) >= 0 }
  function toggleFavorite(friend) {
    var key = FriendLogic.key(friend)
    if (!key) return
    save(isFavorite(friend) ? favorites.filter(function(value) { return value !== key }) : favorites.concat([key]), collapsed)
  }
  function toggleCollapsed() { save(favorites, !collapsed) }
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/friends.json"
    blockLoading: true; blockWrites: true; atomicWrites: true
    watchChanges: true; printErrors: false
    onFileChanged: reload()
    onSaveFailed: root.error = "Couldn't save friend preferences on this device."
    JsonAdapter { id: preferences; property var accounts: ({}) }
  }
}
