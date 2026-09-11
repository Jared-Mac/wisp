import QtQuick
import Quickshell
import "app" as Wisp
import "app/FriendLogic.js" as FriendLogic

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("FRIENDS_FAILED " + message) } }
  Wisp.WispFriendPreferences { id: prefs; account: "fixture-account" }
  Wisp.WispFriendPreferences { id: peer; account: "fixture-account" }
  Wisp.WispFriendPreferences { id: legacy; account: "legacy-account" }
  Timer {
    interval: 200; running: true
    onTriggered: {
      var friends = [
        {id:"b",display_name:"beta",online:true}, {id:"a",display_name:"Alpha",online:true},
        {id:"d",display_name:"Delta",online:false}, {id:"c",display_name:"Cedar",online:false},
        {id:"f",display_name:"Foxtrot",online:true}, {id:"e",display_name:"Echo",online:true},
        {id:"h",display_name:"Hotel",online:false}, {id:"g",display_name:"golf",online:false}
      ]
      var favorites = ["a","b","c","d"]
      test.check(FriendLogic.sorted(friends, favorites).map(function(f) { return f.id }).join("") === "abcdefgh", "four priorities, alphabetical order, case insensitive")
      test.check(friends[0].id === "b", "sort does not mutate server snapshot")
      if (Quickshell.env("WISP_FRIENDS_RELOAD") === "1") {
        test.check(prefs.favorites.join() === "a" && prefs.collapsed, "favorites and collapsed state survive restart")
        test.check(!prefs.trayCollapsed && !prefs.membersCollapsed && prefs.trayMembersCollapsed,"app and tray section choices persist independently")
        test.check(!legacy.collapsed && legacy.trayCollapsed && !legacy.membersCollapsed && legacy.trayMembersCollapsed,"old shared preferences migrate without coupling later changes")
        prefs.account = "another-account"
        test.check(prefs.favorites.length === 0 && !prefs.collapsed, "preferences isolated by account")
        test.check(!prefs.trayCollapsed && !prefs.trayMembersCollapsed,"tray choices are also isolated by account")
        prefs.account = "fixture-account"
        test.check(prefs.isFavorite({id:"a",display_name:"renamed"}), "favorite survives display-name change")
      } else {
        prefs.toggleFavorite(friends[1])
        test.check(prefs.isFavorite(friends[1]), "favorite added")
        prefs.toggleFavorite(friends[1])
        test.check(!prefs.isFavorite(friends[1]), "favorite removed")
        prefs.toggleFavorite(friends[1])
        prefs.toggleCollapsed()
        test.check(prefs.collapsed && !prefs.error, "collapsed state saved")
        test.check(!prefs.trayCollapsed,"first desktop collapse preserves the tray default")
        prefs.setMemberPreference("membersCollapsed",true,"panel")
        peer.toggleCollapsed("panel")
        test.check(peer.collapsed && peer.trayCollapsed && peer.trayMembersCollapsed,"another preference instance retains desktop fields")
        prefs.toggleFavorite(friends[0])
        test.check(prefs.trayCollapsed && prefs.trayMembersCollapsed,"favorite edits retain the tray's latest choices")
        peer.toggleCollapsed("panel")
        test.check(peer.favorites.length===2 && !peer.trayCollapsed && peer.collapsed,"tray edits retain shared favorites")
        prefs.toggleFavorite(friends[0])
        test.check(legacy.collapsed && legacy.trayCollapsed && legacy.membersCollapsed && legacy.trayMembersCollapsed,"old shared choices are preserved initially")
        legacy.toggleCollapsed()
        legacy.setMemberPreference("membersCollapsed",false)
        test.check(!legacy.collapsed && legacy.trayCollapsed && !legacy.membersCollapsed && legacy.trayMembersCollapsed,"first app edit freezes the tray's legacy choice")
      }
      console.log(test.failed ? "FRIENDS_FAILED" : "FRIENDS_OK")
      Qt.quit()
    }
  }
}
