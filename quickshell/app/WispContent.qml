import QtQuick
import "components"
import "views"

FocusScope {
  id: root

  required property var bridge
  required property var theme
  required property url logoSource
  property string presentation: "panel"
  property var anchorController: null
  property int contentPadding: theme.spacing.huge
  property bool dismissOnNavigate: false
  property bool showAppButton: false
  property bool showCloseButton: false
  property bool settingsOpen: false
  readonly property bool wideLayout: presentation === "app"
    && width >= theme.space(760)
  readonly property int contentWidthLimit: presentation === "app"
    ? theme.space(1160) : 0

  signal closeRequested()
  signal appRequested()

  implicitWidth: presentation === "app"
    ? theme.space(960) : theme.space(390) + contentPadding * 2
  implicitHeight: presentation === "app"
    ? theme.space(720)
    : Math.min(theme.space(620), panelColumn.implicitHeight + contentPadding * 2)
  focus: true

  function maybeDismiss() {
    if (dismissOnNavigate) requestClose()
  }

  function requestClose() {
    resetNavigation()
    closeRequested()
  }

  function resetNavigation() {
    settingsOpen = false
    scrollView.contentY = 0
  }

  function toggleSettings() {
    settingsOpen = !settingsOpen
    scrollView.contentY = 0
    if (settingsOpen) bridge.refreshAudioDevices()
  }

  Keys.priority: Keys.BeforeItem
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape) {
      if (root.settingsOpen) root.toggleSettings()
      else root.requestClose()
      event.accepted = true
    } else if (event.text === "m" || event.text === "M") {
      root.bridge.toggleMuted()
      event.accepted = true
    } else if (event.text === "d" || event.text === "D") {
      root.bridge.toggleDeafened()
      event.accepted = true
    } else if (event.text === "v" || event.text === "V") {
      root.bridge.toggleSurface()
      event.accepted = true
    } else if (event.text === "s" || event.text === "S") {
      root.bridge.toggleShare()
      event.accepted = true
    } else if (event.text === "l" || event.text === "L") {
      root.bridge.leave()
      event.accepted = true
    }
  }

  Flickable {
    id: scrollView
    anchors.fill: parent
    contentWidth: width
    contentHeight: panelColumn.implicitHeight + root.contentPadding * 2
    clip: true
    boundsBehavior: Flickable.StopAtBounds

    Column {
      id: panelColumn
      x: Math.max(root.contentPadding,
        Math.round((scrollView.width - width) / 2))
      y: root.contentPadding
      width: {
        var available = Math.max(1, scrollView.width - root.contentPadding * 2)
        return root.contentWidthLimit > 0
          ? Math.min(available, root.contentWidthLimit) : available
      }
      spacing: root.theme.spacing.lg

      Item {
        width: parent.width
        height: root.theme.space(36)

        Row {
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          spacing: root.theme.spacing.md

          Image {
            width: root.theme.space(30)
            height: width
            anchors.verticalCenter: parent.verticalCenter
            source: root.logoSource
            fillMode: Image.PreserveAspectFit
          }

          Column {
            anchors.verticalCenter: parent.verticalCenter

            Text {
              text: "Wisp"
              color: root.theme.foreground
              font.family: root.theme.font.family
              font.pixelSize: root.theme.font.title
              font.weight: Font.DemiBold
            }

            Row {
              spacing: root.theme.spacing.xs

              PresenceDot {
                anchors.verticalCenter: parent.verticalCenter
                presence: root.bridge.daemonConnected
                  ? String(root.bridge.selfState.presence || "away")
                  : "closed"
                theme: root.theme
              }

              Text {
                text: String(root.bridge.selfState.display_name
                    || root.bridge.configuredProfile
                    || "Unknown profile")
                  + " · " + root.bridge.selfStatusLabel
                color: root.bridge.hasError ? root.theme.danger : root.theme.muted
                font.family: root.theme.font.family
                font.pixelSize: root.theme.font.caption
              }
            }
          }
        }

        Row {
          id: headerActions
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          spacing: root.theme.spacing.sm

          Rectangle {
            id: settingsButton
            width: settingsText.implicitWidth + root.theme.spacing.lg * 2
            height: root.theme.space(30)
            radius: root.theme.cornerRadius
            color: settingsMouse.containsMouse
              ? root.theme.alpha(root.theme.foreground, 0.12)
              : root.theme.alpha(root.theme.foreground, 0.055)

            Text {
              id: settingsText
              anchors.centerIn: parent
              text: root.settingsOpen ? "Back" : "Settings"
              color: root.theme.foreground
              font.family: root.theme.font.family
              font.pixelSize: root.theme.font.caption
            }

            MouseArea {
              id: settingsMouse
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.toggleSettings()
            }
          }

          Rectangle {
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
              text: "Open app"
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
        visible: root.settingsOpen
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        anchorController: root.anchorController
      }

      Loader {
        id: dashboardLoader
        visible: !root.settingsOpen
        width: parent.width
        sourceComponent: root.wideLayout
          ? wideDashboardComponent : compactDashboardComponent
      }
    }
  }

  Component {
    id: compactDashboardComponent

    Column {
      id: compactDashboard
      spacing: root.theme.spacing.lg

      SettingsView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
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

      NowView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onJoined: root.maybeDismiss()
      }

      FriendsView {
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onSelected: root.maybeDismiss()
      }

      MediaControls {
        visible: root.bridge.inHangout
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onLeaveRequested: root.maybeDismiss()
      }
    }
  }

  Component {
    id: wideDashboardComponent

    Row {
      id: wideDashboard
      spacing: root.theme.spacing.xxl

      Column {
        id: activityColumn
        width: Math.round((wideDashboard.width - wideDashboard.spacing) * 0.57)
        spacing: root.theme.spacing.lg

        SettingsView {
          width: parent.width
          bridge: root.bridge
          theme: root.theme
        }

        Repeater {
          model: root.bridge.knocks
          delegate: KnockCard {
            required property var modelData
            width: activityColumn.width
            knock: modelData
            bridge: root.bridge
            theme: root.theme
            onAccepted: root.maybeDismiss()
          }
        }

        NowView {
          width: parent.width
          bridge: root.bridge
          theme: root.theme
          onJoined: root.maybeDismiss()
        }
      }

      Column {
        width: wideDashboard.width - activityColumn.width - wideDashboard.spacing
        spacing: root.theme.spacing.lg

        FriendsView {
          width: parent.width
          bridge: root.bridge
          theme: root.theme
          onSelected: root.maybeDismiss()
        }

        MediaControls {
          visible: root.bridge.inHangout
          width: parent.width
          bridge: root.bridge
          theme: root.theme
          onLeaveRequested: root.maybeDismiss()
        }
      }
    }
  }
}
