import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

Column {
  id: root
  required property var bridge
  required property var theme
  readonly property var plugins: bridge.plugins
  function conversationLabel(c) {
    var server=(bridge.servers || []).filter(function(s){return String(s.id)===String(c.server_id)})[0]
    return String(c.label || "Conversation") + (server ? " · "+server.name : "") + (c.server_channel ? " · server channel" : " · friends")
  }
  property var scopes: [{id:"",label:"Personal draft"}].concat((bridge.conversations || []).map(function(c){return {id:String(c.id),label:root.conversationLabel(c)}}))
  readonly property var selectedPackage: plugins.installed.filter(function(p){return p.manifest.id===root.plugins.selected})[0] || null
  readonly property bool supportsArchives: !!selectedPackage && (selectedPackage.manifest.permissions || []).indexOf("local_archives")>=0
  spacing: theme.spacing.md
  Component.onCompleted: plugins.refresh()
  Text {text:"Plugins";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
  Text {width:parent.width;text:"Shared tools for your friends and servers";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
  Flow {
    width:parent.width;spacing:root.theme.spacing.sm
    ChatButton {theme:root.theme;text:"Install plugin…";enabled:!root.plugins.busy;onClicked:packagePicker.open()}
    ChatButton {theme:root.theme;text:"Refresh";enabled:!root.plugins.busy;onClicked:root.plugins.selected ? root.plugins.open(root.plugins.selected,root.plugins.conversation) : root.plugins.refresh()}
  }
  Rectangle {
    visible:root.plugins.review!==null;width:parent.width;height:reviewContent.implicitHeight+root.theme.spacing.lg*2
    color:root.theme.surface;radius:root.theme.cornerRadius;border.color:root.theme.focusBorder;border.width:1
    Column {
      id:reviewContent;x:root.theme.spacing.lg;y:x;width:parent.width-x*2;spacing:root.theme.spacing.sm
      Text {width:parent.width;text:root.plugins.review ? root.plugins.review.manifest.name+" · "+root.plugins.review.manifest.version : "";color:root.theme.foreground;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:root.theme.font.body;font.family:root.theme.font.family}
      Text {width:parent.width;text:root.plugins.review ? root.plugins.review.manifest.description : "";color:root.theme.muted;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:root.theme.font.body;font.family:root.theme.font.family}
      Text {width:parent.width;text:root.plugins.review ? "Permissions: "+root.plugins.review.manifest.permissions.map(function(p){return ({documents:"shared documents",thunderstore:"Thunderstore downloads",managed_files:"local mod profiles",local_archives:"private mod ZIPs you select"})[p] || p}).join(", ")+"\nPackage SHA-256: "+root.plugins.review.digest : "";color:root.theme.muted;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText;font.pixelSize:root.theme.font.caption;font.family:root.theme.font.family}
      Text {width:parent.width;text:"Plugins run in an isolated Wasm runtime. Sharing and installing game mods require your action. Game mods run with the game's permissions when you launch it.";color:root.theme.muted;wrapMode:Text.Wrap;font.pixelSize:root.theme.font.caption;font.family:root.theme.font.family}
      Flow {width:parent.width;spacing:root.theme.spacing.sm
        ChatButton {theme:root.theme;text:"Install reviewed package";enabled:!root.plugins.busy;onClicked:root.plugins.install()}
        ChatButton {theme:root.theme;text:"Cancel";enabled:!root.plugins.busy;onClicked:root.plugins.review=null}
      }
    }
  }
  ComboBox {
    id:pluginChoice;objectName:"pluginChoice";width:parent.width;enabled:!root.plugins.busy
    model:root.plugins.installed.map(function(p){return p.manifest.name+(p.enabled ? "" : " · disabled")})
    currentIndex:root.plugins.installed.findIndex(function(p){return p.manifest.id===root.plugins.selected})
    displayText:currentIndex<0 ? "Choose an installed plugin" : currentText
    ThemeControlStyle {theme:root.theme;control:pluginChoice}
    onActivated:root.plugins.open(root.plugins.installed[currentIndex].manifest.id,root.plugins.conversation)
  }
  ComboBox {
    id:scopeChoice;objectName:"pluginConversation";width:parent.width;enabled:!root.plugins.busy
    model:root.scopes.map(function(c){return c.label})
    currentIndex:Math.max(0,root.scopes.findIndex(function(c){return c.id===root.plugins.conversation}))
    ThemeControlStyle {theme:root.theme;control:scopeChoice}
    onActivated:root.plugins.open(root.plugins.selected,root.scopes[currentIndex].id)
  }
  Flow {
    visible:root.selectedPackage!==null;width:parent.width;spacing:root.theme.spacing.sm
    ChatButton {theme:root.theme;text:root.selectedPackage && root.selectedPackage.enabled ? "Disable plugin" : "Enable plugin";enabled:!root.plugins.busy;onClicked:root.plugins.request("plugin_enable",{enabled:!root.selectedPackage.enabled})}
    ChatButton {theme:root.theme;text:"Remove plugin";enabled:!root.plugins.busy;onClicked:removeDialog.open()}
  }
  Text {objectName:"pluginFeedback";width:parent.width;text:root.plugins.feedback;visible:text.length>0;color:root.theme.muted;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;Accessible.role:Accessible.AlertMessage}
  Column {
    width:parent.width;spacing:root.theme.spacing.md;visible:root.plugins.view!==null
    Text {width:parent.width;text:root.plugins.view ? root.plugins.view.title : "";wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title}
    Text {width:parent.width;text:root.plugins.view ? root.plugins.view.description : "";wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    Repeater {
      model:root.plugins.view ? root.plugins.view.fields || [] : []
      delegate:Column {
        required property var modelData
        width:parent.width;spacing:root.theme.spacing.xs
        Text {width:parent.width;text:modelData.label;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
        TextArea {
          id:field;objectName:"pluginField-"+modelData.id;width:parent.width
          text:String(root.plugins.fields[modelData.id] || "");readOnly:root.plugins.busy;wrapMode:TextEdit.Wrap
          textFormat:TextEdit.PlainText;selectByMouse:true
          font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
          Accessible.name:modelData.label
          ThemeControlStyle {theme:root.theme;control:field}
          onTextChanged:if(text!==String(root.plugins.fields[modelData.id] || ""))root.plugins.setField(modelData.id,text)
        }
      }
    }
    Flow {
      width:parent.width;spacing:root.theme.spacing.sm
      Repeater {model:root.plugins.view ? root.plugins.view.actions || [] : [];delegate:ChatButton {required property var modelData;theme:root.theme;text:modelData.label;enabled:!root.plugins.busy;onClicked:root.plugins.action(modelData.id)}}
      ChatButton {objectName:"pluginAddArchive";theme:root.theme;text:"Add private mod ZIP…";visible:root.supportsArchives;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:archivePicker.open()}
      ChatButton {objectName:"pluginResolve";theme:root.theme;text:"Resolve dependencies";visible:!!root.plugins.view && ((root.plugins.view.packages || []).length>0 || (root.plugins.view.archives || []).length>0);enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.request("plugin_resolve",{})}
      ChatButton {objectName:"pluginShare";theme:root.theme;text:"Share this revision…";visible:!!root.plugins.view && !!root.plugins.view.document;enabled:!root.plugins.busy && !root.plugins.dirty && root.plugins.conversation!=="";onClicked:shareDialog.open()}
      ChatButton {objectName:"pluginApply";theme:root.theme;text:"Install local profile…";visible:!!root.plugins.view && (root.plugins.view.files || []).length>0;enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:profilePicker.open()}
    }
    Column {
      visible:root.plugins.archiveReview!==null;width:parent.width;spacing:root.theme.spacing.sm
      Text {width:parent.width;text:root.plugins.archiveReview ? "Review private ZIP: "+root.plugins.archiveReview.artifact.package+"\n"+root.plugins.archiveReview.artifact.size+" bytes · SHA-256 "+root.plugins.archiveReview.artifact.sha256+"\nDependencies: "+root.plugins.archiveReview.dependencies.join(", ")+"\nThis file stays local until you share a pack revision. Everyone in the selected conversation will receive its contents." : "";wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
      Flow {width:parent.width;spacing:root.theme.spacing.sm
        ChatButton {objectName:"pluginConfirmArchive";theme:root.theme;text:"Add reviewed ZIP";enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.addArchive()}
        ChatButton {theme:root.theme;text:"Cancel";enabled:!root.plugins.busy;onClicked:root.plugins.archiveReview=null}
      }
    }
    Repeater {model:root.plugins.view ? root.plugins.view.archives || [] : [];delegate:RowLayout {
      required property var modelData;width:parent.width
      Text {Layout.fillWidth:true;text:"Private ZIP · "+modelData.package;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
      ChatButton {theme:root.theme;text:"Remove ZIP";enabled:!root.plugins.busy && !root.plugins.dirty;onClicked:root.plugins.request("plugin_remove_archive",{sha256:modelData.sha256})}
    }}
    Repeater {model:root.plugins.view ? root.plugins.view.rows || [] : [];delegate:Text {required property var modelData;width:parent.width;text:modelData.join(" · ");wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}}
    Text {width:parent.width;visible:root.plugins.folder!=="";text:"Installed profile: "+root.plugins.folder+"\nOn Linux, launch Valheim with this profile's start_game_bepinex.sh and the absolute path to valheim.x86_64. Older profile folders remain available for rollback.";wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    ChatButton {theme:root.theme;text:"Open profile folder";visible:root.plugins.folderUrl!=="";onClicked:Qt.openUrlExternally(root.plugins.folderUrl)}
  }
  Column {
    width:parent.width;spacing:root.theme.spacing.sm;visible:root.plugins.conversation!=="" && root.plugins.selected!==""
    Text {text:"Shared in this conversation";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.weight:Font.DemiBold}
    Text {width:parent.width;visible:root.plugins.shared.length===0;text:"Shared pack attachments from loaded chat history appear here. Refresh after someone shares a revision.";wrapMode:Text.Wrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    Repeater {model:root.plugins.shared;delegate:ChatButton {required property var modelData;theme:root.theme;width:parent.width;text:"Import from "+modelData.author+" · "+modelData.name;enabled:!root.plugins.busy;onClicked:root.plugins.request("plugin_import",{message_id:modelData.id})}}
  }
  FolderDialog {id:packagePicker;title:"Choose a plugin folder containing plugin.json and plugin.wasm";onAccepted:root.plugins.inspect(selectedFolder)}
  FolderDialog {id:profilePicker;title:"Choose a folder for a new, separate game profile";onAccepted:root.plugins.apply(selectedFolder)}
  FileDialog {id:archivePicker;title:"Choose a private mod ZIP containing manifest.json";nameFilters:["Mod packages (*.zip)"];onAccepted:root.plugins.inspectArchive(selectedFile)}
  Dialog {id:shareDialog;ThemeControlStyle {theme:root.theme;control:shareDialog;outline:true}
    title:"Share this pack revision?";modal:true;standardButtons:Dialog.Ok | Dialog.Cancel
    Text {width:Math.min(root.width,400);wrapMode:Text.Wrap;text:"Send the locked modpack, including any private ZIP contents, to everyone in the selected conversation. Each person chooses whether to install it.";color:root.theme.foreground}
    onAccepted:root.plugins.request("plugin_share",{})
  }
  Dialog {id:removeDialog;ThemeControlStyle {theme:root.theme;control:removeDialog;outline:true}
    title:"Remove this plugin?";modal:true;standardButtons:Dialog.Ok | Dialog.Cancel
    Text {width:Math.min(root.width,400);wrapMode:Text.Wrap;text:"Local game profiles, shared messages, and saved drafts will be kept.";color:root.theme.foreground}
    onAccepted:root.plugins.request("plugin_remove",{})
  }
}
