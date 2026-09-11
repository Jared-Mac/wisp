import QtQuick
import QtQuick.Controls

Button {
  id: root
  objectName: "identityMenuButton"
  required property var bridge
  required property var theme
  required property url logoSource
  property bool showWordmark: false
  property bool compact: false
  property real maximumWidth: 300
  property bool homeAvailable: false
  property bool showLayout: false
  signal layoutRequested()
  readonly property bool useWordmark: showWordmark
  signal settingsRequested()
  signal newRoomRequested()
  signal homeRequested()
  function closeMenu() { menu.close() }
  implicitHeight: theme.space(compact ? 32 : theme.comfortable ? 54 : 42)
  implicitWidth: compact ? theme.space(32) : root.theme.comfortable ? Math.min(maximumWidth, root.theme.space(300)) : Math.min(maximumWidth,
    Math.max(useWordmark ? wordmark.implicitWidth : titleText.implicitWidth,
             statusText.implicitWidth + theme.space(12))
      + (logo.visible ? theme.space(66) : theme.space(34)))
  padding: compact || useWordmark ? theme.space(2) : theme.spacing.sm
  Accessible.name: "Wisp account menu for " + String(bridge.selfState.display_name || bridge.configuredProfile || "your profile")
  ToolTip.visible: compact && (hovered || visualFocus)
  ToolTip.text: Accessible.name + " · " + bridge.selfStatusLabel
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
    Item {
      id: logo
      width: visible ? root.theme.space(root.compact ? 28 : 30) : 0; height: width
      visible: root.compact || !root.theme.tui && !root.useWordmark
      anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
      Image { anchors.fill: parent; source: root.logoSource; fillMode: Image.PreserveAspectFit }
    }
    Column {
      visible: !root.compact
      anchors.left: logo.visible ? logo.right : parent.left
      anchors.leftMargin: logo.visible ? root.theme.spacing.md : 0
      anchors.right: arrow.left; anchors.rightMargin: root.theme.spacing.md
      anchors.verticalCenter: parent.verticalCenter
      Item {
        width: parent.width
        height: root.useWordmark ? wordmark.implicitHeight : titleText.implicitHeight
        Item {
          id: wordmark
          objectName: "wispWordmark"
          implicitHeight: root.theme.space(root.theme.comfortable ? 32 : 26)
          implicitWidth: Math.round(implicitHeight * 4.687150837988827)
          visible: root.useWordmark
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          width: Math.min(implicitWidth, parent.width)
          height: implicitHeight
          Image {
            anchors.fill: parent
            source: Qt.resolvedUrl("../assets/wisp-wordmark.svg")
            fillMode: Image.PreserveAspectFit
            sourceSize: Qt.size(Math.ceil(width * 2), Math.ceil(height * 2))
          }
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
    WispIcon { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; theme: root.theme; name: "chevron"; visible: root.theme.friendly && !root.compact }
    Text {
      id: arrow
      visible: !root.compact
      opacity: root.theme.friendly ? 0 : 1
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
    id: styledControl1
      ThemeControlStyle { theme: root.theme; control: styledControl1 }
      objectName: "identityHome"
      visible: root.homeAvailable
      height: visible ? implicitHeight : 0
      text: root.theme.friendly ? "Home" : "[home]"
      onTriggered: root.homeRequested()
    }
    MenuItem {
    id: styledControl2
      ThemeControlStyle { theme: root.theme; control: styledControl2 }
      objectName: "identitySettings"
      text: "Settings"; onTriggered: root.settingsRequested()
    }
    MenuItem {
    id: styledControl3
      ThemeControlStyle { theme: root.theme; control: styledControl3 }
      objectName: "identityNewRoom"
      text: "New Room"; onTriggered: root.newRoomRequested()
    }
    MenuItem {
      visible: root.showLayout; height: visible ? implicitHeight : 0
      text: "Workspace layout…"; onTriggered: root.layoutRequested()
    }

  }
}
