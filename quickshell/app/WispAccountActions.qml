import QtQuick
import Quickshell.Io

Item {
  id: root
  required property var bridge
  property var states: ({})
  function state(serverId) { return states[String(serverId)] || {busy:false,error:"",feedback:"",invitation:null,person:null,overview:{},invites:[]} }
  function put(serverId,patch) { states=bridge.replaceEntry(states,String(serverId),Object.assign({},state(serverId),patch)) }
  function act(command,args,serverId) {
    serverId=String(serverId || bridge.activeServer.id)
    if (state(serverId).busy) return false
    var id=bridge.send(command,Object.assign({},args || {},{server_id:serverId}))
    if (!id) {put(serverId,{error:"Reconnect to try again."});return false}
    bridge.requests[id]={kind:"membership",command:command,serverId:serverId,args:args || {}}
    var patch={busy:true,error:"",feedback:""}
    if(command==="lookup_person") patch.person=null
    if(command==="create_server_invite") patch.invitation=null
    put(serverId,patch);return true
  }
  function finish(message,request) {
    var value=message.value || {}, patch={busy:false}
    if (!message.ok) patch.error=String((message.error || {}).message || "Couldn't complete this action.")
    else {
      switch(request.command) {
        case "create_server_invite":patch.invitation=value;break
        case "revoke_server_invite":patch.invitation=state(request.serverId).invitation && state(request.serverId).invitation.id===request.args.invite_id ? null : state(request.serverId).invitation;patch.invites=state(request.serverId).invites.filter(function(i){return i.id!==request.args.invite_id});patch.feedback="Invitation revoked";break
        case "list_server_invites":patch.invites=value.invites || [];break
        case "lookup_person":patch.person=value.person;patch.feedback=value.person ? "" : "No account found with that username.";break
        case "account_overview":patch.overview=value;break
        case "set_public_handle":patch.overview=Object.assign({},state(request.serverId).overview,{handle:value.handle});patch.feedback="Public username saved";break
        case "block_person":patch.feedback="Account blocked";break
        case "unblock_person":patch.feedback="Account unblocked";break
        case "leave_server":patch.feedback="Left server";break
      }
    }
    put(request.serverId,patch)
    if(message.ok && ["block_person","unblock_person"].indexOf(request.command)>=0) act("account_overview",{},request.serverId)
  }
  property string incomingInvitation: ""
  function openInvitation(link) {
    if (!/^https:\/\/wisp\.you\/[a-z]+(?:-[a-z]+){0,3}[0-9]{12}$/.test(String(link))
        && !/^https:\/\/[^/?#]+\/join\/#v2\.[A-Za-z0-9_-]{43}$/.test(String(link))) return false
    joinServer(String(link)); return true
  }
  function joinServer(invitation) {
    if (onboarding.running) return
    incomingInvitation=String(invitation || "")
    onboarding.running=true
  }
  Process {id:onboarding;command:["env","WISP_ONBOARDING_JOIN=1","WISP_ONBOARDING_INVITE="+root.incomingInvitation,"wisp-onboarding"]}

  Connections {
    target:root.bridge
    function onDaemonConnectedChanged() {if(!root.bridge.daemonConnected)root.states=({})}
  }
}
