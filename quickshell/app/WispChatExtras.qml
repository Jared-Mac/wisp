import QtQuick
import "ChatMarkup.js" as Markup

Item {
  id: root
  required property var bridge
  property var catalogs: ({})
  property var images: ({})
  property var loading: ({})
  property var feedback: ({})
  property var pendingReactions: ({})
  property int epoch: 0
  function key(server,id) { return String(server)+"::"+String(id) }
  function request(command,args,action) {
    var id=bridge.send(command,args)
    if(id) bridge.requests[id]=Object.assign({kind:"chatExtras",epoch:epoch},action)
    return id
  }
  function refresh(server, force) {
    if(!server || !bridge.daemonConnected || loading[server] || catalogs[server] && !force) return
    loading=bridge.replaceEntry(loading,server,true)
    feedback=bridge.replaceEntry(feedback,server,"")
    request("list_emojis",{server_id:server},{action:"catalog",serverId:server,items:[]})
  }
  function invalidate() { catalogs=({}); loading=({}); pendingReactions=({}); feedback=({}); epoch++ }
  function library(server,scope) { return (catalogs[server] || []).filter(function(e){return !scope || e.scope===scope}) }
  function upload(server,scope,name,path) {
    feedback=bridge.replaceEntry(feedback,server,"Adding emoji…")
    request("upload_emoji",{server_id:server,scope:scope,name:name,path:path},{action:"mutation",serverId:server})
  }
  function remove(server,id) { request("remove_emoji",{server_id:server,emoji_id:id},{action:"mutation",serverId:server}) }
  function url(server,emoji) {
    var built=/^:wisp_([a-z]+):$/.exec(emoji)
    if(built && Markup.wispNames.indexOf(built[1])>=0) return Qt.resolvedUrl("assets/emojis/"+built[1]+".svg")
    var custom=/^:e_([0-9a-f-]{36}):$/.exec(emoji)
    return custom ? images[key(server,custom[1])] || "" : ""
  }
  function load(server,emoji) {
    var custom=/^:e_([0-9a-f-]{36}):$/.exec(emoji)
    if(!custom) return
    var id=custom[1], k=key(server,id)
    if(images[k] !== undefined || loading[k]) return
    loading=bridge.replaceEntry(loading,k,true)
    request("emoji_image",{server_id:server,emoji_id:id},{action:"image",serverId:server,key:k})
  }
  function loadText(server,text) { Markup.parts(text).forEach(function(p){if(p.emoji) root.load(server,p.emoji)}) }
  function richText(server,text,size,color,conversationId) {
    var changed=images, people=bridge.mentionPeople(conversationId)
    return Markup.richText(text,function(e){return root.url(server,e)},size,color,function(name) {
      return people.some(function(p) { return String(p.display_name).toLowerCase()===name.toLowerCase() })
    })
  }
  function groups(server,target) {
    var state=bridge.participantServer({server_id:server}), grouped={}
    ;(state.reactions || []).forEach(function(r){
      var m=r.message || {}, p=m.payload || {}, emoji=String(p.emoji || "")
      if(String(r.target_id)!==String(target) || String(p.target)!==String(target) || m.content_type!=="application/vnd.wisp.reaction+json" || !emoji) return
      if(!grouped[emoji]) grouped[emoji]={emoji:emoji,users:[],own:false}
      if(!grouped[emoji].users.some(function(u){return u.id===m.sender.id})) grouped[emoji].users.push(m.sender)
      if(m.sender.id===(state.self || {}).id) grouped[emoji].own=true
    })
    return Object.keys(grouped).map(function(e){return grouped[e]})
  }
  function react(server,target,emoji) {
    var message=(bridge.participantServer({server_id:server}).messages || []).filter(function(m){return String(m.id)===String(target)})[0]
    if (!message || message.content_type==="application/vnd.wisp.room-invitation+json") return
    var k=key(server,target)
    if(pendingReactions[k]) return
    var id=request("toggle_reaction",{server_id:server,message_id:target,emoji:emoji},{action:"reaction",key:k})
    if(id) pendingReactions=bridge.replaceEntry(pendingReactions,k,true)
  }
  function reply(message,action) {
    if(action.epoch!==epoch) return
    var value=message.value || {}, server=action.serverId
    if(action.action==="image") {
      loading=bridge.replaceEntry(loading,action.key,undefined)
      images=bridge.replaceEntry(images,action.key,message.ok ? String(value.url || "") : "")
    } else if(action.action==="reaction") pendingReactions=bridge.replaceEntry(pendingReactions,action.key,undefined)
    else if(action.action==="catalog") {
      if(message.ok) {
        var items=action.items.concat(value.emojis || [])
        if(value.next) {request("list_emojis",{server_id:server,after:value.next},{action:"catalog",serverId:server,items:items});return}
        catalogs=bridge.replaceEntry(catalogs,server,items)
      } else feedback=bridge.replaceEntry(feedback,server,String((message.error || {}).message || "Could not load emojis"))
      loading=bridge.replaceEntry(loading,server,undefined)
    } else if(action.action==="mutation") {
      feedback=bridge.replaceEntry(feedback,server,message.ok?"Emoji library updated.":String((message.error || {}).message || "Could not save emoji"))
      if(message.ok) {catalogs=bridge.replaceEntry(catalogs,server,undefined);refresh(server,true)}
    }
  }
}
