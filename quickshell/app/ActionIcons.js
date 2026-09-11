.pragma library

// Wisp-owned, 24px outline symbols. No font glyph dependency or remote assets.
var paths = {
  inbox: '<path d="M3 4h18v16H3zM3 13h5l2 3h4l2-3h5"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="2"/><path d="m3 18 6-6 4 4 4-7 4 5"/>',
  add: '<path d="M12 5v14M5 12h14"/>',
  close: '<path d="m6 6 12 12M18 6 6 18"/>',
  more: '<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  back: '<path d="m14 6-6 6 6 6"/>',
  returnChat: '<path d="m9 4-5 5 5 5M4 9h9a7 7 0 0 1 0 14"/>',
  reply: '<path d="m9 4-6 6 6 6M3 10h10a8 8 0 0 1 8 8v2"/>',
  forwardMessage: '<path d="m15 4 6 6-6 6M21 10H11a8 8 0 0 0-8 8v2"/>',
  pin: '<path d="m15 3 6 6-3 1-4 4v4l-8-8h4l4-4zM9 15l-6 6"/>',
  forward: '<path d="m10 6 6 6-6 6"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  settings: '<path d="m10 3-.7 2.4-2 .9L5 5.5 3 9l1.8 1.6v2.8L3 15l2 3.5 2.3-.8 2 .9L10 21h4l.7-2.4 2-.9 2.3.8 2-3.5-1.8-1.6v-2.8L21 9l-2-3.5-2.3.8-2-.9L14 3z"/><circle cx="12" cy="12" r="3"/>',
  microphone: '<rect x="9" y="2" width="6" height="13" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M9 22h6"/>',
  headphones: '<path d="M4 14v-3a8 8 0 0 1 16 0v3"/><rect x="3" y="12" width="4" height="8" rx="2"/><rect x="17" y="12" width="4" height="8" rx="2"/>',
  camera: '<rect x="3" y="5" width="12" height="14" rx="3"/><path d="m15 10 6-4v12l-6-4"/>',
  screen: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4m-3-10 3-3 3 3M12 8v6"/>',
  phone: '<path d="m7 3 3 5-3 3a15 15 0 0 0 6 6l3-3 5 3-1 4C11 23 1 13 3 4z"/>',
  disconnect: '<path d="M4 15v-3c4-5 12-5 16 0v3a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-3a9 9 0 0 0-6 0v3a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1Z"/>',
  invite: '<circle cx="9" cy="8" r="4"/><path d="M2 21v-2a7 7 0 0 1 14 0v2M19 7v6M16 10h6"/>',
  people: '<circle cx="9" cy="8" r="4"/><path d="M2 21v-2a7 7 0 0 1 14 0v2M17 4a4 4 0 0 1 0 8M19 15a5 5 0 0 1 3 5"/>',
  profile: '<circle cx="12" cy="8" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2"/>',
  chat: '<path d="M21 11a9 8 0 0 1-9 8H8l-5 3 1-6a8 8 0 0 1-1-5 9 8 0 0 1 18 0Z"/><path d="M8 9h8M8 13h5"/>',
  room: '<path d="M4 4h14v17M8 21H2M8 21V5l10-2v18z"/><path d="M14 12h.1"/>',
  home: '<path d="m3 10 9-8 9 8M5 9v12h14V9M9 21v-8h6v8"/>',
  lock: '<rect x="4" y="10" width="16" height="12" rx="3"/><path d="M8 10V6a4 4 0 0 1 8 0v4M12 15v2"/>',
  bell: '<path d="M5 10a7 7 0 0 1 14 0v5l2 3H3l2-3zM9 21h6"/>',
  moon: '<path d="M20 15A9 9 0 0 1 9 3a9 9 0 1 0 11 12Z"/>',
  palette: '<path d="M12 3a9 9 0 1 0 0 18h1a2 2 0 0 0 1-4c-2-1-1-3 1-3h3c5 0 4-11-6-11Z"/><circle cx="7" cy="9" r=".8"/><circle cx="11" cy="6" r=".8"/><circle cx="16" cy="8" r=".8"/><circle cx="6" cy="14" r=".8"/>',
  layout: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M9 12h12"/>',
  focus: '<path d="M9 3H3v6M15 3h6v6M3 15v6h6M21 15v6h-6"/><path d="m8 8-5-5m13 5 5-5M8 16l-5 5m13-5 5 5"/>',
  grip: '<path d="M8 5h.1M16 5h.1M8 12h.1M16 12h.1M8 19h.1M16 19h.1" stroke-width="3"/>',
  window: '<path d="M14 3h7v7M21 3l-9 9M10 3H3v18h18v-7"/>',
  tile: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18m9-12-4 3 4 3"/>',
  hash: '<path d="m10 3-3 18M17 3l-3 18M4 9h17M3 15h17"/>',
  play: '<path d="m8 4 13 8-13 8z"/>',
  stop: '<rect x="5" y="5" width="14" height="14" rx="2"/>',
  refresh: '<path d="M20 7a9 9 0 0 0-16 1M4 3v5h5M4 17a9 9 0 0 0 16-1m0 5v-5h-5"/>',
  copy: '<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>',
  edit: '<path d="m15 4 5 5M4 15 16 3a3 3 0 0 1 5 5L9 20l-6 1z"/>',
  trash: '<path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/>',
  save: '<path d="M4 3h13l4 4v14H3V3zM8 3v6h8V3M7 21v-8h10v8"/>',
  folder: '<path d="M3 6V4h7l2 3h9v14H3z"/>',
  download: '<path d="M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4"/>',
  upload: '<path d="M12 16V3m-5 5 5-5 5 5M4 17v4h16v-4"/>',
  emoji: '<circle cx="12" cy="12" r="9"/><path d="M8 9h.1M16 9h.1M7 14a5 5 0 0 0 10 0"/>',
  send: '<path d="m3 3 19 9-19 9 4-9zM7 12h15"/>',
  attach: '<path d="m9 16 8-8a3 3 0 0 0-4-4L4 13a5 5 0 0 0 7 7l9-9M7 14l8-8"/>',
  star: '<path d="m12 2 3 6 7 1-5 5 1 8-6-4-6 4 1-8-5-5 7-1z"/>',
  search: '<circle cx="10" cy="10" r="7"/><path d="m15 15 6 6"/>',
  shield: '<path d="m12 2 9 4v6c0 5-9 10-9 10S3 17 3 12V6zM8 12l3 3 5-6"/>',
  sliders: '<path d="M4 3v18M12 3v18M20 3v18M1 8h6M9 16h6M17 8h6"/>',
  volume: '<path d="M3 9h4l5-5v16l-5-5H3zM16 8a6 6 0 0 1 0 8M19 5a10 10 0 0 1 0 14"/>',
  soundboard: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  key: '<circle cx="8" cy="8" r="5"/><path d="m12 12 9 9M17 17l3-3M14 14l3-3"/>',
  keyboard: '<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.1M10 9h.1M14 9h.1M18 9h.1M6 13h.1M18 13h.1M9 15h6"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.1"/>',
  wisp: '<path d="M5 20v-9a7 7 0 0 1 14 0v9l-4-2-3 3-3-3z"/><path d="M9 10h.1M15 10h.1M10 14q2 2 4 0"/>'
}
function source(name, color) {
  var off = /-off$/.test(name), key = off ? name.slice(0,-4) : name
  if (!paths[key]) return ""
  return "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="' + color + '" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">' + paths[key] + (off ? '<path d="m3 3 18 18"/>' : '') + '</svg>')
}
function label(text) {
  var value = String(text || "").replace(/^\[|\]$/g, "").trim()
  var aliases = {"d/c":"Disconnect",inv:"Invite",stngs:"Settings",msg:"Message","+ add chat":"Add chat",cam:"Camera",browser:"Open in browser",tile:"Move to tile",window:"Open window"}
  return aliases[value.toLowerCase()] || (value ? value.charAt(0).toUpperCase() + value.slice(1) : "")
}
function icon(text) {
  var s = label(text).toLowerCase()
  if (/^(···|⋯|…|:)$/.test(s)) return "more"
  if (/^[+＋]$/.test(s)) return "add"
  if (/^(×|✕|x)$/.test(s)) return "close"
  if (/^[‹<]$/.test(s)) return "back"
  if (/^[›>]$/.test(s)) return "forward"
  if (s === "⠿") return "grip"
  if (s === "☺") return "emoji"
  if (/^(unpin|pin|pinned)\b/.test(s)) return "pin"
  if (/^reply\b/.test(s)) return "reply"
  if (/^forward\b/.test(s)) return "forwardMessage"
  if (/server (mute|unmute)|server (deafen|undeafen)|role|admin|permission/.test(s)) return "shield"
  if (/disconnect/.test(s)) return "disconnect"
  if (/stop|leave/.test(s)) return "stop"
  if (/unmute/.test(s)) return /notification/.test(s) ? "bell" : "microphone"
  if (/mute/.test(s)) return /notification/.test(s) ? "bell-off" : "microphone-off"
  if (/undeafen/.test(s)) return "headphones"
  if (/deafen/.test(s)) return "headphones-off"
  if (/camera|webcam/.test(s)) return "camera"
  if (/share|screen/.test(s)) return "screen"
  if (/invite/.test(s)) return "invite"
  if (/join|call/.test(s)) return "phone"
  if (/knock/.test(s)) return "bell"
  if (/settings/.test(s)) return "settings"
  if (/home/.test(s)) return "home"
  if (/watch|play|test|preview/.test(s)) return "play"
  if (/window|pop out|open app|browser/.test(s)) return "window"
  if (/return|anchor|tile/.test(s)) return "tile"
  if (/layout|split|activity/.test(s)) return "layout"
  if (/refresh|reset|restore|default|retry/.test(s)) return "refresh"
  if (/delete|remove|revoke|clear (chat|history)|discard/.test(s)) return "trash"
  if (/cancel|close|clear|dismiss/.test(s)) return "close"
  if (/copy/.test(s)) return "copy"
  if (/edit|rename/.test(s)) return "edit"
  if (/save|apply/.test(s)) return "save"
  if (/download|export|backup/.test(s)) return "download"
  if (/upload|import/.test(s)) return "upload"
  if (/emoji|react/.test(s)) return "emoji"
  if (/choose|browse|file/.test(s)) return "folder"
  if (/password|recovery/.test(s)) return "key"
  if (/privacy|security|encryption|closed/.test(s)) return "lock"
  if (/appearance|palette|color|theme/.test(s)) return "palette"
  if (/profile|account/.test(s)) return "profile"
  if (/notification|sound/.test(s)) return "bell"
  if (/audio|microphone/.test(s)) return "microphone"
  if (/video|device/.test(s)) return "screen"
  if (/group|member|people/.test(s)) return "people"
  if (/chat|message|dm/.test(s)) return "chat"
  if (/room/.test(s)) return "room"
  if (/channel/.test(s)) return "hash"
  if (/create|add|new/.test(s)) return "add"
  if (/send/.test(s)) return "send"
  if (/accept|yes|confirm|enable|on$/.test(s)) return "check"
  return "" // Full descriptive text remains a button, never an arbitrary symbol.
}
function symbolOnly(text) { return /^[+＋×✕☺⠿‹›<>]$/.test(text) || /^(···|⋯|…|:)$/.test(text) }
function accessible(text) {
  return {add:"Add",close:"Close",more:"More options",grip:"Tile layout",emoji:"Choose emoji"}[icon(text)] || label(text)
}
