.pragma library

function key(friend) { return String(friend.id || friend.display_name || "") }
function sorted(friends, favorites) {
  return friends.slice().sort(function(a, b) {
    var aRank = (favorites.indexOf(key(a)) >= 0 ? 0 : 2) + (a.online ? 0 : 1)
    var bRank = (favorites.indexOf(key(b)) >= 0 ? 0 : 2) + (b.online ? 0 : 1)
    if (aRank !== bRank) return aRank - bRank
    var nameOrder = String(a.display_name || "").toLocaleLowerCase().localeCompare(String(b.display_name || "").toLocaleLowerCase())
    return nameOrder || key(a).localeCompare(key(b))
  })
}
