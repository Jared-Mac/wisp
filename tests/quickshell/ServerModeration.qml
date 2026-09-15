import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,message) {if(!ok){failed=true;console.error("SERVER_MODERATION_FAILED: "+message)}}
  function find(item,name) {
    if(!item)return null
    if(item.objectName===name)return item
    for(var child of item.children || []){var match=find(child,name);if(match)return match}
    return null
  }
  function last(){return bridge.sent[bridge.sent.length-1]}
  Wisp.WispTheme {
    id:testTheme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"
    appearanceController:QtObject {property bool managed:false;property string palette:testTheme.profile;property var colorOptions:({senderNames:true,chatBorders:false,chatHeadings:false,roomSections:false,friendSections:false})}
  }
  Wisp.WispBridge {
    id:bridge;property var sent:[]
    function send(name,args){var id="test-"+(++requestId);sent.push({id:id,name:name,args:args});return id}
  }
  FloatingWindow {
    id:window;visible:true;width:380;height:900;color:testTheme.surface
    Rectangle {id:captureRoot;anchors.fill:parent;color:testTheme.surface
    ScrollView {anchors.fill:parent;contentWidth:availableWidth
      Views.ServerSettingsView {id:page;bridge:bridge;theme:testTheme;width:parent.width}
    }
    }
  }
  TestCase {id:input;name:"ServerModerationInput";when:false}
  function settings(role) {
    return {name:"Test server",role:role,member_moderation:true,categories:[],channels:[],rooms:[],bans:[],members:[
      {id:"owner",display_name:"Owner",role:"owner"},
      {id:"admin",display_name:"Administrator",role:"admin"},
      {id:"offline",display_name:"Offline member with a long display name",role:"member"}
    ]}
  }
  Component.onCompleted: {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id="owner";data.self.server_owner=true
    data.servers=[{id:"primary",name:"Test server",connected:true}];data.selected_server_id="primary"
    data.server_states=[Object.assign({},data,{server:data.servers[0]})]
    bridge.applySnapshot(data);bridge.serverSettings=settings("owner")
  }
  Timer {
    interval:350;running:true
    onTriggered:{
      test.find(page,"serverRolesSection").expanded=true;input.wait(60)
      var kick=test.find(page,"kickServerMember-offline"),ban=test.find(page,"banServerMember-offline")
      test.check(kick && kick.visible && ban && ban.visible,"offline member has moderation controls")
      test.check(!test.find(page,"kickServerMember-owner").visible,"owner cannot be removed")
      var before=bridge.sent.length
      kick.clicked();input.wait(40)
      test.check(bridge.sent.length===before,"clicking Kick only asks for confirmation")
      test.check(page.pendingModeration.serverId==="primary","confirmation pins the server")
      page.moderatePending();input.wait(30)
      test.check(test.last().name==="moderate_server_member" && test.last().args.action==="kick" && test.last().args.user_id==="offline" && test.last().args.server_id==="primary","confirmed kick uses stable member and server IDs")
      bridge.finishRequest({id:test.last().id,ok:true,value:{ok:true,media_pending:true}})
      test.check(bridge.serverSettingsFeedback.indexOf("retry")>=0,"pending media removal is clearly reported")
      ban.clicked();input.wait(30)
      test.check(bridge.sent.length===before+1,"Ban also needs confirmation")
      page.moderatePending();input.wait(30)
      test.check(test.last().args.action==="ban","confirmed ban is distinct from kick")
      bridge.finishRequest({id:test.last().id,ok:true,value:{ok:true}})
      var current=settings("admin");bridge.serverSettings=current;input.wait(30)
      test.check(!test.find(page,"kickServerMember-admin").visible,"administrators cannot remove peers")
      current=settings("owner");current.bans=[{id:"offline",display_name:"Offline member",reason:"Test reason"}];bridge.serverSettings=current
      test.find(page,"serverBansSection").expanded=true;input.wait(30)
      test.find(page,"unbanServerMember-offline").clicked();input.wait(30);page.moderatePending()
      test.check(test.last().args.action==="unban","bans can be lifted explicitly")
      bridge.finishRequest({id:test.last().id,ok:true,value:{ok:true}})
      page.confirmModeration("ban",{id:"offline",display_name:"Offline member"})
      page.pendingModeration=Object.assign({},page.pendingModeration,{serverId:"another-server"})
      before=bridge.sent.length;page.moderatePending()
      test.check(bridge.sent.length===before,"switching server cannot redirect a pending moderation")
      current=settings("owner");delete current.member_moderation;bridge.serverSettings=current;input.wait(30)
      test.check(!test.find(page,"kickServerMember-offline").visible && !test.find(page,"serverBansSection").visible,"older servers hide unsupported moderation")
      bridge.serverSettings=settings("owner");input.wait(50)
      var path=Quickshell.env("WISP_MODERATION_SCREENSHOT")
      if(!test.failed)console.log("SERVER_MODERATION_OK")
      if(path){captureRoot.grabToImage(function(result){result.saveToFile(path);Qt.quit()})}
      else Qt.quit()
    }
  }
}
