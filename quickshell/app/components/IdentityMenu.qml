import QtQuick
import QtQuick.Controls

Button {
  id: root
  objectName: "identityMenuButton"
  required property var bridge
  required property var theme
  required property url logoSource
  property bool showWordmark: false
  property real maximumWidth: 300
  readonly property bool useWordmark: showWordmark
  signal settingsRequested()
  signal newRoomRequested()
  function closeMenu() { menu.close() }
  implicitHeight: theme.space(42)
  implicitWidth: Math.min(maximumWidth,
    Math.max(useWordmark ? wordmark.implicitWidth : titleText.implicitWidth,
             statusText.implicitWidth + theme.space(12))
      + (logo.visible ? theme.space(66) : theme.space(34)))
  padding: useWordmark ? theme.space(2) : theme.spacing.sm
  Accessible.name: "Wisp account menu for " + String(bridge.selfState.display_name || bridge.configuredProfile || "your profile")
  onClicked: menu.opened ? menu.close() : menu.open()
  Keys.onDownPressed: menu.open()
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.down || menu.opened ? root.theme.alpha(root.theme.accent, 0.12)
      : root.hovered ? root.theme.alpha(root.theme.foreground, 0.07) : "transparent"
    border.width: root.visualFocus ? 1 : 0
    border.color: root.theme.focusBorder
  }
  contentItem: Item {
    Image {
      id: logo
      width: visible ? root.theme.space(30) : 0; height: width
      visible: !root.theme.tui && !root.useWordmark
      anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
      source: root.logoSource; fillMode: Image.PreserveAspectFit
    }
    Column {
      anchors.left: logo.visible ? logo.right : parent.left
      anchors.leftMargin: logo.visible ? root.theme.spacing.md : 0
      anchors.right: arrow.left; anchors.rightMargin: root.theme.spacing.md
      anchors.verticalCenter: parent.verticalCenter
      Item {
        width: parent.width
        height: root.useWordmark ? wordmark.implicitHeight : titleText.implicitHeight
        WispLogo {
          id: wordmark
          visible: root.useWordmark
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          width: Math.min(implicitWidth, parent.width)
          height: implicitHeight
          theme: root.theme
        }
        Text {
          id: titleText
          visible: !root.useWordmark
          anchors.left: parent.left; anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: root.theme.tui ? String(root.bridge.selfState.display_name || root.bridge.configuredProfile || "user").toLowerCase() + "@wisp:~" : "Wisp"
          elide: Text.ElideRight
          color: root.theme.tui ? root.theme.accent : root.theme.foreground
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.title; font.weight: Font.DemiBold
        }
      }
      Item {
        width: parent.width; height: statusText.implicitHeight
        PresenceDot {
          id: dot
          anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
          presence: root.bridge.daemonConnected ? String(root.bridge.selfState.presence || "away") : "closed"
          theme: root.theme
        }
        Text {
          id: statusText
          anchors.left: dot.right; anchors.leftMargin: root.theme.spacing.xs; anchors.right: parent.right
          text: root.theme.tui
            ? String(root.bridge.selfState.display_name || root.bridge.configuredProfile || "user").toLowerCase() + " · " + root.bridge.selfStatusLabel.toLowerCase()
            : String(root.bridge.selfState.display_name || root.bridge.configuredProfile || "Unknown profile") + " · " + root.bridge.selfStatusLabel
          elide: Text.ElideRight
          color: root.bridge.hasError ? root.theme.danger : root.theme.muted
          font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
      }
    }
    Text {
      id: arrow
      anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      text: root.theme.tui ? "[≡]" : "▾"; color: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
  Menu {
    id: menu
    objectName: "identityMenu"
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
    y: root.height + root.theme.spacing.sm
    width: Math.min(root.maximumWidth, root.theme.space(230))
    ThemeControlStyle { theme: root.theme; control: menu; outline: true; menuOutline: true }
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    palette.window: root.theme.surface; palette.windowText: root.theme.foreground; palette.text: root.theme.foreground
    MenuItem {
      objectName: "identitySettings"
      text: "Settings"; onTriggered: root.settingsRequested()
    }
    MenuItem {
      objectName: "identityNewRoom"
      text: "New Room"; onTriggered: root.newRoomRequested()
    }
  }
}
