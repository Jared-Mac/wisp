import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  objectName: "serverSettingsView"
  required property var bridge
  required property var theme
  property var selectedMemberIds: []
  property string pendingDeleteKind: ""
  property string pendingDeleteId: ""
  property string pendingDeleteName: ""
  property var pendingModeration: ({})
  readonly property bool owner: String(bridge.serverSettings.role || "") === "owner"
  readonly property var categories: [{id:"",name:"No category"}].concat(bridge.serverSettings.categories || [])
  width: parent ? parent.width : 0
  spacing: theme.spacing.xl

  function categoryIndex(id) {
    var value = String(id || "")
    for (var i=0; i<categories.length; ++i) if (String(categories[i].id) === value) return i
    return 0
  }
  function toggleMember(id) {
    id = String(id)
    selectedMemberIds = selectedMemberIds.indexOf(id) >= 0
      ? selectedMemberIds.filter(function(value) { return value !== id })
      : selectedMemberIds.concat([id])
  }
  function confirmDelete(kind, id, name) {
    pendingDeleteKind = kind; pendingDeleteId = String(id); pendingDeleteName = String(name)
    deleteDialog.open()
  }
  function deletePending() {
    var action = pendingDeleteKind === "category" ? "delete_server_category"
      : pendingDeleteKind === "channel" ? "delete_server_channel" : "delete_server_room"
    bridge.serverMutation(action, {id:pendingDeleteId})
    deleteDialog.close()
  }
  function confirmModeration(action, member) {
    pendingModeration = {action:action,id:String(member.id),name:String(member.display_name),serverId:String(bridge.activeServer.id)}
    moderationReason.text = ""
    moderationDialog.open()
  }
  function moderatePending() {
    if (String(bridge.activeServer.id) !== pendingModeration.serverId) { moderationDialog.close(); return }
    if (bridge.serverMutation("moderate_server_member", {user_id:pendingModeration.id,action:pendingModeration.action,reason:moderationReason.text.trim()})) moderationDialog.close()
  }

  Row {
    width: parent.width
    spacing: theme.spacing.sm
    Column {
      width: parent.width - refreshButton.width - parent.spacing
      Text {
        text: "Server settings"
        color: root.theme.foreground
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
      }
      Text {
        width: parent.width; wrapMode: Text.WordWrap
        text: root.owner
          ? "Owner · manage this server, its members, and administrators."
          : "Administrator · manage members, rooms, and channels."
        color: root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
      ChatButton {
        id: refreshButton
        objectName: "serverSettingsRefresh"
      theme: root.theme; text: root.bridge.serverSettingsBusy ? "loading…" : "refresh"
      enabled: !root.bridge.serverSettingsBusy
      onClicked: root.bridge.refreshServerSettings()
    }
  }

  Text {
    visible: !!root.bridge.serverSettingsFeedback
    width: parent.width; wrapMode: Text.WordWrap
    text: root.bridge.serverSettingsFeedback
    color: String(text).toLocaleLowerCase().indexOf("saved") >= 0 ? root.theme.accent : root.theme.danger
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }

  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Text {
      text: "Server identity"
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "The name everyone sees in Wisp."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Row {
      width: parent.width; spacing: root.theme.spacing.sm
      TextField {
        id: serverName
        objectName: "serverNameField"
        property bool wispTextEditor: true
        width: parent.width - saveServerName.width - parent.spacing
        height: root.theme.space(34); maximumLength: 60
        text: String(root.bridge.serverSettings.name || root.bridge.activeServer.name || "")
        placeholderText: "Server name"; color: root.theme.foreground; placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
        background: Rectangle { color: root.theme.background; border.width: 1; border.color: serverName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
        Keys.onReturnPressed: if (saveServerName.enabled) saveServerName.clicked()
        Keys.onEnterPressed: if (saveServerName.enabled) saveServerName.clicked()
      }
      ChatButton {
        id: saveServerName
        objectName: "saveServerName"
        theme: root.theme; text: "save"; primary: true; height: root.theme.space(34)
        enabled: !root.bridge.serverSettingsBusy && !!serverName.text.trim()
          && serverName.text.trim() !== String(root.bridge.serverSettings.name || "")
        onClicked: root.bridge.serverMutation("rename_server", {name:serverName.text.trim()})
      }
    }
  }


  SettingsSection {
    theme:root.theme;title:"People and roles";summary:"Manage members and administrators";objectName:"serverRolesSection"
  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Item { objectName: "settingsServerRoles"; width: 0; height: 0 }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Members can be managed while offline. Only the owner can manage administrators."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Repeater {
      model: root.bridge.serverSettings.members || []
      delegate: Rectangle {
        required property var modelData
        width: root.width; height: memberContent.implicitHeight + root.theme.spacing.sm * 2
        radius: root.theme.cornerRadius; color: root.theme.alpha(root.theme.foreground, 0.04)
        border.width: root.theme.tui ? 1 : 0; border.color: root.theme.separator
        Column {
          id: memberContent
          anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top
          anchors.margins:root.theme.spacing.sm;spacing:root.theme.spacing.xs
        Text {
          width: parent.width
          text: String(modelData.display_name) + " · " + String(modelData.role)
          elide: Text.ElideRight; color: root.theme.foreground
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        Flow {
          width:parent.width;spacing:root.theme.spacing.xs
        ChatButton {
          id: roleButton
          visible: root.owner && modelData.role !== "owner"
          height: root.theme.space(28)
          theme: root.theme
          text: modelData.role === "admin" ? "remove admin" : "make admin"
          destructive: modelData.role === "admin"
          enabled: !root.bridge.serverSettingsBusy
          onClicked: root.bridge.serverMutation("set_server_admin", {user_id:String(modelData.id),admin:modelData.role !== "admin"})
        }
        ChatButton {
          objectName:"kickServerMember-"+String(modelData.id)
          theme:root.theme;text:"kick";destructive:true;height:root.theme.space(28)
          visible:!!root.bridge.serverSettings.member_moderation && modelData.role!=="owner" && (root.owner || modelData.role!=="admin")
          enabled:!root.bridge.serverSettingsBusy
          ToolTip.visible:hovered;ToolTip.text:"Remove from this server; they can rejoin with a new invite."
          onClicked:root.confirmModeration("kick",modelData)
        }
        ChatButton {
          objectName:"banServerMember-"+String(modelData.id)
          theme:root.theme;text:"ban";destructive:true;height:root.theme.space(28)
          visible:!!root.bridge.serverSettings.member_moderation && modelData.role!=="owner" && (root.owner || modelData.role!=="admin")
          enabled:!root.bridge.serverSettingsBusy
          ToolTip.visible:hovered;ToolTip.text:"Remove from this server and prevent rejoining."
          onClicked:root.confirmModeration("ban",modelData)
        }
        }
        }
      }
    }
  }
  }

  SettingsSection {
    theme:root.theme;title:"Banned members";summary:"Manage server bans";objectName:"serverBansSection"
    visible:!!root.bridge.serverSettings.member_moderation
    Column {
      width:parent.width;spacing:root.theme.spacing.sm
      Text {
        width:parent.width;wrapMode:Text.WordWrap
        text:(root.bridge.serverSettings.bans || []).length ? "Lifting a ban allows a new invite; it does not rejoin the server." : "No banned members."
        color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
      }
      Repeater {
        model:root.bridge.serverSettings.bans || []
        delegate:Column {
          required property var modelData
          width:parent.width;spacing:root.theme.spacing.xs
          Text {width:parent.width;text:String(modelData.display_name);elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          Text {width:parent.width;visible:!!modelData.reason;text:String(modelData.reason || "");wrapMode:Text.WordWrap;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          ChatButton {objectName:"unbanServerMember-"+String(modelData.id);theme:root.theme;text:"unban";enabled:!root.bridge.serverSettingsBusy;onClicked:root.confirmModeration("unban",modelData)}
        }
      }
    }
  }


  SettingsSection {
    theme:root.theme;title:"Chat categories";summary:"Organize rooms and text channels";objectName:"serverCategoriesSection"
  Column {
    width: parent.width; spacing: root.theme.spacing.sm

    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Categories organize voice rooms and dedicated text channels without changing their permissions."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Row {
      width: parent.width; spacing: root.theme.spacing.sm
      TextField {
        id: newCategoryName
        objectName: "newServerCategoryName"
        property bool wispTextEditor: true
        width: parent.width - createCategory.width - parent.spacing; height: root.theme.space(34)
        maximumLength: 60; placeholderText: "Category name"
        color: root.theme.foreground; placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
        background: Rectangle { color: root.theme.background; border.width: 1; border.color: newCategoryName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
        Keys.onReturnPressed: createCategory.clicked()
        Keys.onEnterPressed: createCategory.clicked()
      }
      ChatButton {
        id: createCategory
        objectName: "createServerCategory"
        theme: root.theme; text: "create"; primary: true
        enabled: !!newCategoryName.text.trim() && !root.bridge.serverSettingsBusy
        onClicked: if (root.bridge.serverMutation("create_server_category", {name:newCategoryName.text.trim()})) newCategoryName.text = ""
      }
    }
    Repeater {
      model: root.bridge.serverSettings.categories || []
      delegate: Row {
        required property var modelData
        width: root.width; spacing: root.theme.spacing.sm
        TextField {
          id: categoryName
          property bool wispTextEditor: true
          width: parent.width - saveCategory.width - deleteCategory.width - parent.spacing * 2
          height: root.theme.space(34); maximumLength: 60
          text: String(modelData.name); color: root.theme.foreground
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          background: Rectangle { color: root.theme.background; border.width: 1; border.color: categoryName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
        }
        ChatButton {
          id: saveCategory; theme: root.theme; text: "save"; height: root.theme.space(34)
          enabled: !!categoryName.text.trim() && categoryName.text.trim() !== String(modelData.name) && !root.bridge.serverSettingsBusy
          onClicked: root.bridge.serverMutation("rename_server_category", {id:String(modelData.id),name:categoryName.text.trim()})
        }
        ChatButton {
          id: deleteCategory; theme: root.theme; text: "delete"; destructive: true; height: root.theme.space(34)
          enabled: !root.bridge.serverSettingsBusy
          onClicked: root.confirmDelete("category", modelData.id, modelData.name)
        }
      }
    }
  }
  }


  SettingsSection {
    theme:root.theme;title:"Dedicated text channels";summary:"Create and edit dedicated chats";objectName:"serverChannelsSection"
  Column {
    width: parent.width; spacing: root.theme.spacing.sm

    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Channels are visible to everyone by default. You can change their audience at any time."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Row {
      width: parent.width; spacing: root.theme.spacing.sm
      TextField {
        id: newChannelName
        objectName: "newServerChannelName"
        property bool wispTextEditor: true
        width: parent.width * 0.58; height: root.theme.space(34); maximumLength: 80
        placeholderText: "Channel name"; color: root.theme.foreground; placeholderTextColor: root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
        background: Rectangle { color: root.theme.background; border.width: 1; border.color: newChannelName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
      }
      WispComboBox {
    theme: root.theme
        id: newChannelCategory
        width: parent.width - newChannelName.width - parent.spacing; height: root.theme.space(34)
        model: root.categories; textRole: "name"
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        ThemeControlStyle { theme: root.theme; control: newChannelCategory; outline: true }
      }
    }
    ChannelAccessEditor {
      id:newChannelAccess;width:parent.width;theme:root.theme;people:root.bridge.serverSettings.members || []
    }
    ChatButton {
      objectName: "createServerChannel"
      theme: root.theme; text: "create channel"; primary: true
      enabled: !!newChannelName.text.trim() && !root.bridge.serverSettingsBusy
      onClicked: {
        var category = root.categories[newChannelCategory.currentIndex]
        if (root.bridge.serverMutation("create_server_channel", {name:newChannelName.text.trim(),category_id:category && category.id ? String(category.id) : null,visibility:newChannelAccess.visibility,member_ids:newChannelAccess.selectedIds})) {
          newChannelName.text = ""; newChannelAccess.selectedIds = []
        }
      }
    }
    Repeater {
      model: root.bridge.serverSettings.channels || []
      delegate: Rectangle {
        required property var modelData
        width: root.width; height: channelEditor.implicitHeight + root.theme.spacing.md * 2
        radius: root.theme.cornerRadius; color: root.theme.alpha(root.theme.foreground, 0.035)
        border.width: root.theme.tui ? 1 : 0; border.color: root.theme.separator
        Column {
          id: channelEditor
          anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
          anchors.margins: root.theme.spacing.md; spacing: root.theme.spacing.sm
          Row {
            width: parent.width; spacing: root.theme.spacing.sm
            TextField {
              id: channelName
              property bool wispTextEditor: true
              width: (parent.width-parent.spacing)/2; height: root.theme.space(34); maximumLength: 80
              text: String(modelData.name); color: root.theme.foreground
              font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
              background: Rectangle { color: root.theme.background; border.width: 1; border.color: channelName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
            }
            WispComboBox {
    theme: root.theme
              id: channelCategory
              width: (parent.width-parent.spacing)/2; height: root.theme.space(34)
              model: root.categories; textRole: "name"
              currentIndex: root.categoryIndex(modelData.category_id)
              font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
              ThemeControlStyle { theme: root.theme; control: channelCategory; outline: true }
            }
          }
          ChannelAccessEditor {
            id:channelAccess;width:parent.width;theme:root.theme;people:root.bridge.serverSettings.members || []
            visibility:String(modelData.visibility || "members");selectedIds:modelData.member_ids || []
          }
          Row {
            anchors.right: parent.right; spacing: root.theme.spacing.sm
            ChatButton {
              id: saveChannel; objectName:"saveChannel-"+String(modelData.id); theme: root.theme; text: "save"; height: root.theme.space(30)
              enabled: !root.bridge.serverSettingsBusy && !!channelName.text.trim()
                && (channelName.text.trim() !== String(modelData.name)
                  || String((root.categories[channelCategory.currentIndex] || {}).id || "") !== String(modelData.category_id || "")
                  || channelAccess.visibility!==String(modelData.visibility || "members")
                  || JSON.stringify(channelAccess.selectedIds.slice().sort())!==JSON.stringify((modelData.member_ids || []).slice().sort()))
              onClicked: {
                var category = root.categories[channelCategory.currentIndex]
                root.bridge.serverMutation("update_server_channel", {id:String(modelData.id),name:channelName.text.trim(),category_id:category && category.id ? String(category.id) : null,visibility:channelAccess.visibility,member_ids:channelAccess.selectedIds})
              }
            }
            ChatButton {
              id: deleteChannel; theme: root.theme; text: "delete"; destructive: true; height: root.theme.space(30)
              enabled: !root.bridge.serverSettingsBusy
              onClicked: root.confirmDelete("channel", modelData.id, modelData.name)
            }
          }
        }
      }
    }
  }
  }


  SettingsSection {
    theme:root.theme;title:"Voice rooms";summary:"Names, categories, and invite-only access";objectName:"serverRoomsSection"
  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Item { objectName: "settingsServerRooms"; width: 0; height: 0 }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Active rooms cannot be deleted. Deleting a room permanently removes its room chat after confirmation."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Repeater {
      model: root.bridge.serverSettings.rooms || []
      delegate: Column {
        required property var modelData
        width: root.width; spacing: root.theme.spacing.xs
        Row {
          width: parent.width; spacing: root.theme.spacing.sm
          TextField {
            id: roomName
            property bool wispTextEditor: true
            width: parent.width * 0.34
            height: root.theme.space(34); maximumLength: 60; text: String(modelData.name)
            color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
            background: Rectangle { color: root.theme.background; border.width: 1; border.color: roomName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
          }
          WispComboBox {
    theme: root.theme
            id: roomCategory
            width: parent.width * 0.25; height: root.theme.space(34)
            model: root.categories; textRole: "name"
            currentIndex: root.categoryIndex(modelData.category_id)
            font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
            ThemeControlStyle { theme: root.theme; control: roomCategory; outline: true }
          }
          Text {
            id: roomState; width: root.theme.space(48); anchors.verticalCenter: parent.verticalCenter
            text: modelData.active ? "active" : "empty"; color: modelData.active ? root.theme.onlineIndicator : root.theme.muted
            font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          }
          ChatButton {
            id: saveRoom; theme: root.theme; text: "save"; height: root.theme.space(34)
            enabled: !!roomName.text.trim() && !root.bridge.serverSettingsBusy
              && (roomName.text.trim() !== String(modelData.name)
                || roomPrivacy.checked !== !!modelData.private
                || String((root.categories[roomCategory.currentIndex] || {}).id || "") !== String(modelData.category_id || ""))
            onClicked: {
              var category=root.categories[roomCategory.currentIndex]
              root.bridge.serverMutation("rename_server_room", {id:String(modelData.id),name:roomName.text.trim(),category_id:category && category.id ? String(category.id) : null,private:roomPrivacy.checked})
            }
          }
          ChatButton {
            id: deleteRoom; theme: root.theme; text: "delete"; destructive: true; height: root.theme.space(34)
            enabled: !modelData.active && !root.bridge.serverSettingsBusy
            onClicked: root.confirmDelete("room", modelData.id, modelData.name)
          }
        }
        CheckBox {
          id: roomPrivacy
          objectName: "serverRoomInviteOnly-" + String(modelData.id)
          text: "Private / invite-only"; checked: !!modelData.private
          enabled: !root.bridge.serverSettingsBusy
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          ThemeControlStyle { theme: root.theme; control: roomPrivacy }
          ToolTip.visible: hovered; ToolTip.text: "Only current members and invitees can access this room."
        }
      }
    }
  }
  }

  Dialog {
    id:moderationDialog;objectName:"serverMemberModerationDialog"
    parent:Overlay.overlay
    width:Math.min(root.theme.space(430),parent ? parent.width-root.theme.space(24) : root.theme.space(430))
    x:parent ? (parent.width-width)/2 : 0;y:parent ? (parent.height-height)/2 : 0
    modal:true;closePolicy:Popup.CloseOnEscape
    title:root.pendingModeration.action==="unban" ? "Lift server ban" : root.pendingModeration.action==="ban" ? "Ban member" : "Kick member"
    font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
    ThemeControlStyle {theme:root.theme;control:moderationDialog;outline:true}
    contentItem:Column {
      spacing:root.theme.spacing.md
      Text {
        width:parent.width;wrapMode:Text.WordWrap;color:root.theme.foreground
        font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
        text:root.pendingModeration.action==="unban"
          ? "Allow “"+root.pendingModeration.name+"” to rejoin with a new invite?"
          : "Remove “"+root.pendingModeration.name+"” from this server and disconnect their voice? "
            +(root.pendingModeration.action==="ban" ? "They cannot rejoin until this ban is lifted." : "They can rejoin with a new invite.")
      }
      TextField {
        id:moderationReason;objectName:"serverModerationReason"
        visible:root.pendingModeration.action==="ban";width:parent.width;maximumLength:280
        placeholderText:"Reason (optional)";color:root.theme.foreground;placeholderTextColor:root.theme.muted
        font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
        background:Rectangle {color:root.theme.background;border.width:1;border.color:root.theme.separator;radius:root.theme.cornerRadius}
      }
    }
    footer:Flow {
      padding:root.theme.spacing.sm;spacing:root.theme.spacing.sm
      ChatButton {objectName:"confirmServerModeration";theme:root.theme;text:root.pendingModeration.action==="unban" ? "lift ban" : root.pendingModeration.action==="ban" ? "ban member" : "kick member";primary:true;destructive:root.pendingModeration.action!=="unban";enabled:!root.bridge.serverSettingsBusy && String(root.bridge.activeServer.id)===root.pendingModeration.serverId;onClicked:root.moderatePending()}
      ChatButton {theme:root.theme;text:"cancel";onClicked:moderationDialog.close()}
    }
  }

  Dialog {
    id: deleteDialog
    parent: Overlay.overlay
    width: Math.min(root.theme.space(430), parent ? parent.width - root.theme.space(24) : root.theme.space(430))
    implicitHeight: root.theme.space(220)
    x: parent ? (parent.width-width)/2 : 0; y: parent ? (parent.height-height)/2 : 0
    modal: true; title: "Confirm deletion"
    closePolicy: Popup.CloseOnEscape
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    ThemeControlStyle { theme: root.theme; control: deleteDialog; outline: true }
    contentItem: Text {
      id: deleteWarning
      wrapMode: Text.WordWrap
      text: root.pendingDeleteKind === "room"
        ? "Delete room “" + root.pendingDeleteName + "”? Its room chat will be cleared for everyone and cannot be recovered."
        : "Delete " + root.pendingDeleteKind + " “" + root.pendingDeleteName + "”? This cannot be reversed."
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    }
    footer: Row {
      padding: root.theme.spacing.sm; spacing: root.theme.spacing.sm; layoutDirection: Qt.RightToLeft
      ChatButton { theme: root.theme; text: "yes, delete"; destructive: true; primary: true; onClicked: root.deletePending() }
      ChatButton { theme: root.theme; text: "cancel"; onClicked: deleteDialog.close() }
    }
  }

  Component.onCompleted: bridge.refreshServerSettings()
  SettingsSection {theme:root.theme;title:"Server emojis";summary:"Shared custom emojis for everyone";objectName:"serverEmojisSection"
  EmojiLibrary {objectName:"serverEmojiLibrary";width:parent.width;bridge:root.bridge;theme:root.theme;serverId:String(root.bridge.activeServer.id);scope:"server"}
  }
}
