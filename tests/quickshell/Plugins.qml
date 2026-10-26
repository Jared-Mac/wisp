import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ModpackLogic.js" as Mods
ShellRoot {
  id:test;property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("PLUGINS_FAILED: "+label)}}
  function find(item,name,seen){if(!item || seen.indexOf(item)>=0)return null;seen.push(item);if(item.objectName===name)return item;if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}for(var child of Array.from(item.data || []).concat(Array.from(item.children || []))){var r=find(child,name,seen);if(r)return r}return null}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="fixture-"+sent.length;sent.push({name:name,args:args,id:id});return id}}
  FloatingWindow {id:window;visible:true;implicitWidth:Number(Quickshell.env("WISP_PLUGINS_WIDTH") || 980);implicitHeight:980;color:theme.background
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
    var entry={manifest:manifest,digest:"d".repeat(64),installed_digest:"d".repeat(64),enabled:true}
    bridge.serverPlugins.catalog=[entry];bridge.serverPlugins.canManage=true;input.wait(40)
    test.check(test.find(page,"valheimModManager",[])!==null,"Valheim opens its own manager")
    test.check(test.find(page,"serverPluginOpen",[])===null,"catalog management is separate from the mod manager")
    test.find(page,"modManagerCatalog",[]).clicked();input.wait(40)
    test.check(test.find(page,"serverPluginOpen",[]).visible,"catalog is available through All plugins")
    test.find(page,"serverPluginOpen",[]).clicked();input.wait(40)
    reply({installed:{manifest:manifest},plugins:[{manifest:manifest,enabled:true}]})
    reply({view:view,generation:9,digest:"abc",shared:[]});input.wait(40)
    bridge.plugins.openServer(entry,"server-two")
    var prepare=bridge.sent[bridge.sent.length-1]
    test.check(prepare.name==="plugin_server_prepare" && prepare.args.server_id==="server-two" && !prepare.args.path,"opening a server plugin needs no folder picker")
    reply({installed:{manifest:manifest},plugins:[{manifest:manifest,enabled:true}]})
    var opened=bridge.sent[bridge.sent.length-1]
    test.check(opened.name==="plugin_open" && opened.args.server_id==="server-two" && opened.args.server_pack && opened.args.conversation_id==="","prepared plugin defaults to the whole server without a channel")
    reply({view:view,generation:9,digest:"abc",shared:[],server_pack:true,can_publish:false});input.wait(40)
    test.check(bridge.plugins.serverPack && !test.find(page,"modAddPublic",[]).enabled && !test.find(page,"pluginShare",[]).visible,"members view the shared server pack without admin editing controls")
    test.check(page.scopes[0].id==="@server","whole-server scope is listed first")
    bridge.plugins.canPublish=true
    bridge.plugins.setField("title","Keep these unsaved edits")
    bridge.plugins.checkSync(true)
    test.check(bridge.sent[bridge.sent.length-1].name==="plugin_sync_status" && bridge.sent[bridge.sent.length-1].args.verify,"Check requests real file verification")
    reply({sync_status:{state:"update_available",revision:"new-server-revision",digest:"new-plan",can_sync:true,needs_folder:false}});input.wait(30)
    test.check(bridge.plugins.dirty && bridge.plugins.fields.title==="Keep these unsaved edits","polling preserves unsaved fields")
    test.check(test.find(page,"pluginSync",[]).visible,"server update exposes Sync button")
    test.find(page,"pluginSync",[]).clicked()
    var sync=bridge.sent[bridge.sent.length-1]
    test.check(sync.name==="plugin_sync" && sync.args.sync_revision==="new-server-revision" && sync.args.sync_digest==="new-plan" && !sync.args.path,"Sync pins reviewed server revision and reuses installed parent")
    reply({generation:10,folder:"/new/profile",folder_url:"file:///new/profile",sync_status:{state:"up_to_date",revision:"new-server-revision",digest:"new-plan",can_sync:true,checked_at:1}});input.wait(30)
    test.check(bridge.plugins.generation===10 && bridge.plugins.dirty && bridge.plugins.fields.title==="Keep these unsaved edits","sync updates installation without replacing editable draft")
    test.check(!test.find(page,"pluginSync",[]).visible && test.find(page,"pluginSyncStatus",[]).text.indexOf("Up to date")>=0,"verified match is visible without redundant update button")
    bridge.plugins.checkSync(true)
    var failedCheck=bridge.sent[bridge.sent.length-1]
    bridge.finishRequest({id:failedCheck.id,ok:false,error:{message:"Server unavailable"}});input.wait(20)
    test.check(bridge.plugins.syncStatus.state==="unknown","failed check clears stale success status")
    bridge.plugins.setField("title",view.fields[0].value);bridge.plugins.serverDraft=false
    bridge.plugins.syncStatus={state:"update_available",revision:"next",digest:"next-plan",can_sync:true,needs_folder:false}
    bridge.plugins.sync("");reply({generation:11,folder:"/newer/profile",sync_status:{state:"up_to_date"}})
    test.check(bridge.sent[bridge.sent.length-1].name==="plugin_open","sync refreshes the mod list when no draft edits need preservation")
    reply({view:view,generation:12,digest:"next-plan",server_pack:true,can_publish:false,sync_status:{state:"up_to_date"}})
    bridge.plugins.open("games.valheim","")
    test.check(bridge.sent[bridge.sent.length-1].args.server_id==="server-two","personal draft refresh preserves server scope")
    reply({view:view,generation:9,digest:"abc",shared:[]})
    view=Object.assign({},view,{files:view.files.concat([{package:"Local-Foundation-0.1.0",source:"local",sha256:"b".repeat(64),size:33957}])})
    bridge.plugins.view=view;input.wait(30)
    test.check(Mods.upsert("Team-Mod-1.0.0\nTeam-Other-1.0.0","Team-Mod-2.0.0")==="Team-Other-1.0.0\nTeam-Mod-2.0.0","changing a pinned version replaces the existing mod")
    test.check(!Mods.validDependency("../bad") && Mods.validDependency("Team-Mod-1.2.3"),"mod entry validates exact dependency IDs")
    test.check(Mods.mods({packages:["Team-Mod-1.0.0"],files:[{package:"Team-Mod-1.0.0"}]},"").length===0,"removed roots do not reappear as dependencies")
    var manager=test.find(page,"valheimModManager",[])
    test.check(!!manager,"manager restored after server prepare")
    test.find(page,"modAddPublic",[]).clicked();input.wait(40)
    var addDialog=test.find(window.contentItem,"addModDialog",[]), dependency=test.find(window.contentItem,"modDependencyInput",[])
    test.check(!!addDialog && !addDialog.standardButton(Dialog.Ok).enabled,"add-mod dialog cannot submit an empty package")
    dependency.text="Example-NewMod-1.0.0";input.wait(30)
    test.check(addDialog.standardButton(Dialog.Ok).enabled,"valid package enables Add")
    var previousPackages=bridge.plugins.fields.packages, previousRequests=bridge.sent.length
    addDialog.accept();input.wait(30)
    test.check(bridge.plugins.fields.packages.indexOf("Example-NewMod-1.0.0")>=0 && bridge.plugins.dirty && bridge.sent.length===previousRequests,"adding a public mod edits the draft without downloading or sharing")
    bridge.plugins.setField("packages",previousPackages)
    var search=test.find(page,"modSearch",[]);search.text="not-a-mod";input.wait(30)
    test.check(manager.filtered.length===0,"mod search filters the list")
    search.text="";input.wait(30)
    test.check(manager.mods.length===2,"private ZIP and resolved dependency each appear once")
    manager.selectedId="Local-Foundation-0.1.0";input.wait(30)
    test.check(test.find(page,"modRemoveSelected",[]).visible,"private mod has a deliberate removal action")
    page.width=360;input.wait(50)
    test.check(!manager.wide && manager.width<=390,"manager adapts to tray width")
    page.width=Qt.binding(function(){return card.width-24});input.wait(50)
    var screenshot=Quickshell.env("WISP_PLUGINS_SCREENSHOT")
    if(screenshot){card.grabToImage(function(image){image.saveToFile(screenshot)});input.wait(100)}
    var before=bridge.sent.length;bridge.serverPlugins.canManage=false;bridge.serverPlugins.setEnabled(entry,false)
    test.check(bridge.sent.length===before,"members cannot submit server catalog changes")
    var previousEpoch=bridge.serverPlugins.epoch;bridge.serverPlugins.reset()
    bridge.serverPlugins.finish({ok:true,value:{catalog:[entry],can_manage:true}},{epoch:previousEpoch,server:bridge.serverPlugins.server,command:"plugin_server_catalog"})
    test.check(bridge.serverPlugins.catalog.length===0 && !bridge.serverPlugins.canManage,"stale replies cannot restore another server's plugins")
    bridge.plugins.reset()
    test.check(!bridge.plugins.selected && !bridge.plugins.view && !bridge.plugins.serverMode,"server changes clear the previous plugin editor")
    console.log(test.failed ? "PLUGINS_FAILED" : "PLUGINS_OK");Qt.quit()
  }}
}
