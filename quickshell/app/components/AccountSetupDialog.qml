import QtQuick
import QtQuick.Controls

Popup {
  id: root
  required property var setup
  required property var theme
  property bool surfaceActive: false
  property string openedScope: ""
  readonly property var status: setup.status
  readonly property bool classic: status.mode === "classic" && !status.pending
  readonly property bool needsPassword: classic && !!status.identity_ready || !!status.pending
    || status.mode === "secure" && (!status.unlocked || !!status.repair_needed)
  parent: Overlay.overlay
  width: Math.min(theme.space(390), parent ? parent.width-theme.space(24) : theme.space(390))
  height: Math.min(content.implicitHeight+padding*2, parent ? parent.height-theme.space(24) : theme.space(640))
  x: parent ? (parent.width-width)/2 : 0
  y: parent ? Math.max(theme.space(12),(parent.height-height)/2) : 0
  padding:theme.spacing.md
  modal:true; focus:true
  closePolicy:Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background:Rectangle {color:root.theme.surface;border.color:root.theme.separator;radius:root.theme.cornerRadius}
  function maybeOpen() {
    if (visible || !surfaceActive || !setup.claim(root)) return
    openedScope=setup.scope; open()
  }
  function submit(name,args) {
    if (setup.act(name,args || {})) password.text=""
  }
  function reconcile() {
    if (visible && !setup.needsAttention && !setup.busy) close()
    else maybeOpen()
  }
  onSurfaceActiveChanged: { if (!surfaceActive) close(); else Qt.callLater(maybeOpen) }
  Component.onCompleted: Qt.callLater(maybeOpen)
  onOpened: if (password.visible) password.forceActiveFocus()
  onClosed: {password.text=""; recovery.expanded=false; setup.dismiss(root,openedScope)}
  Connections {
    target:root.setup
    function onOfferedChanged() { Qt.callLater(root.maybeOpen) }
    function onHostChanged() { Qt.callLater(root.maybeOpen) }
    function onInvalidated() {password.text=""; root.close()}
    function onStatusChanged() {Qt.callLater(root.reconcile)}
    function onBusyChanged() {Qt.callLater(root.reconcile)}
  }
  component Label: Text {
    width:content.width; wrapMode:Text.WordWrap; textFormat:Text.PlainText
    color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
  }
  contentItem:ScrollView {
    clip:true
    Column {
      id:content; width:root.availableWidth; spacing:root.theme.spacing.sm
      Row {
        width:parent.width; spacing:root.theme.spacing.sm
        Text {
          width:parent.width-closeButton.width-parent.spacing; anchors.verticalCenter:parent.verticalCenter
          text:root.classic ? "Finish updating your account" : "Restore account access"
          wrapMode:Text.Wrap; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body; font.bold:true
        }
        ChatButton {id:closeButton;theme:root.theme;text:"Close";iconName:"close";iconOnly:true;forceIcon:true;quiet:true;onClicked:root.close()}
      }
      Label {
        text:root.classic ? (root.status.identity_ready
          ? "Confirm your current password once to use this account on your other devices. Your password, encryption keys and chats stay the same."
          : "Finish this update on a device that already has your chat encryption keys, then sign in here again.")
          : root.status.pending ? "An account action was interrupted. Continue it to finish safely."
          : root.status.installation_pending ? "Your account is ready. Finish setting up this device."
          : root.status.repair_needed ? "Confirm your current password on a trusted device to restore encrypted account access."
          : "Enter your current password to unlock this device."
      }
      ErrorBanner {width:parent.width;theme:root.theme;message:root.setup.error;onDismissed:root.setup.error=""}
      TextField {
        id:password; objectName:"accountSetupPassword"; width:parent.width
        visible:root.needsPassword; enabled:!root.setup.busy; maximumLength:1024; echoMode:TextInput.Password
        placeholderText:"Current password"; Accessible.name:placeholderText; selectByMouse:true
        onAccepted:if(primary.enabled)primary.clicked()
        ThemeControlStyle {theme:root.theme;control:password}
      }
      ChatButton {
        id:primary; objectName:"accountSetupContinue"; theme:root.theme
        text:root.setup.busy ? "Working…" : "Continue"
        visible:!root.classic || !!root.status.identity_ready
        enabled:!root.setup.busy && (!root.needsPassword || !!root.status.pending || !!password.text)
        onClicked: {
          if(root.status.pending) root.submit("backup_resume",{password:password.text})
          else if(root.status.installation_pending) root.submit("backup_finish_install")
          else if(root.classic) root.submit("backup_upgrade",{current_password:password.text})
          else root.submit(root.status.repair_needed && root.status.unlocked ? "backup_repair" : "backup_unlock",{password:password.text})
        }
      }
      SettingsSection {
        id:recovery; width:parent.width; theme:root.theme; title:"Recovery options"; summary:"If Continue cannot finish"; expanded:false
        visible:!!root.status.pending
        Label {text:"Use the password for the interrupted action. For an older update that changed your password, use that new password if it completed, or the original password if it did not."}
        ChatButton {theme:root.theme;text:"Recover secure sign-in";enabled:!root.setup.busy && !!password.text;onClicked:root.submit("backup_recover_secure",{password:password.text})}
        ChatButton {theme:root.theme;text:"Recover original sign-in";visible:root.status.pending==="migration";enabled:!root.setup.busy && !!password.text;onClicked:root.submit("backup_recover_classic",{password:password.text})}
        ChatButton {theme:root.theme;text:"Cancel prepared action";visible:!!root.status.can_cancel;enabled:!root.setup.busy;onClicked:root.submit("backup_cancel")}
      }
      ChatButton {theme:root.theme;text:"Later";quiet:true;onClicked:root.close()}
    }
  }
}
