// Local, persistent conversation identities; never derived from tile positions,
// labels, presence, or the currently focused pane.
var swatches = ["#7fa9cf", "#cb8897", "#c5b16d", "#aa93d0",
                "#78b5a3", "#c99b75", "#70b6c7", "#a8b97a"]

function parse(encoded) {
  var result = Object.create(null), source
  try { source = JSON.parse(encoded) } catch (_) { return result }
  if (!source || typeof source !== "object" || Array.isArray(source)) return result
  Object.keys(source).forEach(function(id) {
    if (id && typeof source[id] === "string" && /^#[0-9a-f]{6}$/i.test(source[id]))
      result[id] = source[id].toLowerCase()
  })
  return result
}

function candidate(index) {
  if (index < swatches.length) return swatches[index]
  // Continue around the hue wheel for larger histories without reusing the
  // initial eight swatches. Many colors cannot all be perceptually distinct.
  var hue = ((index - swatches.length) * 137.508 + 23) % 360
  var saturation = 0.38 + (Math.floor(index / 24) % 3) * 0.07
  var lightness = 0.62
  var chroma = (1 - Math.abs(2 * lightness - 1)) * saturation
  var x = chroma * (1 - Math.abs((hue / 60) % 2 - 1))
  var rgb = hue < 60 ? [chroma,x,0] : hue < 120 ? [x,chroma,0]
    : hue < 180 ? [0,chroma,x] : hue < 240 ? [0,x,chroma]
    : hue < 300 ? [x,0,chroma] : [chroma,0,x]
  return "#" + rgb.map(function(value) {
    return ("0" + Math.round((value + lightness - chroma / 2) * 255).toString(16)).slice(-2)
  }).join("")
}

function assign(encoded, conversationIds) {
  var colors = parse(encoded), used = Object.create(null), index = 0
  Object.keys(colors).forEach(function(id) { used[colors[id]] = true })
  conversationIds.map(String).filter(function(id) { return id !== "" }).sort().forEach(function(id) {
    if (colors[id]) return
    var color
    do { color = candidate(index++) } while (used[color])
    colors[id] = color
    used[color] = true
  })
  return JSON.stringify(colors)
}
