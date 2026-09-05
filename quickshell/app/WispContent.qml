import QtQuick
import QtQuick.Controls
import "components"
import "views"

FocusScope {
  id: root
  objectName: "wispContent"

  required property var bridge
  required property var theme
  required property url logoSource
  property string presentation: "panel"
  property var anchorController: null
  property int contentPadding: theme.cleanTui ? theme.space(14) : theme.tui ? theme.space(10) : theme.spacing.huge
  property bool dismissOnNavigate: false
  property bool showAppButton: false
  property bool showCloseButton: false
  // Top-level pages share one route home. Non-home pages expose it in both the
  // header and the identity menu without duplicating navigation state.
  property string currentPage: "chats"
  readonly property bool settingsOpen: currentPage === "settings"
  readonly property bool showingChats: currentPage === "chats"
  onCurrentPageChanged: if (savedStatus) savedStatus.clear()
  property bool localPreviewsPoppedOut: false
  readonly property bool wideLayout: presentation === "app"
    && width - contentPadding * 2 >= theme.space(600)
    && ["top", "bottom"].indexOf(bridge.workspaceLayout.dock) < 0
  readonly property int contentWidthLimit: 0
  readonly property bool inlineHeader: presentation === "app" && width - contentPadding * 2 >= theme.space(740)

  signal closeRequested()
  signal appRequested()
  signal popOutLocalPreviewsRequested()

  implicitWidth: presentation === "app"
    ? theme.space(960) : theme.space(390) + contentPadding * 2
  implicitHeight: presentation === "app"
    ? theme.space(840)
    : Math.min(theme.space(800), fixedHeader.height + panelColumn.implicitHeight + contentPadding * 2 + theme.spacing.lg)
  focus: true

  function maybeDismiss() {
    if (dismissOnNavigate) requestClose()
  }

  function requestClose() {
    resetNavigation()
    closeRequested()
  }

  function resetNavigation() {
    cameraConfirmation.close()
    accountMenu.closeMenu()
    currentPage = "chats"
    scrollView.contentY = 0
  }

  function toggleSettings() {
    currentPage = settingsOpen ? "chats" : "settings"
    scrollView.contentY = 0
    if (settingsOpen) {
      bridge.refreshAudioDevices()
      bridge.refreshVideoDevices()
      bridge.refreshDevices()
      bridge.refreshServerSettings()
    }
  }

  function goHome() {
    resetNavigation()
    root.forceActiveFocus()
  }

  function openServerSettings() {
    if (!bridge.canManageServer) return
    settingsMenu.section = "server"
    if (!settingsOpen) toggleSettings()
    else bridge.refreshServerSettings()
  }

  // Editors consume typing/paste before the single-letter call shortcuts.
  Keys.priority: Keys.AfterItem
  Keys.onPressed: function(event) { root.handleWindowKey(event) }
  function handleWindowKey(event) {
    if (cameraConfirmation.visible) return
    if (event.modifiers === Qt.ShiftModifier && (event.key === Qt.Key_M || event.key === Qt.Key_D)) {
      if (!event.isAutoRepeat) {
        if (event.key === Qt.Key_M) root.bridge.toggleMuted()
        else root.bridge.toggleDeafened()
      }
      event.accepted = true
      return
    }
    if (event.modifiers !== Qt.NoModifier && event.key !== Qt.Key_Escape) return
    if (event.key === Qt.Key_Escape) {
      if (!root.showingChats) root.goHome()
      else root.requestClose()
      event.accepted = true
    } else if (event.text === "v" || event.text === "V") {
      root.bridge.toggleSurface()
      event.accepted = true
    } else if (event.text === "s" || event.text === "S") {
      root.bridge.toggleShare()
      event.accepted = true
    } else if (event.text === "c" || event.text === "C") {
      root.requestCamera()
      event.accepted = true
    } else if (event.text === "l" || event.text === "L") {
      root.bridge.leave()
      event.accepted = true
    }
  }

  RoomManager { id: identityRoomManager; objectName: "identityRoomManager"; bridge: root.bridge; theme: root.theme }
  function requestCamera() {
    if (bridge.cameraActive) bridge.stopCamera()
    else if (!bridge.cameraStarting) cameraConfirmation.confirm()
  }
  CameraConfirmation {
    id: cameraConfirmation
    bridge: root.bridge; theme: root.theme
    hostVisible: root.visible && !!root.Window.window && root.Window.window.visible
  }

  SaveStatus {
    id: savedStatus
    theme: root.theme
    anchors.right: parent.right; anchors.bottom: parent.bottom
    anchors.margins: root.contentPadding
    z: 2000
  }
  Rectangle {
    objectName: "knockSentNotice"
    visible: !!root.bridge.knockFeedback
    anchors.right: parent.right; anchors.rightMargin: root.contentPadding
    y: fixedHeader.y + fixedHeader.height + root.theme.spacing.sm
    width: Math.min(knockNoticeText.implicitWidth + root.theme.spacing.lg * 2,
      Math.max(1, root.width - root.contentPadding * 2))
    height: knockNoticeText.contentHeight + root.theme.spacing.md * 2
    radius: root.theme.cornerRadius
    color: root.theme.surface
    border.width: 1; border.color: root.theme.accent
    z: 2000
    Accessible.role: Accessible.AlertMessage
    Accessible.name: root.bridge.knockFeedback || ""
    Text {
      id: knockNoticeText
      x: root.theme.spacing.lg; y: root.theme.spacing.md
      width: parent.width - root.theme.spacing.lg * 2
      text: root.bridge.knockFeedback || ""
      textFormat: Text.PlainText; wrapMode: Text.Wrap
      color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
  Connections {
    target: root.bridge
    function onSettingsSaved() { if (root.settingsOpen) savedStatus.showSaved() }
    function onSettingsSaveFailed() { savedStatus.clear() }
  }
  Connections {
    target: root.theme.appearanceController
    function onSettingsSaved() { if (root.settingsOpen) savedStatus.showSaved() }
    function onSettingsSaveFailed() { savedStatus.clear() }
  }
  Connections {
    target: root.bridge.workspaceLayout
    function onSettingsSaveFailed() { savedStatus.clear(); root.bridge.lastError = root.bridge.workspaceLayout.error }
  }

  Column {
    id: fixedHeader
    z: 20
    x: Math.max(root.contentPadding, Math.round((root.width - width) / 2))
    y: root.contentPadding
    width: root.contentWidthLimit > 0 ? Math.min(root.width - root.contentPadding * 2, root.contentWidthLimit) : root.width - root.contentPadding * 2
    spacing: root.theme.spacing.lg

    Item {
      id: topBar
      width: parent.width
      height: root.theme.space(42)

      IdentityMenu {
        id: accountMenu
        anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
        maximumWidth: root.inlineHeader ? Math.min(root.theme.space(220), Math.max(0, headerActions.x - root.theme.space(520) - root.theme.spacing.lg * 2)) : Math.max(0, headerActions.x - root.theme.spacing.lg)
        bridge: root.bridge; theme: root.theme; logoSource: root.logoSource
        showWordmark: root.presentation === "app"
        homeAvailable: !root.showingChats
        onHomeRequested: root.goHome()
        onSettingsRequested: if (!root.settingsOpen) root.toggleSettings()
        onNewRoomRequested: identityRoomManager.createRoom()
      }

      Menu {
        id: layoutMenu
        x: headerActions.x; y: parent.height
        ThemeControlStyle { theme: root.theme; control: layoutMenu; outline: true; menuOutline: true }
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        Repeater {
          model: [{key:"auto",label:"Automatic layout"},{key:"left",label:"Activity on left"},{key:"right",label:"Activity on right"},{key:"top",label:"Activity above chat"},{key:"bottom",label:"Activity below chat"}]
          MenuItem {
            required property var modelData
            text: modelData.label; checkable: true; checked: root.bridge.workspaceLayout.dock === modelData.key
            onTriggered: root.bridge.workspaceLayout.dock = modelData.key
          }
        }
        MenuSeparator {}
        MenuItem { text: "Reset layout"; onTriggered: root.bridge.workspaceLayout.reset() }
      }

      Row {
        id: headerActions
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: root.theme.spacing.sm

        ChatButton {
          objectName: "workspaceLayoutButton"
          visible: root.presentation === "app" && root.showingChats
          theme: root.theme; text: root.theme.tui ? "layout" : "Layout"; height: root.theme.space(30)
          onClicked: layoutMenu.open()
        }

        ChatButton {
          id: homeButton
          objectName: "headerHomeButton"
          visible: !root.showingChats
          theme: root.theme
          text: root.theme.tui ? "home" : "[home]"
          height: root.theme.space(30)
          onClicked: root.goHome()
        }

        Rectangle {
          objectName: "headerOpenAppButton"
          visible: root.showAppButton
          width: visible ? appButtonText.implicitWidth + root.theme.spacing.lg * 2 : 0
          height: root.theme.space(30)
          radius: root.theme.cornerRadius
          color: appButtonMouse.containsMouse
            ? root.theme.alpha(root.theme.accent, 0.24)
            : root.theme.alpha(root.theme.foreground, 0.055)

          Text {
            id: appButtonText
            anchors.centerIn: parent
            text: root.theme.tui ? "open app" : "Open app"
            color: root.theme.foreground
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.caption
          }

          MouseArea {
            id: appButtonMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.appRequested()
          }
        }

        Rectangle {
          id: closeButton
          visible: root.showCloseButton
          width: visible ? root.theme.space(30) : 0
          height: root.theme.space(30)
          radius: root.theme.cornerRadius
          color: closeMouse.containsMouse
            ? root.theme.alpha(root.theme.foreground, 0.12)
            : root.theme.alpha(root.theme.foreground, 0.055)

          Text {
            anchors.centerIn: parent
            text: "×"
            color: root.theme.foreground
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.title
          }

          MouseArea {
            id: closeMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.requestClose()
          }
        }
      }
    }


    SettingsView {
      id: accessControls
      objectName: "alwaysVisibleControls"
      showActivityToggle: root.presentation === "app" && root.showingChats
      activityStacked: !root.wideLayout
      showAddChat: root.presentation === "app" && root.showingChats
      canAddChat: root.presentation === "app" && !!dashboardLoader.item && dashboardLoader.item.canAddChat
      onAddChatRequested: function(id) { if (dashboardLoader.item) dashboardLoader.item.addChat(id) }
      parent: root.inlineHeader ? topBar : fixedHeader
      x: root.inlineHeader ? accountMenu.width + root.theme.spacing.lg : 0
      Binding on y { when: root.inlineHeader; value: (topBar.height - accessControls.height) / 2; restoreMode: Binding.RestoreBindingOrValue }
      width: root.inlineHeader ? Math.max(1, headerActions.x - x - root.theme.spacing.lg) : parent.width
      bridge: root.bridge
      theme: root.theme
    }
    Rectangle {
      width: parent.width; height: root.theme.terminal ? 1 : 0
      color: root.theme.separator
    }
  }

  Flickable {
    id: scrollView
    objectName: "dashboardScroll"
    anchors { left: parent.left; right: parent.right; bottom: parent.bottom; top: fixedHeader.bottom; topMargin: root.theme.spacing.lg }
    anchors.bottomMargin: terminalStatus.visible ? terminalStatus.height : 0
    contentWidth: width
    contentHeight: panelColumn.implicitHeight + root.contentPadding * 2
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: root.presentation !== "app" || !root.showingChats

    Column {
      id: panelColumn
      x: Math.max(root.contentPadding,
        Math.round((scrollView.width - width) / 2))
      y: 0
      width: {
        var available = Math.max(1, scrollView.width - root.contentPadding * 2)
        return root.contentWidthLimit > 0
          ? Math.min(available, root.contentWidthLimit) : available
      }
      spacing: root.theme.spacing.lg


      Rectangle {
        visible: root.bridge.lastError !== ""
        width: parent.width
        height: errorText.implicitHeight + root.theme.spacing.md * 2
        radius: root.theme.cornerRadius
        color: root.theme.alpha(root.theme.danger, 0.12)

        Text {
          id: errorText
          anchors.fill: parent
          anchors.margins: root.theme.spacing.md
          text: root.bridge.lastError
          color: root.theme.danger
          wrapMode: Text.WordWrap
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }

      SettingsMenu {
        id: settingsMenu
        // Incoming invites remain reachable even when Activity is collapsed.
        visible: root.settingsOpen
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        anchorController: root.anchorController
      }

      Flow {
        width: parent.width; spacing: root.theme.spacing.sm
        visible: (root.bridge.roomInvitations || []).length > 0
        Repeater {
          model: root.bridge.roomInvitations || []
          ChatButton {
            required property var modelData
            theme: root.theme; primary: true
            text: "Voice invite · " + modelData.from.display_name
            width: Math.min(implicitWidth, parent.width)
            onClicked: { root.currentPage = "chats"; root.bridge.selectConversation(root.bridge.scopedConversationId(modelData.server_id,modelData.conversation_id)) }
          }
        }
      }
      Text {
        width: parent.width; wrapMode: Text.Wrap
        visible: !!root.bridge.invitationFeedback
        text: root.bridge.invitationFeedback || ""
        color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }

      Loader {
        id: dashboardLoader
        visible: root.showingChats
        width: parent.width
        height: root.presentation === "app" && root.showingChats
          ? Math.max(1, scrollView.height - y - root.contentPadding)
          : implicitHeight
        sourceComponent: root.presentation === "app"
          ? wideDashboardComponent : compactDashboardComponent
      }
    }
  }

  Rectangle {
    id: terminalStatus
    objectName: "terminalStatusLine"
    visible: root.theme.tui
    anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
    height: root.theme.space(root.theme.cleanTui ? 22 : 24)
    color: root.theme.statusBackground
    Rectangle {
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      height: 1; color: root.theme.surfaceBorder
    }
    Text {
      anchors.fill: parent; anchors.leftMargin: root.theme.space(8); anchors.rightMargin: root.theme.space(8)
      verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
      text: "wisp | " + (root.bridge.daemonConnected ? "connected" : "disconnected")
        + " | mic:" + (root.bridge.selfState.muted || root.bridge.selfState.deafened ? "muted" : "unmuted")
        + (root.presentation === "app" ? " | cam:" + (root.bridge.cameraStarting ? "starting" : root.bridge.cameraActive ? "on" : "off") + " share:" + (root.bridge.shareStarting ? "starting" : root.bridge.sharing ? "on" : "off") + "    Shift+M mute · Shift+D deafen · Esc back" : " | Esc back")
      color: root.theme.statusText
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }

  LocalBroadcastPreviews {
    objectName: "localBroadcastPreviews"
    anchors.fill: parent
    anchors.bottomMargin: terminalStatus.visible ? terminalStatus.height : 0
    z: 100
    bridge: root.bridge
    theme: root.theme
    poppedOut: root.localPreviewsPoppedOut
    onPopOutRequested: root.popOutLocalPreviewsRequested()
  }

  Component {
    id: compactDashboardComponent

    Column {
      id: compactDashboard
      spacing: root.theme.spacing.lg

      ServerSelector {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        compact: true
        onSettingsRequested: root.openServerSettings()
      }

      Repeater {
        model: root.bridge.knocks
        delegate: KnockCard {
          required property var modelData
          width: compactDashboard.width
          knock: modelData
          bridge: root.bridge
          theme: root.theme
          onAccepted: root.maybeDismiss()
        }
      }

      TrayRoomsView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onCreateRoomRequested: identityRoomManager.createRoom()
      }
      CurrentCallBar {
        width: parent.width; height: visible ? implicitHeight : 0
        bridge: root.bridge; theme: root.theme
        onCameraRequested: root.requestCamera()
      }
      ServerChannelsView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onSelected: root.maybeDismiss()
      }
      FriendsView {
        collapsible: true
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onSelected: root.maybeDismiss()
      }

      MessagesView {
        width: parent.width
        availableHeight: Math.max(0, scrollView.height - dashboardLoader.y - y - root.contentPadding * 2)
        bridge: root.bridge
        theme: root.theme
      }

    }
  }

  Component {
    id: wideDashboardComponent

    MainWorkspace {
      bridge: root.bridge
      theme: root.theme
      height: dashboardLoader.height
      onCameraRequested: root.requestCamera()
      onServerSettingsRequested: root.openServerSettings()
      onCreateRoomRequested: identityRoomManager.createRoom()
      onRevealMainRequested: { root.goHome(); root.appRequested() }
    }
  }
}
