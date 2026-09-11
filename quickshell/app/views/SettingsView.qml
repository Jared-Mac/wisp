import QtQuick
import QtQuick.Controls
import "../components"
import "../PresenceText.js" as PresenceText

Column {
  id: root
  required property var bridge
  required property var theme
  property bool showActivityToggle: false
  property bool activityStacked: false
  property bool navigationDrawer: false
  property bool navigationOpen: false
  signal navigationRequested()
  property bool showAddChat: false
  property bool canAddChat: false
  property bool showAudioControls: true
  property bool compactPresence: false
  property bool audioFallback: false
  signal addChatRequested(string conversationId)
  function closeMenus() { presenceMenu.close(); addChatPicker.close() }
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  Flow {
    width: parent.width
    spacing: root.theme.spacing.sm
    ActivityToggle {
      visible: root.showActivityToggle && !root.navigationDrawer
      bridge: root.bridge; theme: root.theme; stacked: root.activityStacked
    }
    ChatButton {
      objectName: "navigationDrawerButton"
      visible: root.showActivityToggle && root.navigationDrawer
      theme: root.theme; text: root.navigationOpen ? "Back to chat" : "Rooms & friends"
      Accessible.name: text
      onClicked: root.navigationRequested()
    }
    ChatButton {
      id: presenceButton
      objectName: root.theme.friendly ? "availabilityPicker" : "presenceMenuButton"
      visible: root.compactPresence || root.theme.comfortable || root.theme.refinedTui || root.theme.friendly
      theme: root.theme
      text: "Presence: " + String(root.bridge.selfState.presence || "away") + " ▾"
      Accessible.name: "Who may join: " + String(root.bridge.selfState.presence || "away")
      ToolTip.visible: hovered; ToolTip.text: PresenceText.description(root.bridge.selfState.presence || "away", true)
      onClicked: presenceMenu.open()
      Menu {
        id: presenceMenu; objectName: "presenceMenu"
        y: presenceButton.height
        width: root.theme.space(290)
        ThemeControlStyle { theme: root.theme; control: presenceMenu; outline: true; menuOutline: true }
        Repeater {
          model: [{key:"open",label:"Open · friends can join"}, {key:"knock",label:"Knock · ask before joining"}, {key:"closed",label:"Closed · no joins or knocks"}, {key:"away",label:"Away"}]
          MenuItem {
            id: presenceChoice
            required property var modelData
            objectName: "presenceChoice-" + modelData.key
            text: modelData.label
            font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
            checkable: true; checked: root.bridge.selfState.presence === modelData.key
            onTriggered: root.bridge.setPresence(modelData.key)
            ThemeControlStyle { theme: root.theme; control: presenceChoice }
          }
        }
      }
    }
    Repeater {
      model: root.compactPresence || root.theme.comfortable || root.theme.refinedTui || root.theme.friendly ? [] : ["open", "knock", "closed", "away"]
      delegate: Rectangle {
        objectName: "presence-" + modelData
        required property string modelData
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: "Who may join: " + modelData
        Accessible.description: PresenceText.description(modelData, true)
        ToolTip.visible: presenceMouse.containsMouse || activeFocus
        ToolTip.delay: 500
        ToolTip.text: Accessible.description
        Keys.onSpacePressed: root.bridge.setPresence(modelData)
        Keys.onReturnPressed: root.bridge.setPresence(modelData)
        width: presenceText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: root.theme.tui ? (root.bridge.selfState.presence === modelData ? root.theme.selectionBackground : "transparent") : root.bridge.selfState.presence === modelData
          ? root.theme.alpha(root.theme.accent, 0.38)
          : root.theme.alpha(root.theme.foreground, presenceMouse.containsMouse ? 0.12 : 0.055)
        border.width: root.theme.tui ? (activeFocus ? 1 : 0) : root.theme.terminal || activeFocus ? 1 : 0
        border.color: activeFocus || root.bridge.selfState.presence === modelData ? root.theme.accent : root.theme.separator
        Text {
          id: presenceText
          anchors.centerIn: parent
          text: root.theme.tui ? "[" + modelData + "]" : modelData.charAt(0).toUpperCase() + modelData.slice(1)
          color: root.theme.tui && root.bridge.selfState.presence === modelData ? root.theme.selectionText : root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: presenceMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.bridge.setPresence(modelData)
        }
      }
    }
    Item { width: root.theme.spacing.sm; height: root.theme.space(30) }
    Row {
      spacing: root.theme.spacing.sm
      ChatButton {
        id: addChatButton
        objectName: "headerAddChatButton"
        visible: root.showAddChat; enabled: root.canAddChat
        theme: root.theme; text: root.theme.comfortable ? "+ Chat" : "+ add chat"; height: root.theme.space(root.theme.comfortable ? 36 : 32)
        Accessible.name: "Add chat tile"
        ToolTip.visible: hovered; ToolTip.text: enabled ? "Add a chat tile" : "Up to eight chat tiles"
        onClicked: addChatPicker.open()
        ConversationPicker {
          id: addChatPicker; objectName: "headerAddChatPicker"
          bridge: root.bridge; theme: root.theme
          width: Math.min(root.theme.space(380), root.Window.window ? root.Window.window.width-root.theme.space(24) : root.width)
          x: root.Window.window ? Math.min(0, root.Window.window.width-addChatButton.mapToItem(root.Window.window.contentItem,0,0).x-width-root.theme.space(12)) : 0
          y: addChatButton.height
          onChosen: function(id) { root.addChatRequested(id) }
          onNewChatRequested: newChatDialog.begin()
        }
      }
      AudioStateIndicator {
        objectName: "globalAudioControls"
        visible: root.showAudioControls && (root.audioFallback || !root.theme.friendly || !root.bridge.currentVoiceRoom)
        bridge: root.bridge; theme: root.theme
        muted: !!root.bridge.selfState.muted || !!root.bridge.selfState.deafened
        deafened: !!root.bridge.selfState.deafened
      }
    }
  }
  NewChatDialog {
    id: newChatDialog; objectName: "headerNewChatDialog"
    bridge: root.bridge; theme: root.theme
    onCreated: function(id) { root.addChatRequested(id) }
  }
}
