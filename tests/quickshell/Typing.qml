import QtQuick
import QtTest
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  function check(ok,label) { if (!ok) { failed=true; console.error("TYPING_FAILED: "+label) } }
  Item {
    id: bridge
    property bool daemonConnected: true
    property var updates: ({preparing:false})
    property var requests: ({})
    property var sent: []
    property var serverStates: [{server:{id:"home",connected:true},self:{id:"self"}}, {server:{id:"other",connected:true},self:{id:"other-self"}}]
    function scopeForConversation(id) { var parts=id.split("::"); return {server_id:parts[0],conversation_id:parts[1]} }
    function scopedConversationId(server,id) { return server+"::"+id }
    function conversationById(id) { return id==="home::private" ? null : {id:id,pending_access:id==="home::pending"} }
    function replaceEntry(map,key,value) { var next=Object.assign({},map);if(value===undefined)delete next[key];else next[key]=value;return next }
    function send(name,args) { sent.push({name:name,args:args});return String(sent.length) }
  }
  Wisp.WispTyping { id:typing;bridge:bridge }
  TestCase { id:input;when:false }
  Timer { running:true;interval:10;onTriggered: {
    typing.edited("home::room","h");typing.edited("home::room","hi")
    test.check(bridge.sent.length===1 && bridge.sent[0].args.active,"burst sends one activity event")
    test.check(bridge.sent[0].args.text===undefined,"draft contents never leave the client")
    typing.outgoing=bridge.replaceEntry(typing.outgoing,"home::room",{lastEdit:Date.now(),lastSent:Date.now()-4000})
    typing.edited("home::room","hi!")
    test.check(bridge.sent.length===2,"continued edits renew the lease")
    typing.edited("home::room","")
    test.check(bridge.sent.length===3 && !bridge.sent[2].args.active,"clearing draft stops typing")
    typing.edited("home::pending","draft")
    test.check(bridge.sent.length===3,"pending chat access never emits typing")
    typing.edited("home::room","forgotten letter")
    typing.outgoing=bridge.replaceEntry(typing.outgoing,"home::room",{lastEdit:Date.now()-8100,lastSent:Date.now()-8100})
    input.wait(550)
    test.check(!typing.outgoing["home::room"] && !bridge.sent[bridge.sent.length-1].args.active,"abandoned draft times out without clearing its text")
    typing.receive({server_id:"home",conversation_id:"room",user_id:"friend",display_name:"Mira",active:true,timeout_ms:8000})
    test.check(typing.label("home::room")==="Mira is typing…","incoming typing is displayed")
    test.check(typing.label("other::room")==="","same room ID on another server stays independent")
    typing.receive({server_id:"home",conversation_id:"room",user_id:"self",display_name:"Me",active:true})
    test.check(typing.label("home::room")==="Mira is typing…","own typing is hidden")
    typing.receive({server_id:"home",conversation_id:"private",user_id:"friend",display_name:"Mira",active:true})
    test.check(typing.label("home::private")==="","unknown/private conversations are ignored")
    bridge.serverStates=[{server:{id:"home",connected:false},self:{id:"self"}}]
    typing.reconcile()
    test.check(typing.label("home::room")==="","losing server connection clears incoming typing")
    typing.reset()
    console.log(test.failed ? "TYPING_FAILED" : "TYPING_OK");Qt.quit()
  } }
}
