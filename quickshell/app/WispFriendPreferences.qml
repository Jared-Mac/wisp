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
  readonly property bool membersCollapsed: accountSettings.membersCollapsed === true
  readonly property bool trayCollapsed: typeof accountSettings.trayCollapsed === "boolean" ? accountSettings.trayCollapsed : collapsed
  readonly property bool trayMembersCollapsed: typeof accountSettings.trayMembersCollapsed === "boolean" ? accountSettings.trayMembersCollapsed : membersCollapsed
  property string error: ""
  function collapsedFor(presentation) { return presentation === "panel" ? trayCollapsed : collapsed }
  function membersCollapsedFor(presentation) { return presentation === "panel" ? trayMembersCollapsed : membersCollapsed }
  function friendsViewFor(presentation) { return accountSettings[presentation === "panel" ? "trayFriendsView" : "friendsView"] === true }
  function setFriendsView(value, presentation) {
    var patch={}
    patch[presentation === "panel" ? "trayFriendsView" : "friendsView"]=!!value
    save(patch)
  }
  function save(patch) {
    if (!account) return
    // The Omarchy panel can run in another process. Merge its latest fields
    // before writing so a desktop edit retains the panel's saved layout.
    settings.reload()
    settings.text()
    var accounts = Object.assign({}, preferences.accounts)
    // Seed both tray fields from the old shared values before the first edit.
    // Later edits to either surface must not change the other's defaults.
    accounts[account] = Object.assign({trayCollapsed:collapsed,trayMembersCollapsed:membersCollapsed},accountSettings,patch)
    preferences.accounts = accounts
    error = ""
    settings.writeAdapter()
  }
  function isFavorite(friend) { return favorites.indexOf(FriendLogic.key(friend)) >= 0 }
  function toggleFavorite(friend) {
    var key = FriendLogic.key(friend)
    if (!key) return
    save({favorites:isFavorite(friend) ? favorites.filter(function(value) { return value !== key }) : favorites.concat([key])})
  }
  function toggleCollapsed(presentation) {
    var patch={}
    patch[presentation === "panel" ? "trayCollapsed" : "collapsed"]=!collapsedFor(presentation)
    save(patch)
  }
  function setMemberPreference(key, value, presentation) {
    if (key === "membersCollapsed" && presentation === "panel") key="trayMembersCollapsed"
    var patch={}
    patch[key]=!!value
    save(patch)
  }
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/friends.json"
    blockLoading: true; blockAllReads: true; blockWrites: true; atomicWrites: true
    watchChanges: true; printErrors: false
    onFileChanged: reload()
    onSaveFailed: root.error = "Couldn't save friend preferences on this device."
    JsonAdapter { id: preferences; property var accounts: ({}) }
  }
}
