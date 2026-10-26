// Presentation only: privileged package changes remain explicit host commands.
function packageInfo(id) {
  var parts=String(id || "").split("-")
  return {id:String(id || ""), author:parts.shift() || "", version:parts.pop() || "", name:parts.join("-") || String(id || "")}
}
function roots(text) { return String(text || "").split(/\r?\n/).map(function(s){return s.trim()}).filter(function(s){return !!s}) }
function mods(view,text) {
  var direct=roots(text), values={}, order=[]
  function add(id,artifact) {
    if(!values[id]){values[id]=packageInfo(id);order.push(id)}
    var row=values[id]
    row.direct=direct.indexOf(id)>=0
    row.local=!!artifact && artifact.source==="local" || row.author==="Local"
    row.source=row.local ? "Private ZIP" : "Thunderstore"
    if(artifact){row.sha256=artifact.sha256 || "";row.size=artifact.size || 0}
    return row
  }
  direct.forEach(function(id){add(id,null)})
  ;((view || {}).archives || []).forEach(function(file){add(file.package,file)})
  ;((view || {}).files || []).forEach(function(file){
    // A root removed from the unsaved draft must not reappear as a dependency.
    if(((view || {}).packages || []).indexOf(file.package)>=0 && direct.indexOf(file.package)<0)return
    add(file.package,file).resolved=true
  })
  return order.map(function(id){return values[id]}).sort(function(a,b){return a.name.localeCompare(b.name)})
}
function size(bytes) { return bytes>=1048576 ? (bytes/1048576).toFixed(1)+" MB" : bytes>=1024 ? Math.round(bytes/1024)+" KB" : bytes+" B" }
function validDependency(value) { return /^[A-Za-z0-9_]+-[A-Za-z0-9_]+-\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$/.test(value) && value.indexOf("Local-")!==0 }

function upsert(text,id) {
  var incoming=packageInfo(id), result=roots(text).filter(function(existing){var old=packageInfo(existing);return old.author!==incoming.author || old.name!==incoming.name})
  result.push(id);return result.join("\n")
}
