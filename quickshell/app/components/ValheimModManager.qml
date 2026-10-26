import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../ModpackLogic.js" as Mods

Column {
  id: root
  objectName: "valheimModManager"
  required property var bridge
  required property var theme
  required property var scopes
  readonly property var plugins: bridge.plugins
  readonly property var view: plugins.view || ({})
  readonly property var mods: Mods.mods(view,plugins.fields.packages)
  readonly property var filtered: mods.filter(function(m){return (!search.text || (m.name+" "+m.author+" "+m.version).toLowerCase().indexOf(search.text.toLowerCase())>=0) && (sourceFilter.currentIndex===0 || (sourceFilter.currentIndex===1 ? !m.local : m.local))})
  readonly property var selected: mods.filter(function(m){return m.id===root.selectedId})[0] || null
  readonly property bool resolved: (view.files || []).length>0 && !plugins.dirty
  readonly property bool editable: !plugins.serverPack || plugins.canPublish
  readonly property bool wide: width>=theme.space(740)
  property string selectedId: ""
  property int tab: 0
  property bool advanced: false
  property string nextScope: ""
  spacing: theme.spacing.lg
  signal catalogRequested()
  signal archiveRequested()
  signal profileRequested()
  signal shareRequested()
  function scopeIndex(){return Math.max(0,scopes.findIndex(function(c){return c.id===root.plugins.conversation}))}
  function changeScope(id) {
    if(id===plugins.conversation)return
    if(plugins.dirty){nextScope=id;scopeWarning.open()}
    else {selectedId="";plugins.open(plugins.selected,id)}
    scopeChoice.currentIndex=Qt.binding(function(){return root.scopeIndex()})
  }
  function removeSelected() {
    if(!selected)return
    if(selected.local)plugins.request("plugin_remove_archive",{sha256:selected.sha256})
    else plugins.setField("packages",Mods.roots(plugins.fields.packages).filter(function(id){return id!==root.selectedId}).join("\n"))
    selectedId=""
  }

  RowLayout {
    width:parent.width;spacing:root.theme.spacing.md
    Rectangle {
      Layout.preferredWidth:root.theme.space(48);Layout.preferredHeight:width
      radius:root.theme.cornerRadius;color:root.theme.alpha(root.theme.accent,0.12)
      Text {anchors.centerIn:parent;text:"V";font.pixelSize:root.theme.space(30);font.weight:Font.DemiBold;font.family:root.theme.font.family;color:root.theme.accent}
    }
    Column {
      Layout.fillWidth:true;spacing:root.theme.space(2)
      Text {text:"VALHEIM";font.letterSpacing:2;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;color:root.theme.accent}
      Text {text:"Mod manager";font.family:root.theme.font.family;font.pixelSize:root.theme.space(26);font.weight:Font.DemiBold;color:root.theme.foreground}
    }
    ChatButton {objectName:"modManagerCatalog";theme:root.theme;text:"All plugins";enabled:!root.plugins.busy;onClicked:root.catalogRequested()}
  }
  Rectangle {
    width:parent.width;height:packHeader.implicitHeight+root.theme.spacing.lg*2
    radius:root.theme.cornerRadius;color:root.theme.surface;border.width:1;border.color:root.theme.separator
    Column {
      id:packHeader;x:root.theme.spacing.lg;y:x;width:parent.width-2*x;spacing:root.theme.spacing.md
      Text {text:"ACTIVE PACK";font.letterSpacing:1.2;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      TextField {
        id:packName;objectName:"pluginField-title";width:parent.width
        text:String(root.plugins.fields.title || "");placeholderText:"Name your modpack";readOnly:!root.editable || root.plugins.busy || !root.plugins.view
        font.family:root.theme.font.family;font.pixelSize:root.theme.space(22);font.weight:Font.DemiBold
        selectByMouse:true;Accessible.name:"Pack name"
        onTextEdited:root.plugins.setField("title",text)
        // Also supports programmatic editing by accessibility clients.
        onTextChanged:if(root.plugins.view && text!==String(root.plugins.fields.title || ""))root.plugins.setField("title",text)
        ThemeControlStyle {theme:root.theme;control:packName}
      }
      RowLayout {
        width:parent.width;spacing:root.theme.spacing.md
        ComboBox {
          id:scopeChoice;objectName:"pluginConversation";Layout.fillWidth:true;enabled:!root.plugins.busy
          model:root.scopes.map(function(c){return c.label});currentIndex:root.scopeIndex()
          Accessible.name:"Pack scope";ThemeControlStyle {theme:root.theme;control:scopeChoice}
          onActivated:root.changeScope(root.scopes[currentIndex].id)
        }
        Text {visible:root.wide;text:root.mods.length+" mods · "+root.mods.filter(function(m){return m.local}).length+" private";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      }
      Flow {
        width:parent.width;spacing:root.theme.spacing.sm
        ChatButton {objectName:"pluginSave";theme:root.theme;text:"Save changes";primary:root.plugins.dirty;visible:root.plugins.dirty && root.editable;enabled:!root.plugins.busy;onClicked:root.plugins.action("save")}
        ChatButton {objectName:"pluginResolve";theme:root.theme;visible:root.editable;text:root.resolved ? "Recheck dependencies" : "Resolve mods";enabled:!root.plugins.busy && !root.plugins.dirty && root.mods.length>0;onClicked:root.plugins.request("plugin_resolve",{})}
        ChatButton {objectName:"pluginApply";theme:root.theme;text:"Install profile…";primary:root.resolved;enabled:!root.plugins.busy && root.resolved;onClicked:root.profileRequested()}
        ChatButton {objectName:"pluginShare";theme:root.theme;text:root.plugins.serverPack ? "Publish to server…" : "Share pack…";visible:root.editable;enabled:!root.plugins.busy && root.resolved && !!root.view.document && !!root.plugins.conversation;onClicked:root.shareRequested();ToolTip.visible:hovered && !enabled;ToolTip.text:!root.plugins.conversation ? "Choose a conversation to share this pack." : "Save and resolve the pack before sharing."}
      }
      Flow {width:parent.width;spacing:root.theme.spacing.sm;visible:root.plugins.serverPack
        ChatButton {objectName:"pluginSeedPersonal";theme:root.theme;text:"Use personal draft";visible:root.editable && root.mods.length===0;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.request("plugin_seed_personal",{})}
        ChatButton {objectName:"pluginLoadServer";theme:root.theme;text:"Load published server pack";visible:root.plugins.updateAvailable || root.plugins.serverDraft;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:loadServer.open()}
      }
      Text {width:parent.width;visible:root.plugins.serverPack;text:root.editable ? (root.plugins.serverDraft ? "Server draft · publish when ready so members receive this revision." : "Server pack · admins publish revisions; members choose when to install.") : "Server pack · managed by your server admins. Installing only changes your PC.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      Text {
        width:parent.width;wrapMode:Text.Wrap;color:root.plugins.dirty ? root.theme.accent : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
        text:root.plugins.dirty ? "Unsaved changes · save before resolving or installing." : root.resolved ? "Dependencies resolved · ready to install or share." : "Add your mods, then resolve their dependencies to prepare the pack."
      }
    }
  }
  RowLayout {
    width:parent.width
    Flow {Layout.fillWidth:true;spacing:root.theme.spacing.sm
      Repeater {model:["Mods", "Shared packs", "Installation"]
        ChatButton {required property string modelData;required property int index;theme:root.theme;text:modelData;primary:root.tab===index;onClicked:root.tab=index}
      }
    }
    ChatButton {theme:root.theme;text:"Refresh";iconOnly:!root.wide;forceIcon:true;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.open(root.plugins.selected,root.plugins.conversation)}
  }
  Rectangle {width:parent.width;height:1;color:root.theme.separator}
  Text {
    objectName:"pluginFeedback";width:parent.width;visible:!!text && text!=="Ready.";wrapMode:Text.Wrap;textFormat:Text.PlainText
    text:root.plugins.feedback.indexOf("Profile ready:")===0 ? "Profile installed. See Installation for its location." : root.plugins.feedback;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;Accessible.role:Accessible.AlertMessage
  }
  Column {
    width:parent.width;spacing:root.theme.spacing.md;visible:root.tab===0
    Flow {
      width:parent.width;spacing:root.theme.spacing.sm
      TextField {id:search;objectName:"modSearch";width:Math.min(root.theme.space(260),parent.width);placeholderText:"Search this pack…";selectByMouse:true;Accessible.name:"Search mods";ThemeControlStyle {theme:root.theme;control:search}}
      ComboBox {id:sourceFilter;width:root.theme.space(154);model:["All sources","Thunderstore","Private ZIPs"];Accessible.name:"Filter mod source";ThemeControlStyle {theme:root.theme;control:sourceFilter}}
      ChatButton {objectName:"modAddPublic";theme:root.theme;text:"Add mod…";enabled:root.editable && !root.plugins.busy && !!root.plugins.view;onClicked:{dependency.text="";addMod.open()}}
      ChatButton {objectName:"pluginAddArchive";theme:root.theme;text:"Add private ZIP…";enabled:root.editable && !root.plugins.busy && !root.plugins.dirty && !!root.plugins.view;onClicked:root.archiveRequested()}
    }
    Rectangle {
      visible:root.plugins.archiveReview!==null;width:parent.width;height:archiveReview.implicitHeight+root.theme.spacing.md*2
      color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.accent
      Column {id:archiveReview;x:root.theme.spacing.md;y:x;width:parent.width-2*x;spacing:root.theme.spacing.sm
        Text {width:parent.width;text:root.plugins.archiveReview ? "Review private mod · "+root.plugins.archiveReview.artifact.package : "";wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
        Text {width:parent.width;text:root.plugins.archiveReview ? Mods.size(root.plugins.archiveReview.artifact.size)+" · Dependencies: "+(root.plugins.archiveReview.dependencies.join(", ") || "None") : "";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        Text {width:parent.width;text:"This ZIP stays local until you share the pack.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        Flow {width:parent.width;spacing:root.theme.spacing.sm
          ChatButton {objectName:"pluginConfirmArchive";theme:root.theme;text:"Add reviewed ZIP";primary:true;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.addArchive()}
          ChatButton {theme:root.theme;text:"Cancel";enabled:!root.plugins.busy;onClicked:root.plugins.archiveReview=null}
        }
      }
    }
    GridLayout {
      width:parent.width;columns:root.wide ? 2 : 1;columnSpacing:root.theme.spacing.md;rowSpacing:root.theme.spacing.md
      Rectangle {
        Layout.fillWidth:true;Layout.alignment:Qt.AlignTop;implicitHeight:modList.implicitHeight+root.theme.spacing.md*2
        color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator
        Column {
          id:modList;x:root.theme.spacing.md;y:x;width:parent.width-x*2;spacing:root.theme.space(4)
          RowLayout {width:parent.width;height:root.theme.space(28)
            Text {Layout.fillWidth:true;text:"MODS IN THIS PACK";font.letterSpacing:1;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
            Text {text:String(root.filtered.length);color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          }
          Repeater {model:root.filtered
            delegate:ItemDelegate {
              id:modRow;required property var modelData;objectName:"modRow-"+modelData.id
              width:parent.width;height:Math.max(root.theme.space(64),rowContent.implicitHeight+root.theme.spacing.md*2)
              Accessible.name:modelData.name+", version "+modelData.version+", "+modelData.source
              onClicked:root.selectedId=modelData.id
              background:Rectangle {radius:root.theme.cornerRadius;color:root.selectedId===modRow.modelData.id ? root.theme.alpha(root.theme.accent,0.14) : modRow.hovered ? root.theme.alpha(root.theme.foreground,0.05) : "transparent";border.width:root.selectedId===modRow.modelData.id ? 1 : 0;border.color:root.theme.focusBorder}
              contentItem:RowLayout {
                id:rowContent;spacing:root.theme.spacing.md
                Rectangle {Layout.preferredWidth:root.theme.space(36);Layout.preferredHeight:width;radius:root.theme.cornerRadius;color:root.theme.alpha(root.theme.accent,0.09)
                  Text {anchors.centerIn:parent;text:modRow.modelData.name.slice(0,1).toUpperCase();color:root.theme.accent;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.weight:Font.DemiBold}
                }
                Column {Layout.fillWidth:true;spacing:root.theme.space(4)
                  Text {width:parent.width;text:modRow.modelData.name.replace(/_/g," ");elide:Text.ElideRight;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.weight:Font.DemiBold}
                  Text {width:parent.width;text:modRow.modelData.source+(modRow.modelData.local ? "" : " · "+modRow.modelData.author)+(modRow.modelData.direct || modRow.modelData.local ? "" : " · dependency");elide:Text.ElideRight;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
                }
                Column {Layout.preferredWidth:root.theme.space(96);spacing:root.theme.space(4)
                  Text {width:parent.width;text:"v"+modRow.modelData.version;elide:Text.ElideRight;horizontalAlignment:Text.AlignRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
                  Text {width:parent.width;text:modRow.modelData.resolved && !root.plugins.dirty ? "Resolved" : "Draft";horizontalAlignment:Text.AlignRight;color:modRow.modelData.resolved && !root.plugins.dirty ? root.theme.accent : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
                }
              }
            }
          }
          Text {width:parent.width;visible:root.filtered.length===0;text:root.mods.length ? "No mods match your search." : "Your pack is empty. Add a Thunderstore mod or a private ZIP to get started.";wrapMode:Text.Wrap;padding:root.theme.spacing.lg;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
        }
      }
      Rectangle {
        Layout.preferredWidth:root.wide ? root.theme.space(240) : -1;Layout.fillWidth:!root.wide;Layout.alignment:Qt.AlignTop
        implicitHeight:details.implicitHeight+root.theme.spacing.lg*2;visible:root.wide || !!root.selected
        color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator
        Column {id:details;x:root.theme.spacing.lg;y:x;width:parent.width-2*x;spacing:root.theme.spacing.md
          Text {text:"MOD DETAILS";font.letterSpacing:1;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          Text {width:parent.width;text:root.selected ? root.selected.name.replace(/_/g," ") : "Select a mod";wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
          Text {width:parent.width;text:root.selected ? "Version "+root.selected.version+"\n"+root.selected.source+(root.selected.size ? " · "+Mods.size(root.selected.size) : "") : "View its version, source and package details here.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          Text {width:parent.width;visible:!!root.selected;text:root.selected ? root.selected.id : "";wrapMode:Text.WrapAnywhere;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          Text {width:parent.width;visible:!!root.selected && !root.selected.direct && !root.selected.local;text:"Required by this pack. Dependencies are managed when you resolve mods.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          ChatButton {objectName:"modRemoveSelected";theme:root.theme;text:"Remove from pack";destructive:true;visible:!!root.selected && (root.selected.direct || root.selected.local);enabled:root.editable && !root.plugins.busy && (!root.selected || !root.selected.local || !root.plugins.dirty);onClicked:removeMod.open()}
        }
      }
    }
    ChatButton {theme:root.theme;text:root.advanced ? "Hide advanced editor" : "Advanced · edit dependency list";onClicked:root.advanced=!root.advanced}
    TextArea {id:rawPackages;objectName:"pluginField-packages";visible:root.advanced;width:parent.width;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;selectByMouse:true;readOnly:!root.editable || root.plugins.busy
      text:String(root.plugins.fields.packages || "");Accessible.name:"Exact Thunderstore dependency versions";placeholderText:"Team-Package-1.2.3, one per line"
      font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;ThemeControlStyle {theme:root.theme;control:rawPackages}
      onTextChanged:if(root.plugins.view && text!==String(root.plugins.fields.packages || ""))root.plugins.setField("packages",text)
    }
  }
  Column {
    width:parent.width;spacing:root.theme.spacing.md;visible:root.tab===1
    Text {text:"Shared packs";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
    Text {width:parent.width;text:root.plugins.serverPack ? "The published pack loads automatically. Admin drafts stay local until published. New members may need an admin to republish for their membership." : !root.plugins.conversation ? "Choose a conversation above to see its shared modpacks." : "Import a shared revision to review its mods, then install it on this PC.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    Text {width:parent.width;visible:!root.plugins.serverPack && !!root.plugins.conversation && root.plugins.shared.length===0;text:"No packs in loaded chat history. Refresh after someone shares a revision.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    Repeater {model:root.plugins.serverPack ? [] : root.plugins.shared
      Rectangle {required property var modelData;width:parent.width;height:sharedRow.implicitHeight+root.theme.spacing.md*2;color:root.theme.surface;radius:root.theme.cornerRadius
        Column {id:sharedRow;x:root.theme.spacing.md;y:x;width:parent.width-2*x;spacing:root.theme.spacing.sm
          Text {width:parent.width;text:"Shared by "+modelData.author;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.weight:Font.DemiBold}
          Text {width:parent.width;text:modelData.name;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          ChatButton {theme:root.theme;text:"Import revision";enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:{root.plugins.request("plugin_import",{message_id:modelData.id});root.tab=0}}
        }
      }
    }
  }
  Column {
    width:parent.width;spacing:root.theme.spacing.md;visible:root.tab===2
    Text {text:"Local installation";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
    Text {width:parent.width;text:root.plugins.folder ? "A profile has been saved on this PC. Install again after changing the pack to create a profile for the new revision." : "No local profile saved for this draft. Resolve the pack, then choose Install profile.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    TextArea {id:installedFolder;width:parent.width;visible:!!root.plugins.folder;readOnly:true;selectByMouse:true;text:root.plugins.folder;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;Accessible.name:"Installed profile folder";font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;ThemeControlStyle {theme:root.theme;control:installedFolder}}
    ChatButton {theme:root.theme;text:"Open profile folder";visible:!!root.plugins.folderUrl;onClicked:Qt.openUrlExternally(root.plugins.folderUrl)}
    Text {width:parent.width;text:"On Linux, start Valheim with this profile’s start_game_bepinex.sh and your absolute Valheim executable path. Game launching and live player sync status are not available here yet.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
  Dialog {id:addMod;objectName:"addModDialog";parent:Overlay.overlay;x:parent ? (parent.width-width)/2 : 0;y:parent ? Math.max(0,(parent.height-height)/2) : 0;title:"Add a Thunderstore mod";modal:true;width:Math.min(parent ? parent.width-root.theme.space(24) : root.width,root.theme.space(480));standardButtons:Dialog.Ok | Dialog.Cancel
    ThemeControlStyle {theme:root.theme;control:addMod;outline:true}
    Column {width:parent.width;spacing:root.theme.spacing.md
      Text {width:parent.width;text:"Paste the mod’s exact dependency ID from Thunderstore.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
      TextField {id:dependency;objectName:"modDependencyInput";width:parent.width;placeholderText:"Team-Package-1.2.3";selectByMouse:true;ThemeControlStyle {theme:root.theme;control:dependency}}
      Text {width:parent.width;visible:!!dependency.text && !Mods.validDependency(dependency.text.trim());text:"Use Team-Package-1.2.3, including a version.";wrapMode:Text.Wrap;color:root.theme.danger;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    }
    onOpened:standardButton(Dialog.Ok).enabled=Qt.binding(function(){return Mods.validDependency(dependency.text.trim())})
    onAccepted:{var id=dependency.text.trim();if(Mods.validDependency(id)){root.plugins.setField("packages",Mods.upsert(root.plugins.fields.packages,id));root.selectedId=id}}
  }
  Dialog {id:removeMod;objectName:"removeModDialog";parent:Overlay.overlay;x:parent ? (parent.width-width)/2 : 0;y:parent ? Math.max(0,(parent.height-height)/2) : 0;title:"Remove mod from this pack?";modal:true;width:Math.min(parent ? parent.width-root.theme.space(24) : root.width,root.theme.space(420));standardButtons:Dialog.Ok | Dialog.Cancel
    ThemeControlStyle {theme:root.theme;control:removeMod;outline:true}
    Text {width:parent.width;text:root.selected ? "Remove "+root.selected.name+" from the draft? Previously installed profiles are kept." : "";wrapMode:Text.Wrap;color:root.theme.foreground}
    onAccepted:root.removeSelected()
  }
  Dialog {id:loadServer;width:Math.min(root.width,420);title:"Load published server pack?";modal:true;standardButtons:Dialog.Ok | Dialog.Cancel
    ThemeControlStyle {theme:root.theme;control:loadServer;outline:true}
    Text {width:Math.min(root.width,400);wrapMode:Text.Wrap;text:"Replace this local server draft with the published pack? Personal drafts and installed profiles will be kept.";color:root.theme.foreground}
    onAccepted:root.plugins.request("plugin_load_server",{})
  }
  Dialog {id:scopeWarning;objectName:"scopeWarningDialog";parent:Overlay.overlay;x:parent ? (parent.width-width)/2 : 0;y:parent ? Math.max(0,(parent.height-height)/2) : 0;title:"Discard unsaved changes?";modal:true;width:Math.min(parent ? parent.width-root.theme.space(24) : root.width,root.theme.space(420));standardButtons:Dialog.Discard | Dialog.Cancel
    ThemeControlStyle {theme:root.theme;control:scopeWarning;outline:true}
    Text {width:parent.width;text:"Save this pack first if you want to keep your edits before changing conversations.";wrapMode:Text.Wrap;color:root.theme.foreground}
    onDiscarded:{root.selectedId="";root.plugins.open(root.plugins.selected,root.nextScope)}
  }
}
