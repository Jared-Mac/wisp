.pragma library

function description(presence, self) {
  var descriptions = self ? {
    open: "Open: Friends can join you in voice without asking.",
    knock: "Knock: Friends must knock first. Accept their knock to let them join you in voice.",
    closed: "Closed: You're here, but direct voice joins and knocks are blocked.",
    away: "Away: Shows you're away and blocks direct voice joins and knocks."
  } : {
    open: "Open: You can join them in voice without asking.",
    knock: "Knock: Send a knock and wait for them to accept before joining voice.",
    closed: "Closed: They aren't accepting direct voice joins or knocks.",
    away: "Away: They're away and aren't accepting direct voice joins or knocks."
  }
  return (descriptions[presence] || descriptions.away)
    + (self ? " Saved for this server. Room access and text chat are unchanged." : "")
}
