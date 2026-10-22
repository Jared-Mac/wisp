import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  function check(ok,label) { if (!ok) { failed=true; console.error("CHAT_RELIABILITY_FAILED: "+label) } }
  function editor(item) {
    if (item.objectName === "trayComposerEditor") return item
    for (var child of item.children || []) { var found=editor(child); if(found)return found }
    return null
  }
  Wisp.WispTheme { id: theme; profile: "clean" }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    property int privacyRefreshes: 0
    function refreshPrivacy() { privacyRefreshes++ }
    function send(name,args) { var id="fixture-"+(++requestId); sent.push({id:id,name:name,args:args}); return id }
  }
  FloatingWindow {
    id: window; visible:true; implicitWidth:800; implicitHeight:600
    Column {
      anchors.fill:parent
      Components.ChatComposer { id:first; width:parent.width; bridge:bridge; theme:theme; conversationId:"chat" }
      Components.ChatComposer { id:second; width:parent.width; bridge:bridge; theme:theme; conversationId:"chat" }
      Components.MessageFeed { id:feed; width:parent.width; height:250; bridge:bridge; theme:theme; conversationId:"chat" }
    }
  }
  TestCase { id: input; parent:window.contentItem; when:false }
  Timer { running:true; interval:300; onTriggered: {
    var state=JSON.parse(JSON.stringify(bridge.snapshot))
    state.seq=10
    state.conversations=[{id:"chat",kind:"direct",label:"Fixture",members:[],unread_count:0}]
    state.messages=[]
    bridge.applySnapshot(state)
    var a=test.editor(first),b=test.editor(second)
    first.focusEditor(); input.keyClick(Qt.Key_V,Qt.ControlModifier)
    var paste=bridge.sent.filter(function(c){return c.name==="paste_clipboard"}).slice(-1)[0]
    test.check(!!paste,"paste shortcut requests clipboard once")
    bridge.finishRequest({id:paste.id,ok:true,value:{text:"hello"}})
    input.wait(30)
    test.check(bridge.draftFor("chat")==="hello" && a.text==="hello" && b.text==="hello","two visible composers insert clipboard text only once")
    // Replay cannot insert twice, and selection replacement stays native-like.
    bridge.clipboardTextReady("chat","hello",paste.id)
    test.check(a.text==="hello","duplicate clipboard response is ignored")
    a.select(0,5); first.pasteClipboard()
    bridge.finishRequest({id:first.pasteRequestId,ok:true,value:{text:"replacement"}})
    test.check(a.text==="replacement" && b.text==="replacement","paste replaces the initiating editor selection once")
    first.pasteClipboard(); var abandoned=first.pasteRequestId
    first.conversationId="elsewhere"
    bridge.finishRequest({id:abandoned,ok:true,value:{text:"late"}})
    test.check(bridge.draftFor("chat")==="replacement" && bridge.draftFor("elsewhere")==="","late paste does not move to another composer or conversation")
    // A delayed old snapshot may arrive after the post-send snapshot.
    var fresh=JSON.parse(JSON.stringify(state)); fresh.seq=12
    fresh.messages=[{id:"new-message",conversation_id:"chat",sender:{id:"friend",display_name:"Friend"},created_at:"2026-10-21T12:00:00Z",content_type:"text/plain",payload:"New message"}]
    bridge.applySnapshot(fresh,"message_created")
    var stale=JSON.parse(JSON.stringify(state)); stale.seq=11
    bridge.applySnapshot(stale,"audio_telemetry_changed")
    test.check(bridge.snapshot.seq===12 && feed.incomingMessages.length===1,"old snapshot cannot remove a displayed message")
    var deletion=JSON.parse(JSON.stringify(state)); deletion.seq=13
    bridge.applySnapshot(deletion,"message_deleted")
    test.check(feed.incomingMessages.length===0,"newer legitimate deletion still applies")
    bridge.receivedSnapshot=false; state.seq=1; bridge.applySnapshot(state)
    test.check(bridge.snapshot.seq===1,"reconnecting to a restarted daemon resets the sequence baseline")
    var scoped=JSON.parse(JSON.stringify(state))
    scoped.server_states=[{server:{id:"local",name:"Fixture",connected:true},self:scoped.self,
      conversations:scoped.conversations,messages:[],friends:[],hangouts:[],knocks:[],devices:[],spots:[],room_invitations:[]}]
    for(var m=0;m<300;m++) scoped.server_states[0].messages.push({id:"message-"+m,conversation_id:"chat",
      sender:{id:"friend",display_name:"Friend"},created_at:"2026-10-21T12:00:00Z",content_type:"text/plain",payload:"Fixture message "+m})
    scoped.seq=2; bridge.applySnapshot(scoped)
    input.wait(10)
    var privacyBefore=bridge.privacyRefreshes
    var stableServers=bridge.serverStates, stableMessages=bridge.messages
    for(var tick=0;tick<100;tick++) {
      var telemetry=JSON.parse(JSON.stringify(scoped));telemetry.seq=3+tick
      telemetry.self.media.audio.input_level=tick
      bridge.applySnapshot(telemetry,"audio_telemetry_changed")
    }
    test.check(bridge.serverStates===stableServers && bridge.messages===stableMessages,"audio telemetry does not rebuild server or message models")
    test.check(bridge.audioState.input_level===99,"live audio telemetry still updates")
    input.wait(10)
    test.check(bridge.privacyRefreshes===privacyBefore,"audio telemetry does not flood the daemon with privacy refresh requests")
    var changed=JSON.parse(JSON.stringify(bridge.snapshot));changed.seq++
    changed.server_states[0].messages[0].payload="Edited fixture"
    bridge.applySnapshot(changed,"audio_telemetry_changed")
    test.check(bridge.messages!==stableMessages && bridge.messages[0].payload==="Edited fixture","chat changes carried with telemetry still refresh the stable model")
    // Pending sends appear immediately and reconcile by canonical message ID.
    var chat=bridge.conversations[0].id
    bridge.setDraft(chat,"Immediate message")
    bridge.sendComposedMessage(chat)
    var send=bridge.sent.slice(-1)[0]
    var pending=bridge.messagesFor(chat).slice(-1)[0]
    test.check(pending.payload==="Immediate message" && pending.delivery_status==="sending","message appears before network acknowledgement")
    test.check(bridge.draftFor(chat)==="Immediate message","pending draft remains recoverable until acknowledgement")
    var delivered=JSON.parse(JSON.stringify(bridge.snapshot));delivered.seq++
    delivered.server_states[0].messages.push({id:"delivered-id",conversation_id:"chat",sender:delivered.server_states[0].self,
      created_at:pending.created_at,content_type:"text/plain",payload:"Immediate message"})
    bridge.applySnapshot(delivered,"message_created")
    test.check(bridge.messagesFor(chat).filter(function(m){return m.payload==="Immediate message"}).length===1,"snapshot before acknowledgement does not duplicate pending message")
    bridge.finishRequest({id:send.id,ok:true,value:{message_id:"delivered-id"}})
    test.check(bridge.messagesFor(chat).filter(function(m){return m.payload==="Immediate message"}).length===1 && !bridge.messagesFor(chat).slice(-1)[0].delivery_status,"exact acknowledgement reconciles to authoritative message")
    bridge.setDraft(chat,"Try again")
    bridge.sendComposedMessage(chat);send=bridge.sent.slice(-1)[0]
    bridge.finishRequest({id:send.id,ok:false,error:{message:"Fixture offline"}})
    test.check(bridge.messagesFor(chat).slice(-1)[0].delivery_status==="failed","send failure preserves visible message")
    var sentCount=bridge.sent.length
    bridge.recoverOutgoing(chat)
    test.check(bridge.draftFor(chat)==="Try again" && bridge.sent.length===sentCount,"retry restores for review without blind duplicate delivery")
    bridge.sendComposedMessage(chat);send=bridge.sent.slice(-1)[0]
    bridge.finishRequest({id:send.id,ok:true,value:{message_id:"later-id"}})
    test.check(bridge.messagesFor(chat).slice(-1)[0].delivery_status==="sent","ack before snapshot retains the confirmed message")
    bridge.setDraft(chat,"Disconnected")
    bridge.sendComposedMessage(chat);bridge.failOutgoing()
    test.check(bridge.messagesFor(chat).slice(-1)[0].delivery_status==="failed","disconnect preserves unconfirmed send")
    console.log(test.failed ? "CHAT_RELIABILITY_FAILED" : "CHAT_RELIABILITY_OK"); Qt.quit()
  } }
}
