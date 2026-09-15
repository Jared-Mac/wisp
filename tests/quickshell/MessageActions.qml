import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/views" as Views

ShellRoot {
  id:test
  property bool failed:false
  property string originalId:"22222222-2222-4222-8222-222222222222"
  function check(ok,label){if(!ok){failed=true;console.error("MESSAGE_ACTIONS_FAILED: "+label)}}
  function find(item,name,seen){
    if(!item)return null;seen=seen || [];if(seen.indexOf(item)>=0)return null;seen.push(item)
    if(item.objectName===name)return item
    if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}
    if(item.footer){var r=find(item.footer,name,seen);if(r)return r}
    if(item.item){var r=find(item.item,name,seen);if(r)return r}
    var children=item.data || item.contentData || item.children || []; for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null
  }
  function last(){return bridge.sent[bridge.sent.length-1]}
  function ack(ok,value){bridge.finishRequest({id:"fake-"+bridge.requestId,ok:ok,value:value || {},error:ok ? null : {message:"Test send failure"}})}
  Wisp.WispAppearance {id:appearance;environment:"desktop";Component.onCompleted:setChatLayout(Quickshell.env("WISP_TEST_CHAT_LAYOUT") || "grouped")}
  Wisp.WispTheme {id:theme;appearanceController:appearance;profile:Quickshell.env("WISP_TEST_THEME") || "clean"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){sent.push({name:name,args:args});return "fake-"+(++requestId)}}
  FloatingWindow {
    id:window;visible:true;implicitWidth:620;implicitHeight:760;color:theme.background
    Column {
      id:scene;anchors.fill:parent;anchors.margins:12;spacing:8
      Views.ConversationWorkspace {id:workspace;width:parent.width;height:470;theme:theme;bridge:bridge;selectedId:"local::chat"}
      Views.MessagesView {id:tray;width:parent.width;availableHeight:250;theme:theme;bridge:bridge}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),self={id:"self",display_name:"Me",hangout_id:null,media:{},presence:"open",server_admin:false}
    var server={id:"local",name:"Demo",connected:true};data.servers=[server,{id:"other",name:"Other server",connected:true}];data.selected_server_id="local";data.self=Object.assign(data.self,self)
    var friend={id:"friend",display_name:"Friend"}
    var original={id:originalId,sender:friend,conversation_id:"chat",created_at:"2026-09-10T10:00:00Z",content_type:"text/plain",payload:"An original message",encryption_version:1}
    data.server_states=[{server:server,self:data.self,messages:[original,{id:"reply",sender:self,conversation_id:"chat",created_at:"2026-09-10T10:01:00Z",content_type:"text/plain",payload:"A reply",encryption_version:1,context:{reply_to:{message_id:originalId,sender_name:"Friend",preview:"An original message"}}}],conversations:[{id:"chat",kind:"direct",label:"Friend",members:[self,friend]},{id:"room",kind:"hangout",spot_id:"room",label:"Lounge",members:[self,friend]}],friends:[friend],hangouts:[],spots:[{id:"room",name:"Lounge"}],knocks:[],devices:[],room_invitations:[]},
      {server:data.servers[1],self:self,messages:[],conversations:[{id:"dest",kind:"direct",label:"Second chat",members:[self,{id:"friend2"}]}],friends:[{id:"new",display_name:"New friend"}],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(data);bridge.selectConversation("local::chat")
  }
  Timer {
    id:capture;property var target;interval:300
    onTriggered:{if(!target.grabToImage(function(im){im.saveToFile(Quickshell.env("WISP_MESSAGE_ACTIONS_SCREENSHOT"));console.log("MESSAGE_ACTIONS_OK");Qt.quit()})){console.error("MESSAGE_ACTIONS_FAILED: capture");Qt.quit()}}
  }
  Timer {
    running:true;interval:600
    onTriggered:{
      var feed=test.find(workspace,"messageFeed"), menu=test.find(feed,"messageMenu-"+test.originalId), editor=test.find(workspace,"mainComposerEditor")
      test.check(!!menu && !!editor,"main chat loaded");if(!menu || !editor){Qt.quit();return}
      test.find(feed,"messageOptions-"+test.originalId).clicked();input.wait(30)
      test.check(test.last().name==="list_pins","message menu loads pin state")
      test.ack(true,{messages:[],can_manage:true});input.wait(20)
      test.check(test.find(menu,"pinMessage-"+test.originalId).enabled,"DM participant can pin")
      test.find(menu,"replyMessage-"+test.originalId).triggered();menu.close();input.wait(30)
      test.check(test.find(workspace,"composerReplyBar").visible && editor.activeFocus,"Reply from triple-dot menu displays bar and focuses editor")
      bridge.setDraft("local::chat","My reply");bridge.sendComposedMessage("local::chat")
      test.check(test.last().name==="send_reply" && test.last().args.reply_to===test.originalId && test.last().args.server_id==="local","reply command uses original and server scope")
      test.ack(false);test.check(bridge.draftFor("local::chat")==="My reply" && !!bridge.messageActions.replyFor("local::chat"),"failed send preserves draft and reply")
      bridge.sendComposedMessage("local::chat");test.ack(true)
      test.check(bridge.draftFor("local::chat")==="" && !bridge.messageActions.replyFor("local::chat"),"acknowledged send clears draft and reply")
      bridge.messageActions.beginReply("local::chat",bridge.messagesFor("local::chat")[0]);editor.forceActiveFocus();input.keyClick(Qt.Key_Escape);input.wait(20)
      test.check(!bridge.messageActions.replyFor("local::chat"),"Escape cancels reply")
      test.check(!bridge.messageActions.canPin("local::room"),"non-admin cannot pin server room")
      var changed=JSON.parse(JSON.stringify(bridge.snapshot));changed.server_states[0].self.server_admin=true;bridge.applySnapshot(changed)
      test.check(bridge.messageActions.canPin("local::room"),"server admin can pin room")
      var pinsButton=test.find(workspace,"chatPinsButton"), trayPins=test.find(tray,"chatPinsButton")
      test.check(pinsButton && pinsButton.visible && trayPins && trayPins.visible,"main and tray headers include pins")
      pinsButton.clicked();input.wait(30)
      var old={id:"old",sender:{id:"friend",display_name:"Friend"},conversation_id:"chat",created_at:"2020-01-01T00:00:00Z",content_type:"text/plain",payload:"Pinned before loaded history",encryption_version:1}
      test.ack(true,{messages:[old],can_manage:true});input.wait(30)
      var popup=test.find(pinsButton,"chatPinsPopup");if(Quickshell.env("WISP_CAPTURE_STAGE")==="pins"){capture.target=popup.contentItem.parent;capture.start();return}
      var jump=test.find(popup,"jumpToPin-old")
      test.check(!!jump,"pins panel renders old message");if(jump)jump.clicked()
      test.check(test.last().name==="load_message" && test.last().args.message_id==="old","pin jump rechecks original visibility")
      test.ack(true,old);input.wait(40)
      test.check(!!test.find(feed,"messageBody-old") && feed.highlightedId==="old","old pin is inserted and highlighted")
      feed.scrollToLatest();input.wait(40)
      test.find(menu,"forwardMessage-"+test.originalId).triggered();input.wait(40)
      var dialog=test.find(feed,"forwardMessageDialog"),choices=test.find(dialog,"forwardDestinations"),search=test.find(dialog,"forwardDestinationSearch")
      test.check(dialog && dialog.opened && choices.count===4,"forward picker includes chats and friend without DM")
      if(Quickshell.env("WISP_CAPTURE_STAGE")==="forward"){capture.target=dialog.contentItem.parent;capture.start();return}
      var before=bridge.sent.length;search.text="Second chat";input.wait(40)
      test.check(choices.count===1,"forward search filters destinations")
      var target=test.find(dialog,"forwardDestination-other::dest");test.check(!!target,"other server destination exists");if(target)target.clicked()
      test.check(bridge.sent.length===before,"choosing destination does not send")
      test.find(dialog,"confirmForward").clicked()
      test.check(test.last().name==="forward_message" && test.last().args.server_id==="other" && test.last().args.source_server_id==="local" && test.last().args.conversation_id==="dest","forward command keeps source and destination server scopes")
      test.ack(false);test.check(dialog.opened && !dialog.busy && !!dialog.error,"failed forward keeps picker for retry")
      test.find(dialog,"confirmForward").clicked();test.ack(true);input.wait(40);test.check(!dialog.opened,"successful forward closes picker")
      test.check(!bridge.sent.some(function(c){return /^(join|watch_video|share|camera)/.test(c.name)}),"chat actions never start media")
      var screenshot=Quickshell.env("WISP_MESSAGE_ACTIONS_SCREENSHOT")
      bridge.messageActions.beginReply("local::chat",bridge.messagesFor("local::chat")[1]);input.wait(50)
      if(screenshot)scene.grabToImage(function(image){image.saveToFile(screenshot);console.log(test.failed?"MESSAGE_ACTIONS_FAILED":"MESSAGE_ACTIONS_OK");Qt.quit()})
      else {console.log(test.failed?"MESSAGE_ACTIONS_FAILED":"MESSAGE_ACTIONS_OK");Qt.quit()}
    }
  }
}
