import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
ShellRoot {
  id:test;property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("PLUGINS_FAILED: "+label)}}
  function find(item,name,seen){if(!item || seen.indexOf(item)>=0)return null;seen.push(item);if(item.objectName===name)return item;if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}for(var child of item.data || item.children || []){var r=find(child,name,seen);if(r)return r}return null}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="fixture-"+sent.length;sent.push({name:name,args:args,id:id});return id}}
  FloatingWindow {id:window;visible:true;implicitWidth:720;implicitHeight:980;color:theme.background
    ScrollView {anchors.fill:parent;contentWidth:availableWidth;Item {id:card;width:parent.width;height:page.implicitHeight+24;Rectangle {anchors.fill:parent;color:theme.background}
    Components.PluginsPage {id:page;width:parent.width-24;x:12;y:12;bridge:bridge;theme:theme}}}
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  function reply(value){var cmd=bridge.sent[bridge.sent.length-1];bridge.finishRequest({id:cmd.id,ok:true,value:value})}
  Timer {interval:400;running:true;onTriggered:{
    var manifest={api:1,id:"games.valheim",name:"Valheim Modpacks",version:"0.2.0",description:"Share Valheim packs",permissions:["documents","thunderstore","managed_files","local_archives"]}
    reply({plugins:[{manifest:manifest,enabled:true}]});bridge.plugins.open("games.valheim","")
    var view={title:"Valheim Modpacks",description:"Pin mods, resolve dependencies, and share a revision with your friends.",fields:[{id:"title",label:"Pack name",value:"Our Viking adventures"},{id:"packages",label:"Thunderstore dependencies",value:"denikson-BepInExPack_Valheim-5.4.2351",multiline:true}],actions:[{id:"save",label:"Save draft"}],packages:["denikson-BepInExPack_Valheim-5.4.2351"],files:[{package:"denikson-BepInExPack_Valheim-5.4.2351",sha256:"a".repeat(64),size:702900}],rows:[["denikson-BepInExPack_Valheim-5.4.2351","702900 bytes","a".repeat(12)]],document:{game:"valheim"}}
    reply({view:view,generation:4,digest:"abc",shared:[]});input.wait(70)
    test.check(test.find(page,"pluginShare",[]).enabled===false,"personal drafts cannot share without a conversation")
    var editor=test.find(page,"pluginField-title",[]);editor.text="Vikings";input.wait(30)
    test.check(bridge.plugins.fields.title==="Vikings" && bridge.plugins.dirty,"guest field edits stay local")
    test.check(!test.find(page,"pluginResolve",[]).enabled,"resolve cannot silently ignore unsaved fields")
    bridge.plugins.action("save");var last=bridge.sent[bridge.sent.length-1]
    test.check(last.name==="plugin_action" && last.args.generation===4 && last.args.fields.title==="Vikings","actions carry fields and optimistic state generation")
    view.fields[0].value="Vikings";reply({view:view,generation:5,digest:"abc",shared:[]});input.wait(40)
    test.check(!bridge.plugins.dirty,"saved fields match current view")
    test.check(test.find(page,"pluginAddArchive",[]).visible,"updated plugin exposes private ZIP import")
    var beforeInspect=bridge.sent.length;bridge.plugins.inspectArchive("file:///tmp/Foundation-0.1.0.zip")
    var inspection=bridge.sent[bridge.sent.length-1]
    test.check(inspection.name==="plugin_inspect_archive" && bridge.sent.length===beforeInspect+1,"selection only inspects, without adding or sharing")
    reply({archive_review:{path:"/tmp/Foundation-0.1.0.zip",artifact:{package:"Local-Foundation-0.1.0",sha256:"b".repeat(64),size:100,source:"local"},dependencies:[]}})
    test.check(bridge.plugins.archiveReview!==null,"private ZIP review is shown")
    bridge.plugins.addArchive();var addition=bridge.sent[bridge.sent.length-1]
    test.check(addition.name==="plugin_add_archive" && addition.args.sha256==="b".repeat(64) && addition.args.generation===5,"explicit add pins the reviewed hash and current draft")
    view.archives=[{package:"Local-Foundation-0.1.0",sha256:"b".repeat(64),size:100,source:"local"}]
    reply({view:view,generation:6,digest:"abc",shared:[]})
    test.check(bridge.plugins.archiveReview===null,"successful add clears review")
    bridge.plugins.archiveReview={path:"stale",artifact:{package:"Local-Foundation-0.1.0",sha256:"b".repeat(64),size:100},dependencies:[]};bridge.plugins.open("games.valheim","")
    test.check(bridge.plugins.archiveReview===null,"changing drafts clears private-file approval")
    reply({view:view,generation:7,digest:"abc",shared:[]})
    var oldEpoch=bridge.plugins.epoch;bridge.plugins.reset();bridge.plugins.finish({ok:true,value:{view:view,generation:99}},{epoch:oldEpoch,command:"plugin_open"})
    test.check(bridge.plugins.view===null,"disconnect rejects stale plugin replies")
    bridge.plugins.open("games.valheim","");reply({view:view,generation:5,digest:"abc",shared:[]});input.wait(40)
    test.check(!bridge.sent.some(function(c){return /^(join_|share$|camera$|send_|plugin_share|plugin_apply)/.test(c.name)}),"viewing and editing never publishes media, shares documents, or installs mods")
    var screenshot=Quickshell.env("WISP_PLUGINS_SCREENSHOT")
    if(screenshot){card.grabToImage(function(image){image.saveToFile(screenshot)});input.wait(100)}
    console.log(test.failed ? "PLUGINS_FAILED" : "PLUGINS_OK");Qt.quit()
  }}
}
