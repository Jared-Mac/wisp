import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ChatMarkup.js" as Markup
import "app/MentionLogic.js" as Mentions

ShellRoot {
  id: test
  property bool failed: false
  property int serial: 0
  function check(ok,label) { if (!ok) { failed=true; console.error("MENTIONS_FAILED: "+label) } }
  function find(item,name) {
    if (!item) return null
    if (item.objectName===name) return item
    for (var child of item.children || []) { var found=find(child,name); if(found)return found }
    return null
  }
  function incoming(text,options) {
    options=options || {}
    var s=JSON.parse(JSON.stringify(bridge.snapshot)),state=s.server_states[options.remote ? 1 : 0]
    var m={id:options.id || "arrival-"+(++serial),conversation_id:"chat",sender:options.own ? state.self : {id:"friend",display_name:"Mira"},
      created_at:new Date(Date.now()+serial*1000).toISOString(),content_type:options.type || "text/plain",payload:text,context:options.context}
    state.messages.push(m); state.conversations[0].last_message=m
    state.conversations[0].unread_count=(state.conversations[0].unread_count || 0)+1
    bridge.applySnapshot(s,options.event || "message_created")
  }
  Wisp.WispTheme { id:theme; profile:"clean" }
  Wisp.WispBridge {
    id:bridge
    property var sent:[]
    property var sounds:[]
    function send(name,args) { sent.push({name:name,args:args}); return "fixture-"+(++requestId) }
    function playNotificationSound(kind) { sounds.push(kind || "message") }
  }
  FloatingWindow {
    id:window;visible:true;implicitWidth:640;implicitHeight:620;color:theme.background
    Column {
      id:scene;anchors.fill:parent;anchors.margins:12;spacing:12
      Components.MessageFeed { id:feed;width:parent.width;height:400;bridge:bridge;theme:theme;conversationId:composer.conversationId }
      Components.ChatComposer { id:composer;width:parent.width;bridge:bridge;theme:theme;conversationId:"local::chat" }
    }
  }
  TestCase { id:input;parent:window.contentItem;when:false }
  Component.onCompleted: {
    var s=JSON.parse(JSON.stringify(bridge.snapshot)),server={id:"local",name:"Lantern",connected:true}
    var me={id:"self",display_name:"Rowan",media:{},presence:"open"}, friend={id:"friend",display_name:"Mira"}, spaced={id:"spaced",display_name:"Mira Moon"}
    s.self=Object.assign(s.self,me);s.servers=[server,{id:"remote",name:"Other",connected:true}];s.selected_server_id="local"
    s.server_states=s.servers.map(function(server,index) {
      var self=index ? {id:"other-self",display_name:"Other Rowan",media:{}} : s.self
      return {server:server,self:self,conversations:[{id:"chat",kind:"circle",label:"Friends",members:index ? [self,friend] : [me,friend,spaced],unread_count:0}],
        messages:[],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}
    })
    bridge.applySnapshot(s)
  }
  Timer {
    running:true;interval:500
    onTriggered: {
      test.check(Mentions.token("Mira")==="@Mira" && Mentions.token("Mira Moon")==='@"Mira Moon"',"Readable tokens support names with spaces")
      var odd='Mira "Moon" \\ Star <b>'
      test.check(Markup.mentions(Mentions.token(odd),odd),"Quoted names round-trip quotes, slashes and markup safely")
      test.check(Markup.mentions("Hi @rowan!","Rowan") && !Markup.mentions("@RowanTwo","Rowan"),"Matching is exact and case-insensitive")
      test.check(!Markup.mentions("mail@Rowan https://example.com/@Rowan www.example.com/@Rowan","Rowan"),"Email addresses and web links are not mentions")
      test.check(!Mentions.query("mail@Row",8) && !Mentions.query("https://example.com/@Row",23),"Autocomplete ignores emails and URLs")
      test.check(Mentions.query("@Mira hello",3).end===5 && Mentions.query('@"Mira Moon" hello',5).end===12,"Completing inside a mention replaces the remaining name")
      test.check(Markup.mentions('@"雪 月"','雪 月'),"Unicode display names are supported")
      var editor=test.find(composer,"trayComposerEditor")
      editor.forceActiveFocus();editor.text="Hi @Mi";editor.cursorPosition=editor.text.length;input.wait(100)
      test.check(composer.mentionPicker.visible && composer.mentionChoices.length===2,"Typing @ filters current chat members")
      input.keyClick(Qt.Key_Down);input.keyClick(Qt.Key_Return);input.wait(80)
      test.check(editor.text==='Hi @"Mira Moon" ',"Arrow keys and Enter insert the selected mention")
      test.check(!bridge.sent.some(function(c){return c.name==="send_message"}),"Choosing a mention does not send the draft")
      editor.text="@Mi";editor.cursorPosition=3;input.wait(80)
      input.keyClick(Qt.Key_Escape);input.wait(50)
      test.check(!composer.mentionPicker.visible && editor.text==="@Mi","Escape dismisses autocomplete without losing text")
      editor.text="@Mir";editor.cursorPosition=4;input.wait(80)
      var choice=test.find(composer.mentionPicker.contentItem,"mentionChoice-friend")
      test.check(!!choice,"Mouse selection is available")
      if(choice)input.mouseClick(choice,choice.width/2,choice.height/2)
      input.wait(60);test.check(editor.text==="@Mira ","Mouse selection inserts a readable mention")
      editor.text="Hello @Ro";editor.cursorPosition=editor.text.length;input.wait(60)
      input.keyClick(Qt.Key_Tab);input.wait(60)
      test.check(editor.text==="Hello @Rowan ","Tab accepts a mention")
      input.keyClick(Qt.Key_Return);input.wait(60)
      var sent=bridge.sent.filter(function(c){return c.name==="send_message"})
      test.check(sent.length===1 && sent[0].args.text==="Hello @Rowan" && sent[0].args.server_id==="local","Mentions use the existing scoped encrypted text send path")
      bridge.sendingConversations=({})
      composer.conversationId="remote::chat";input.wait(60)
      editor.text="@";editor.cursorPosition=1;editor.forceActiveFocus();input.wait(60)
      test.check(composer.mentionChoices.some(function(p){return p.id==="other-self"}) && !composer.mentionChoices.some(function(p){return p.id==="spaced"}),"Autocomplete stays scoped when switching servers")
      composer.conversationId="local::chat";input.wait(60)
      bridge.notificationSoundsEnabled=true;bridge.notificationMentionsOnly=true;bridge.notificationPolicy="always"
      bridge.notificationMuted=false;bridge.notificationVolume=50;bridge.sounds=[]
      test.incoming("Ordinary chat");test.check(bridge.sounds.length===0,"Mentions-only suppresses ordinary chat sounds")
      test.incoming("Hello @Rowan!");test.check(bridge.sounds.length===1,"A direct mention plays a sound")
      var count=bridge.sounds.length
      test.incoming("mail@Rowan https://example.com/@Rowan")
      test.incoming("@Rowan",{own:true})
      test.incoming("@Rowan",{context:{forwarded_from:{sender_name:"Mira"}}})
      test.incoming("@Rowan",{event:"server_reconnected"})
      test.check(bridge.sounds.length===count,"Emails, links, own sends, forwarded quotes and reconnects stay silent")
      bridge.toggleChatNotifications("local::chat");test.incoming("@Rowan");test.check(bridge.sounds.length===count,"Muted chats stay silent even for mentions")
      bridge.toggleChatNotifications("local::chat")
      test.incoming("@Rowan",{remote:true});test.check(bridge.sounds.length===count,"Other servers use their own self identity")
      test.incoming('@"Other Rowan"',{remote:true,id:"same-id"});test.check(bridge.sounds.length===++count,"Mentions resolve the receiving server's display name")
      test.incoming("@Rowan",{id:"same-id"});test.check(bridge.sounds.length===++count,"Duplicate message IDs across servers do not hide mentions")
      test.incoming({caption:"Look @Rowan",file_name:"test.txt"},{type:"application/octet-stream"});test.check(bridge.sounds.length===++count,"Attachment captions can mention people")
      bridge.notificationMuted=true;test.incoming("@Rowan");test.check(bridge.sounds.length===count,"Global mute wins")
      bridge.notificationMuted=false;bridge.notificationMentionsOnly=false;test.incoming("Ordinary again");test.check(bridge.sounds.length===++count,"Disabling the filter restores normal notifications")
      var rich=bridge.chatExtras.richText("local",'Hello @Mira and @"Mira Moon" <script>',20,"#aabbcc","local::chat")
      test.check(rich.indexOf("<b>@Mira</b>")>=0 && rich.indexOf("<b>@Mira Moon</b>")>=0 && rich.indexOf("&lt;script&gt;")>=0,"Known mentions highlight while HTML remains escaped")
      test.check(bridge.chatExtras.richText("local","@Unknown",20,"#aabbcc","local::chat").indexOf("<b>")<0,"Unknown names stay plain text")
      test.check(!bridge.sent.some(function(c){return /^(join|share|camera|watch_video)/.test(c.name)}),"Mention interactions never join or publish media")
      editor.text="@Mi";editor.cursorPosition=3;editor.forceActiveFocus();input.wait(100)
      test.check(composer.mentionPicker.visible,"Editing after Escape lets the same query reopen")
      var screenshot=Quickshell.env("WISP_MENTIONS_SCREENSHOT")
      if (screenshot) scene.grabToImage(function(image) { image.saveToFile(screenshot);composer.mentionPicker.contentItem.grabToImage(function(picker) { picker.saveToFile(screenshot+"-picker.png");console.log(test.failed ? "MENTIONS_FAILED" : "MENTIONS_OK");Qt.quit() }) })
      else { console.log(test.failed ? "MENTIONS_FAILED" : "MENTIONS_OK");Qt.quit() }
    }
  }
}
