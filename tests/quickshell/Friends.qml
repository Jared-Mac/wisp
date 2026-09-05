import QtQuick
import Quickshell
import "app" as Wisp
import "app/FriendLogic.js" as FriendLogic

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("FRIENDS_FAILED " + message) } }
  Wisp.WispFriendPreferences { id: prefs; account: "fixture-account" }
  Timer {
    interval: 200; running: true
    onTriggered: {
      var friends = [
        {id:"b",display_name:"beta",online:true}, {id:"a",display_name:"Alpha",online:true},
        {id:"d",display_name:"Delta",online:false}, {id:"c",display_name:"member_c",online:false},
        {id:"f",display_name:"Foxtrot",online:true}, {id:"e",display_name:"Echo",online:true},
        {id:"h",display_name:"Hotel",online:false}, {id:"g",display_name:"golf",online:false}
      ]
      var favorites = ["a","b","c","d"]
      test.check(FriendLogic.sorted(friends, favorites).map(function(f) { return f.id }).join("") === "abcdefgh", "four priorities, alphabetical order, case insensitive")
      test.check(friends[0].id === "b", "sort does not mutate server snapshot")
      if (Quickshell.env("WISP_FRIENDS_RELOAD") === "1") {
        test.check(prefs.favorites.join() === "a" && prefs.collapsed, "favorites and collapsed state survive restart")
        prefs.account = "another-account"
        test.check(prefs.favorites.length === 0 && !prefs.collapsed, "preferences isolated by account")
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
      }
      console.log(test.failed ? "FRIENDS_FAILED" : "FRIENDS_OK")
      Qt.quit()
    }
  }
}
