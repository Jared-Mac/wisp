import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ChatMarkup.js" as Markup

ShellRoot {
  id:test
  property bool failed:false
  function check(value,label){if(!value){failed=true;console.error("CHAT_EXTRAS_FAILED: "+label)}}
  function find(item,name){if(!item)return null;if(item.objectName===name)return item;for(var c of item.children || []){var found=find(c,name);if(found)return found}return null}
  Wisp.WispTheme {id:theme;profile:"clean"}
  Wisp.WispBridge {
    id:bridge
    property var sent:[]
    function send(name,args){sent.push({name:name,args:args});return "fake-"+(++requestId)}
  }
  FloatingWindow {
    id:window;visible:true;implicitWidth:640;implicitHeight:640;color:theme.background
    Column {
      id:scene
      anchors.fill:parent;anchors.margins:12;spacing:10
      Components.MessageFeed {id:feed;width:parent.width;height:460;theme:theme;bridge:bridge;conversationId:"local::chat"}
      Components.ChatComposer {id:composer;width:parent.width;theme:theme;bridge:bridge;conversationId:"local::chat"}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),self={id:"self",display_name:"Me",hangout_id:null,media:{},presence:"open"}
    var server={id:"local",name:"Demo",connected:true};data.servers=[server];data.selected_server_id="local";data.self=Object.assign(data.self,self)
    var message={id:"22222222-2222-4222-8222-222222222222",sender:{id:"friend",display_name:"Friend"},conversation_id:"chat",created_at:new Date().toISOString(),content_type:"text/plain",payload:'<b>plain text</b> https://example.com/path?q=1&b=2 :wisp_love: 😀\nhttps://youtu.be/aqz-KE-bpKQ',encryption_version:0}
    var reactions=["self","friend"].map(function(id){return {target_id:message.id,message:{id:id,payload:{target:message.id,emoji:":wisp_love:"},sender:{id:id,display_name:id},content_type:"application/vnd.wisp.reaction+json"}}})
    data.server_states=[{server:server,self:data.self,messages:[message],reactions:reactions,conversations:[{id:"chat",kind:"direct",label:"Friend",members:[self,message.sender]}],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(data)
  }
  Timer {
    running:true;interval:600
    onTriggered:{
      var id="22222222-2222-4222-8222-222222222222",body=test.find(feed,"messageBody-"+id)
      test.check(body && body.text.indexOf('&lt;b&gt;plain text&lt;/b&gt;')>=0,"HTML remains text")
      test.check(body && body.text.indexOf('href="https://example.com/path?q=1&amp;b=2"')>=0,"URL is linked with escaped query")
      test.check(body && body.text.indexOf("assets/emojis/love.svg")>=0,"Wisp emoji renders inline")
      test.check(!Markup.safeLink("javascript:alert(1)") && !Markup.safeLink("file:///tmp/test"),"Only web links can launch")
      test.check(Markup.youtube("https://youtube.com.evil.test/watch?v=aqz-KE-bpKQ").length===0,"YouTube host spoof rejected")
      test.check(Markup.youtube("https://www.youtube.com/watch?v=aqz-KE-bpKQ&t=4 https://youtube.com/shorts/aqz-KE-bpKQ").length===1,"YouTube formats deduplicate")
      var grouped=bridge.chatExtras.groups("local",id);test.check(grouped.length===1 && grouped[0].users.length===2 && grouped[0].own,"reaction counts and own selection")
      var reaction=test.find(feed,"reaction-"+id+"-:wisp_love:");test.check(!!reaction,"reaction chip exists");if(reaction)reaction.clicked()
      var command=bridge.sent[bridge.sent.length-1];test.check(command.name==="toggle_reaction" && command.args.server_id==="local" && command.args.message_id===id,"reaction is scoped to message/server")
      var oldBody=body, oldEmbed=test.find(feed,"playEmbed-aqz-KE-bpKQ")
      var next=JSON.parse(JSON.stringify(bridge.snapshot));next.self.muted=!next.self.muted;bridge.applySnapshot(next);input.wait(60)
      test.check(test.find(feed,"messageBody-"+id)===oldBody && test.find(feed,"playEmbed-aqz-KE-bpKQ")===oldEmbed,"unrelated snapshots preserve message and player delegates")
      test.check(oldEmbed && !oldEmbed.parent.parent.playing,"embed stays unloaded until clicked")
      test.find(composer,"composerEmojiButton").clicked();input.wait(120)
      var choice=test.find(composer.emojiPicker.contentItem,"emojiChoice-wisp_wave");test.check(!!choice,"picker includes themed emoji")
      if(choice)choice.clicked()
      test.check(bridge.draftFor("local::chat").indexOf(":wisp_wave:")>=0,"picker inserts emoji into the draft")
      test.check(!bridge.sent.some(function(c){return c.name==="send_message" || /^(join|watch_video|share|camera)/.test(c.name)}),"previewing emojis does not send or publish")
      var pending=JSON.parse(JSON.stringify(bridge.snapshot));pending.server_states[0].conversations[0].pending_access=true;bridge.applySnapshot(pending);input.wait(40)
      test.check(!test.find(composer,"composerSendButton").enabled && test.find(composer,"trayComposerEditor").readOnly,"pending room chat cannot compose or send")
      var beforePending=bridge.sent.length;bridge.sendComposedMessage("local::chat");bridge.pasteClipboard("local::chat");bridge.importChatFiles("local::chat",["file:///tmp/test"])
      test.check(bridge.sent.length===beforePending,"pending access blocks message and attachment commands")
      bridge.selectConversation("local::chat");bridge.exitConversation("local::chat")
      test.check(bridge.sent.length===beforePending,"opening or closing a room preview does not issue unauthorized chat commands")
      var screenshot=Quickshell.env("WISP_CHAT_EXTRAS_SCREENSHOT")
      if(screenshot)scene.grabToImage(function(image){image.saveToFile(screenshot);console.log(test.failed?"CHAT_EXTRAS_FAILED":"CHAT_EXTRAS_OK");Qt.quit()})
      else {console.log(test.failed?"CHAT_EXTRAS_FAILED":"CHAT_EXTRAS_OK");Qt.quit()}
    }
  }
}
