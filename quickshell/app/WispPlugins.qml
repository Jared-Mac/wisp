import QtQuick

Item {
  id: root
  required property var bridge
  property var installed: []
  property string selected: ""
  property string conversation: ""
  property string server: ""
  readonly property bool dirty: !!view && (view.fields || []).some(function(f){return String(fields[f.id] || "")!==String(f.value || "")})
  property var review: null
  property var archiveReview: null
  property var view: null
  property var fields: ({})
  property var shared: []
  property double generation: 0
  property string digest: ""
  property string folder: ""
  property string folderUrl: ""
  property string feedback: ""
  property bool busy: false
  property int epoch: 0
  function reset() { epoch++; busy=false; view=null; shared=[]; review=null;archiveReview=null }
  function request(name,args) {
    if (busy) return false
    var values=Object.assign({},bridge.scopeForConversation(conversation),{server_id:server || String(bridge.activeServer.id || ""),plugin_id:selected,generation:generation},args || {})
    var id=bridge.send(name,values)
    if (!id) { feedback="Wisp is disconnected.";return false }
    busy=true; feedback=name==="plugin_resolve" ? "Resolving and verifying Thunderstore packages…" : name==="plugin_apply" ? "Preparing your local profile…" : "Working…"
    bridge.requests[id]={kind:"plugins",epoch:epoch,command:name}
    return true
  }
  function refresh() { request("plugin_list",{}) }
  function open(id,scope) { if(busy)return;selected=id;conversation=scope;server=String(bridge.scopeForConversation(scope).server_id);view=null;shared=[];archiveReview=null;folder="";folderUrl="";if(id)request("plugin_open",{}) }
  function setField(id,value) { var next=Object.assign({},fields);next[id]=value;fields=next }
  function action(id) { request("plugin_action",{action:id,fields:fields}) }
  function inspect(path) { request("plugin_inspect",{path:String(path)}) }
  function install() { if(review)request("plugin_install",{path:review.path,digest:review.digest}) }
  function apply(path) { request("plugin_apply",{path:String(path),digest:digest}) }
  function inspectArchive(path) { if(!dirty)request("plugin_inspect_archive",{path:String(path)}) }
  function addArchive() { if(archiveReview && !dirty)request("plugin_add_archive",{path:archiveReview.path,sha256:archiveReview.artifact.sha256}) }
  function finish(message,action) {
    if(action.epoch!==epoch)return
    busy=false
    if(!message.ok) {feedback=String((message.error || {}).message || "Plugin operation failed.");return}
    var value=message.value || {}
    feedback=value.feedback || "Ready."
    if(value.plugins!==undefined)installed=value.plugins
    if(value.archive_review!==undefined) {archiveReview=value.archive_review;return}
    if(action.command==="plugin_add_archive")archiveReview=null
    if(action.command==="plugin_inspect") {review=value;feedback="Review this plugin before installing.";return}
    if(action.command==="plugin_install") {review=null;open(String(value.installed.manifest.id),conversation);return}
    if(action.command==="plugin_remove") {selected="";view=null;shared=[];folder="";return}
    if(action.command==="plugin_enable") {view=null;return}
    if(value.view!==undefined) {
      view=value.view;generation=Number(value.generation);digest=String(value.digest);folder=String(value.folder || "");folderUrl=String(value.folder_url || "");shared=value.shared || []
      var values={};(view.fields || []).forEach(function(f){values[f.id]=String(f.value || "")});fields=values
    }
  }
}
