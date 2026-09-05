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
          ? "Owner · full Wisp administration. VPS access, secrets, and server shutdown stay outside the app."
          : "Administrator · manage rooms, categories, and channels. Ownership and server infrastructure remain owner-only."
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
      text: "This name appears in the server selector, chat picker, and chat tile paths. The connection address remains private configuration."
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

  Rectangle { width: parent.width; height: 1; color: root.theme.separator }

  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Text {
      text: "People and roles"
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: root.owner ? "Only the owner can grant or revoke persistent server-admin access." : "Only the owner can change server-admin access."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Repeater {
      model: root.bridge.serverSettings.members || []
      delegate: Rectangle {
        required property var modelData
        width: root.width; height: root.theme.space(38)
        radius: root.theme.cornerRadius; color: root.theme.alpha(root.theme.foreground, 0.04)
        border.width: root.theme.tui ? 1 : 0; border.color: root.theme.separator
        Text {
          anchors.left: parent.left; anchors.leftMargin: root.theme.spacing.md
          anchors.right: roleButton.left; anchors.rightMargin: root.theme.spacing.sm
          anchors.verticalCenter: parent.verticalCenter
          text: String(modelData.display_name) + " · " + String(modelData.role)
          elide: Text.ElideRight; color: root.theme.foreground
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        ChatButton {
          id: roleButton
          visible: root.owner && modelData.role !== "owner"
          anchors.right: parent.right; anchors.rightMargin: root.theme.spacing.xs
          anchors.verticalCenter: parent.verticalCenter
          height: root.theme.space(28)
          theme: root.theme
          text: modelData.role === "admin" ? "remove admin" : "make admin"
          destructive: modelData.role === "admin"
          enabled: !root.bridge.serverSettingsBusy
          onClicked: root.bridge.serverMutation("set_server_admin", {user_id:String(modelData.id),admin:modelData.role !== "admin"})
        }
      }
    }
  }

  Rectangle { width: parent.width; height: 1; color: root.theme.separator }

  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Text {
      text: "Chat categories"
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
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

  Rectangle { width: parent.width; height: 1; color: root.theme.separator }

  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Text {
      text: "Dedicated text channels"
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Choose friends who should receive the channel. Chat contents remain end-to-end encrypted."
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
      ComboBox {
        id: newChannelCategory
        width: parent.width - newChannelName.width - parent.spacing; height: root.theme.space(34)
        model: root.categories; textRole: "name"
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        ThemeControlStyle { theme: root.theme; control: newChannelCategory; outline: true }
      }
    }
    Flow {
      width: parent.width; spacing: root.theme.spacing.xs
      Repeater {
        model: root.bridge.friends || []
        CheckDelegate {
          id: friendChoice
          required property var modelData
          height: root.theme.space(30)
          text: String(modelData.display_name)
          checked: root.selectedMemberIds.indexOf(String(modelData.id)) >= 0
          onClicked: root.toggleMember(String(modelData.id))
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          ThemeControlStyle { theme: root.theme; control: friendChoice }
        }
      }
    }
    ChatButton {
      objectName: "createServerChannel"
      theme: root.theme; text: "create channel"; primary: true
      enabled: !!newChannelName.text.trim() && !root.bridge.serverSettingsBusy
      onClicked: {
        var category = root.categories[newChannelCategory.currentIndex]
        if (root.bridge.serverMutation("create_server_channel", {name:newChannelName.text.trim(),category_id:category && category.id ? String(category.id) : null,member_ids:root.selectedMemberIds})) {
          newChannelName.text = ""; root.selectedMemberIds = []
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
            ComboBox {
              id: channelCategory
              width: (parent.width-parent.spacing)/2; height: root.theme.space(34)
              model: root.categories; textRole: "name"
              currentIndex: root.categoryIndex(modelData.category_id)
              font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
              ThemeControlStyle { theme: root.theme; control: channelCategory; outline: true }
            }
          }
          Row {
            anchors.right: parent.right; spacing: root.theme.spacing.sm
            ChatButton {
              id: saveChannel; theme: root.theme; text: "save"; height: root.theme.space(30)
              enabled: !root.bridge.serverSettingsBusy && !!channelName.text.trim()
                && (channelName.text.trim() !== String(modelData.name)
                  || String((root.categories[channelCategory.currentIndex] || {}).id || "") !== String(modelData.category_id || ""))
              onClicked: {
                var category = root.categories[channelCategory.currentIndex]
                root.bridge.serverMutation("update_server_channel", {id:String(modelData.id),name:channelName.text.trim(),category_id:category && category.id ? String(category.id) : null})
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

  Rectangle { width: parent.width; height: 1; color: root.theme.separator }

  Column {
    width: parent.width; spacing: root.theme.spacing.sm
    Text {
      text: "Voice rooms"
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.WordWrap
      text: "Active rooms cannot be deleted. Deleting a room permanently removes its room chat after confirmation."
      color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Repeater {
      model: root.bridge.serverSettings.rooms || []
      delegate: Row {
        required property var modelData
        width: root.width; spacing: root.theme.spacing.sm
        TextField {
          id: roomName
          property bool wispTextEditor: true
          width: parent.width * 0.34
          height: root.theme.space(34); maximumLength: 60; text: String(modelData.name)
          color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          background: Rectangle { color: root.theme.background; border.width: 1; border.color: roomName.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
        }
        ComboBox {
          id: roomCategory
          width: parent.width * 0.25; height: root.theme.space(34)
          model: root.categories; textRole: "name"
          currentIndex: root.categoryIndex(modelData.category_id)
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          ThemeControlStyle { theme: root.theme; control: roomCategory; outline: true }
        }
        Text {
          id: roomState; width: root.theme.space(48); anchors.verticalCenter: parent.verticalCenter
          text: modelData.active ? "active" : "empty"; color: modelData.active ? root.theme.open : root.theme.muted
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        ChatButton {
          id: saveRoom; theme: root.theme; text: "save"; height: root.theme.space(34)
          enabled: !!roomName.text.trim() && !root.bridge.serverSettingsBusy
            && (roomName.text.trim() !== String(modelData.name)
              || String((root.categories[roomCategory.currentIndex] || {}).id || "") !== String(modelData.category_id || ""))
          onClicked: {
            var category=root.categories[roomCategory.currentIndex]
            root.bridge.serverMutation("rename_server_room", {id:String(modelData.id),name:roomName.text.trim(),category_id:category && category.id ? String(category.id) : null})
          }
        }
        ChatButton {
          id: deleteRoom; theme: root.theme; text: "delete"; destructive: true; height: root.theme.space(34)
          enabled: !modelData.active && !root.bridge.serverSettingsBusy
          onClicked: root.confirmDelete("room", modelData.id, modelData.name)
        }
      }
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
}
