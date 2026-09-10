import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs

Column {
  id: root; objectName: "profilePicture"
  required property var bridge
  required property var theme
  required property string serverId
  readonly property string userId: String((bridge.activeServerState.self || {}).id || "")
  readonly property bool busy: !!bridge.avatars.busy[serverId]
  property string selectedPath: ""
  spacing: theme.space(8)
  onServerIdChanged: { selectedPath=""; picker.close() }
  Text { text:"Profile picture"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body; font.bold:true }
  Row {
    width: parent.width; spacing: root.theme.space(12)
    Item {
      width: root.theme.space(64); height: width
      WispAvatar { anchors.fill:parent; theme:root.theme; bridge:root.bridge; userId:root.userId; serverId:root.serverId; name:root.bridge.accountProfile.display_name || ""; visible:!root.selectedPath }
      Image { anchors.fill:parent; visible:!!root.selectedPath; source:root.selectedPath; fillMode:Image.PreserveAspectCrop; asynchronous:true; sourceSize.width:256; sourceSize.height:256 }
    }
    Text { width:Math.max(1,parent.width-root.theme.space(76)); anchors.verticalCenter:parent.verticalCenter; wrapMode:Text.WordWrap; text:"Choose an image up to 2 MB. Pictures are cropped to a square."; color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
  }
  Flow {
    width:parent.width; spacing:root.theme.space(6)
    ChatButton { objectName:"chooseProfilePicture"; theme:root.theme; text:"Choose image"; iconName:"image"; enabled:!root.busy; onClicked:picker.open() }
    ChatButton { objectName:"saveProfilePicture"; theme:root.theme; text:"Save picture"; visible:!!root.selectedPath; enabled:!root.busy; primary:true; onClicked:{root.bridge.avatars.save(root.serverId,root.selectedPath);root.selectedPath=""} }
    ChatButton { theme:root.theme; text:"Cancel"; visible:!!root.selectedPath; enabled:!root.busy; onClicked:root.selectedPath="" }
    ChatButton { objectName:"removeProfilePicture"; theme:root.theme; text:"Remove"; visible:!!root.bridge.avatars.url(root.serverId,root.userId); enabled:!root.busy; onClicked:root.bridge.avatars.save(root.serverId,"") }
  }
  Text { width:parent.width; visible:!!text; wrapMode:Text.WordWrap; text:root.bridge.avatars.feedback[root.serverId] || ""; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
  FileDialog { id:picker; title:"Choose your profile picture"; nameFilters:["Images (*.png *.jpg *.jpeg *.webp *.gif)"]; onAccepted:root.selectedPath=selectedFile.toString() }
}
