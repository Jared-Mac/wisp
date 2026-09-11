import QtQuick
import Quickshell.Io

Item {
  id:root
  required property var bridge
  readonly property var catalog:bridge.serverCatalog
  readonly property string accountKey: {
    if(!bridge.receivedSnapshot || !catalog.length)return ""
    var first=String(catalog[0].id),state=bridge.serverStates.filter(function(s){return String(s.server.id)===first})[0]
    return state && (state.self || {}).id ? JSON.stringify([first,String(state.self.id)]) : ""
  }
  readonly property var saved:JSON.parse(JSON.stringify(preferences.accounts[accountKey] || {}))
  readonly property var order: {
    var current=catalog.map(function(s){return String(s.id)}), known=Array.isArray(saved.order) ? saved.order : []
    return known.filter(function(id){return current.indexOf(id)>=0}).concat(current.filter(function(id){return known.indexOf(id)<0}))
  }
  readonly property var homeIds: {
    var available=catalog.map(function(s){return String(s.id)}),homes=Array.isArray(saved.homes) ? saved.homes : []
    homes=homes.filter(function(id){return available.indexOf(id)>=0})
    return homes.length ? homes : available.slice(0,1)
  }
  readonly property var sorted:catalog.slice().sort(function(a,b) {
    var left=String(a.id),right=String(b.id)
    return Number(homeIds.indexOf(right)>=0)-Number(homeIds.indexOf(left)>=0) || order.indexOf(left)-order.indexOf(right)
  })
  property string error:""
  function isHome(id) {return homeIds.indexOf(String(id))>=0}
  function save(patch) {
    if(!accountKey)return
    settings.reload();settings.text()
    preferences.accounts=Object.assign({},preferences.accounts,{[accountKey]:Object.assign({},saved,patch)})
    settings.writeAdapter()
  }
  function remember() {
    if(!accountKey)return
    // Load the adapter before deriving defaults, so startup cannot overwrite
    // homes or ordering saved by a previous app/tray process.
    settings.text()
    if(JSON.stringify(saved.order)!==JSON.stringify(order) || JSON.stringify(saved.homes)!==JSON.stringify(homeIds))
      save({order:order,homes:homeIds})
  }
  function setHome(id,enabled) {
    id=String(id)
    if(!catalog.some(function(s){return String(s.id)===id}))return
    var homes=homeIds.filter(function(s){return s!==id})
    if(enabled)homes.push(id)
    if(homes.length)save({homes:homes})
  }
  function moveBy(id,offset) {
    id=String(id)
    var group=sorted.filter(function(s){return isHome(s.id)===isHome(id)}).map(function(s){return String(s.id)})
    var from=group.indexOf(id),to=Math.max(0,Math.min(group.length-1,from+offset))
    if(from<0 || from===to)return
    group.splice(from,1);group.splice(to,0,id)
    var next=order.slice(),indices=[]
    next.forEach(function(s,i){if(group.indexOf(s)>=0)indices.push(i)})
    indices.forEach(function(index,i){next[index]=group[i]})
    save({order:next})
  }
  function canMove(id,offset) {
    var group=sorted.filter(function(s){return isHome(s.id)===isHome(id)}).map(function(s){return String(s.id)})
    var index=group.indexOf(String(id))+offset
    return index>=0 && index<group.length
  }
  onAccountKeyChanged:Qt.callLater(remember)
  onCatalogChanged:Qt.callLater(remember)
  FileView {
    id:settings;path:root.bridge.configHome+"/wisp/servers.json"
    blockLoading:true;blockAllReads:true;blockWrites:true;atomicWrites:true;watchChanges:true;printErrors:false
    onFileChanged:reload()
    onSaveFailed:root.error="Couldn't save server homes and order."
    onSaved:root.error=""
    JsonAdapter {id:preferences;property var accounts:({})}
  }
}
