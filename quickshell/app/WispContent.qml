import QtQuick
import QtQuick.Controls
import QtQuick.Window
import "components"
import "views"

FocusScope {
  id: root
  objectName: "wispContent"
  enabled: !bridge.updates.preparing

  required property var bridge
  required property var theme
  required property url logoSource
  property string presentation: "panel"
  readonly property bool trayChatFocused: presentation === "panel" && bridge.workspaceLayout.trayChatFocused
  property bool horizontalPanel: false
  readonly property bool landscapePanel: horizontalPanel && presentation === "panel" && width >= theme.space(680)
  property var anchorController: null
  property int contentPadding: trayChatFocused ? theme.space(8) : presentation === "panel" ? theme.space(12) : theme.comfortable ? theme.space(18) : theme.cleanTui ? theme.space(14) : theme.tui ? theme.space(10) : theme.spacing.huge
  property bool dismissOnNavigate: false
  property bool showAppButton: false
  property string appButtonText: "Open app"
  property bool showCloseButton: false
  // Top-level pages share one route home. Non-home pages expose it in both the
  // header and the identity menu without duplicating navigation state.
  property string currentPage: "chats"
  readonly property bool settingsOpen: currentPage === "settings"
  property string settingsSection: "media"
  readonly property bool showingChats: currentPage === "chats"
  onCurrentPageChanged: if (savedStatus) savedStatus.clear()
  property bool localPreviewsPoppedOut: false
  readonly property bool wideLayout: presentation === "app"
    && width - contentPadding * 2 >= theme.space(600)
    && ["top", "bottom"].indexOf(bridge.workspaceLayout.dock) < 0
  readonly property int contentWidthLimit: 0
  readonly property bool inlineHeader: trayChatFocused || landscapePanel || presentation === "app" && width - contentPadding * 2 >= theme.space(theme.comfortable ? 1100 : 740)

  signal closeRequested()
  signal appRequested()
  signal popOutLocalPreviewsRequested()

  implicitWidth: presentation === "app"
    ? theme.space(960) : theme.space(horizontalPanel ? 960 : 390) + contentPadding * 2
  implicitHeight: presentation === "app"
    ? theme.space(840)
    : landscapePanel ? theme.space(560) : trayChatFocused ? theme.space(800) : Math.min(theme.space(800), fixedHeader.height + panelColumn.implicitHeight + contentPadding * 2 + theme.spacing.lg + trayAudioDock.height)
  focus: true

  AccountSetupDialog {
    id:accountSetupDialog; setup:root.bridge.accountSetup; theme:root.theme
    surfaceActive:root.visible && !!root.Window.window && root.Window.window.visible && root.Window.window.active
      && !root.bridge.delegateConversationsToDesktop
  }

  function maybeDismiss() {
    if (dismissOnNavigate) requestClose()
  }

  function requestClose() {
    resetNavigation()
    closeRequested()
  }

  function resetNavigation() {
    accountSetupDialog.close()
    cameraConfirmation.close()
    accountMenu.closeMenu()
    accessControls.closeMenus()
    layoutMenu.close()
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
    root.settingsSection = "server"
    if (settingsLoader.item) settingsLoader.item.section = "server"
    if (!settingsOpen) toggleSettings()
    else bridge.refreshServerSettings()
  }

  // Editors consume typing/paste before the single-letter call shortcuts.
  Keys.priority: Keys.AfterItem
  Keys.onPressed: function(event) { root.handleWindowKey(event) }
  function handleWindowKey(event) {
    if (cameraConfirmation.visible || accountSetupDialog.visible) return
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
    spacing: root.presentation === "panel" ? root.theme.space(6) : root.theme.spacing.lg

    Item {
      id: topBar
      width: parent.width
      height: Math.max(root.theme.space(root.trayChatFocused ? 32 : root.theme.comfortable ? 60 : 42), root.inlineHeader ? accessControls.implicitHeight : 0)

      IdentityMenu {
        id: accountMenu
        anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
        compact:root.trayChatFocused
        maximumWidth: root.trayChatFocused ? root.theme.space(230) : root.landscapePanel ? Math.min(root.theme.space(260), root.width*0.32) : root.inlineHeader ? Math.min(root.theme.space(root.theme.comfortable ? 320 : 220), Math.max(0, headerActions.x - root.theme.space(root.theme.comfortable ? 540 : 520) - root.theme.spacing.lg * 2)) : Math.max(0, headerActions.x - root.theme.spacing.lg)
        bridge: root.bridge; theme: root.theme; logoSource: root.logoSource
        showWordmark: root.presentation === "app"
        homeAvailable: !root.showingChats
        showLayout: root.presentation === "app" && root.showingChats && root.theme.comfortable && root.width < root.theme.space(600)
        onLayoutRequested: layoutMenu.open()
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
    id: styledControl1
      ThemeControlStyle { theme: root.theme; control: styledControl1 }
            required property var modelData
            text: modelData.label; checkable: true; checked: root.bridge.workspaceLayout.dock === modelData.key
            onTriggered: root.bridge.workspaceLayout.dock = modelData.key
          }
        }
        MenuSeparator {}
        MenuItem {
    id: styledControl2
      ThemeControlStyle { theme: root.theme; control: styledControl2 } text: "Reset layout"; onTriggered: root.bridge.workspaceLayout.reset() }
      }

      Row {
        id: headerActions
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: root.theme.spacing.sm

        ChatButton {
          objectName: "workspaceLayoutButton"
          visible: root.presentation === "app" && root.showingChats && (!root.theme.comfortable || root.width >= root.theme.space(600))
          theme: root.theme; text: root.theme.comfortable ? "Layout ▾" : root.theme.tui ? "layout" : "Layout"; height: root.theme.space(30)
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

        ChatButton {
          objectName: "headerOpenAppButton"; visible: root.showAppButton
          theme: root.theme; text: root.appButtonText; iconName: "window"
          iconOnly:root.presentation === "panel";forceIcon:iconOnly
          ToolTip.visible:hovered || visualFocus;ToolTip.text:"Open app"
          height: root.theme.space(32); onClicked: root.appRequested()
        }
        ChatButton {
          id: closeButton; objectName: "headerCloseButton"; visible: root.showCloseButton
          theme: root.theme; text: "×"; iconName: "close"; iconOnly: true
          width: root.theme.space(32); height: width
          Accessible.name: "Hide Wisp"; onClicked: root.requestClose()
        }
      }
    }


    SettingsView {
      id: accessControls
      objectName: "alwaysVisibleControls"
      audioFallback: root.presentation === "app"
      compactPresence: root.landscapePanel || root.trayChatFocused
      showAudioControls: !(root.presentation === "panel" && root.showingChats) && !(root.presentation === "app" && root.showingChats && dashboardLoader.item && dashboardLoader.item.audioInSidebar)
      showActivityToggle: root.presentation === "app" && root.showingChats
      activityStacked: !root.wideLayout
      navigationDrawer: !!dashboardLoader.item && !!dashboardLoader.item.drawerMode
      navigationOpen: !!dashboardLoader.item && !!dashboardLoader.item.drawerOpen
      onNavigationRequested: if (dashboardLoader.item) dashboardLoader.item.toggleDrawer()
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
    Flow {
      width: parent.width; spacing: root.theme.spacing.sm
      visible: root.bridge.updates.available || root.bridge.updates.busy
      Text {
        width: Math.min(implicitWidth, parent.width); wrapMode: Text.Wrap
        text: root.bridge.updates.statusText; color: root.theme.accent
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        height: Math.max(implicitHeight, updateBannerButton.height)
        verticalAlignment: Text.AlignVCenter
      }
      ChatButton {
        id: updateBannerButton; theme: root.theme; text: "Update"; iconName: "download"
        visible: !root.bridge.updates.busy
        onClicked: {
          root.settingsSection = "updates"
          if (settingsLoader.item) settingsLoader.item.section = "updates"
          if (!root.settingsOpen) root.toggleSettings()
        }
      }
    }
    Rectangle {
      width: parent.width; height: root.theme.terminal ? 1 : 0
      color: root.theme.separator
    }
  }

  Flickable {
    id: scrollView
    objectName: "dashboardScroll"
    anchors { left: parent.left; right: parent.right; bottom: parent.bottom; top: fixedHeader.bottom; topMargin: root.presentation === "panel" ? root.theme.space(6) : root.theme.spacing.lg }
    anchors.bottomMargin: (terminalStatus.visible ? terminalStatus.height : 0) + (trayAudioDock.active ? trayAudioDock.height+root.contentPadding+root.theme.space(6) : 0)
    contentWidth: width
    contentHeight: panelColumn.implicitHeight + root.contentPadding * 2
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: root.presentation !== "app" && !root.landscapePanel && !root.trayChatFocused || !root.showingChats

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


      ErrorBanner {
        width: parent.width
        theme: root.theme
        message: root.bridge.lastError
        onDismissed: root.bridge.dismissError()
      }

      Row {
        width: parent.width; spacing: root.theme.spacing.sm
        visible: root.bridge.voiceRecovery.statusText !== ""
        Text {
          width: parent.width - (cancelRecovery.visible ? cancelRecovery.width + parent.spacing : 0)
          text: root.bridge.voiceRecovery.statusText; wrapMode: Text.Wrap
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        ChatButton {
          id: cancelRecovery; objectName: "cancelVoiceReconnect"
          visible: root.bridge.voiceRecovery.pending; theme: root.theme; text: "cancel"
          onClicked: root.bridge.voiceRecovery.cancel("", true)
        }
      }

      Loader {
        id: settingsLoader
        objectName: "settingsLoader"
        // Keep visited forms alive so switching pages retains local edits, but
        // don't construct settings in every hidden app/panel surface at startup.
        property bool visited: false
        active: root.settingsOpen || visited
        onLoaded: visited = true
        visible: root.settingsOpen
        width: Math.min(parent.width, root.theme.space(820))
        x: (parent.width-width)/2
        sourceComponent: SettingsMenu {
          section: root.settingsSection
          onSectionChanged: root.settingsSection = section
          width: settingsLoader.width
          bridge: root.bridge; theme: root.theme
          anchorController: root.anchorController
          onRevealSetting: function(item) {
            var position = item.mapToItem(scrollView.contentItem, 0, 0)
            scrollView.contentY = Math.max(0, Math.min(position.y - root.theme.spacing.lg,
              scrollView.contentHeight - scrollView.height))
          }
        }
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
        height: (root.presentation === "app" || root.landscapePanel || root.trayChatFocused) && root.showingChats
          ? Math.max(1, scrollView.height - y - root.contentPadding)
          : implicitHeight
        sourceComponent: root.trayChatFocused ? focusedTrayComponent : root.presentation === "app"
          ? wideDashboardComponent : root.landscapePanel ? barDashboardComponent : compactDashboardComponent
      }
    }
  }

  Loader {
    id:trayAudioDock
    active:root.presentation === "panel" && root.showingChats
    visible:active
    anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom
    anchors.leftMargin:root.contentPadding;anchors.rightMargin:root.contentPadding
    anchors.bottomMargin:root.contentPadding+(terminalStatus.visible ? terminalStatus.height : 0)
    height:active ? implicitHeight : 0
    sourceComponent:TrayAudioControls {
      bridge:root.bridge;theme:root.theme;navigationPeeks:root.trayChatFocused
      onCameraRequested:root.requestCamera()
      onServerSettingsRequested:root.openServerSettings()
      onCreateRoomRequested:identityRoomManager.createRoom()
    }
  }

  Rectangle {
    id: terminalStatus
    objectName: "terminalStatusLine"
    visible: root.theme.tui && !root.theme.comfortable && !root.horizontalPanel && !root.trayChatFocused
    anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
    height: root.theme.space(root.theme.cleanTui ? 22 : 24)
    color: root.theme.statusBackground
    Rectangle {
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      height: 1; color: root.theme.surfaceBorder
    }
    Text {
      id: terminalState
      objectName: "terminalStateText"
      anchors.left: parent.left; anchors.leftMargin: root.theme.space(8)
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, parent.width - root.theme.space(16) - (terminalHelp.visible ? terminalHelp.width + root.theme.space(24) : 0))
      elide: Text.ElideRight
      text: "wisp | " + (root.bridge.daemonConnected ? "connected" : "disconnected")
        + " | " + (root.bridge.selfState.deafened ? "deafened" : "mic:" + (root.bridge.selfState.muted ? "muted" : "unmuted"))
        + (root.presentation === "app" ? " | cam:" + (root.bridge.cameraStarting ? "starting" : root.bridge.cameraActive ? "on" : "off") + " share:" + (root.bridge.shareStarting ? "starting" : root.bridge.sharing ? "on" : "off") : "")
      color: root.theme.statusText
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      HoverHandler { id: statusHover }
      ToolTip.visible: statusHover.hovered && terminalState.truncated
      ToolTip.text: text
    }
    Text {
      id: terminalHelp
      objectName: "terminalHelpText"
      anchors.right: parent.right; anchors.rightMargin: root.theme.space(8)
      anchors.verticalCenter: parent.verticalCenter
      visible: parent.width > root.theme.space(820)
      text: "Tab focus · Shift+M mute · Shift+D deafen · Esc back"
      color: root.theme.refinedTui ? root.theme.muted : root.theme.statusText
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }

  LocalBroadcastPreviews {
    objectName: "localBroadcastPreviews"
    anchors.fill: parent
    anchors.bottomMargin: (terminalStatus.visible ? terminalStatus.height : 0)+(trayAudioDock.active ? trayAudioDock.height+root.contentPadding+root.theme.space(6) : 0)
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
      spacing: root.theme.space(8)

      Column {
      width: parent.width; spacing: root.theme.space(8)
      visible: !people.friendsMode
      ServerSelector {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        compact: true
        showInvite: false
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
      ServerChannelsView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onSelected: root.maybeDismiss()
      }
      }
      PeopleView {
        id: people
        presentation: "panel"
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
    id:focusedTrayComponent
    TrayChatWorkspace {
      bridge:root.bridge;theme:root.theme
    }
  }

  Component {
    id: barDashboardComponent
    BarWorkspace {
      bridge: root.bridge; theme: root.theme
      onCameraRequested: root.requestCamera()
      onServerSettingsRequested: root.openServerSettings()
      onCreateRoomRequested: identityRoomManager.createRoom()
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
  Rectangle {
    z: 100
    anchors.horizontalCenter: parent.horizontalCenter; anchors.bottom: parent.bottom; anchors.bottomMargin: root.theme.spacing.lg
    visible: !!root.bridge.messageActions.feedback
    width: Math.min(parent.width-root.theme.spacing.lg*2, root.theme.space(480))
    height: actionFeedback.implicitHeight+root.theme.spacing.lg*2
    color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.accent
    Text {id:actionFeedback;anchors.fill:parent;anchors.margins:root.theme.spacing.lg;textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.bridge.messageActions.feedback;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
}
