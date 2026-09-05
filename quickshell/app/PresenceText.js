.pragma library

function description(presence, self) {
  var descriptions = self ? {
    open: "Friends can join you in voice without asking.",
    knock: "Friends need your approval to join voice.",
    closed: "Block direct voice joins and knocks.",
    away: "Show you're away and block voice joins and knocks."
  } : {
    open: "Join them in voice without asking.",
    knock: "Knock and wait for them to accept.",
    closed: "Not accepting voice joins or knocks.",
    away: "Not accepting voice joins or knocks."
  }
  return descriptions[presence] || descriptions.away
}
