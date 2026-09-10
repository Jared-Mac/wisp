import QtQuick
import QtQuick.Controls
import "../SettingsSearch.js" as Search
import "../components"

Column {
  id: root
  objectName: "settingsMenu"
  property string section: "media"

  required property var bridge
  required property var theme
  property var anchorController: null
  readonly property bool searching: searchField.text.trim() !== ""
  readonly property var searchResults: Search.search(searchField.text, bridge.canManageServer, !!anchorController)
  property string revealedTarget: ""
  signal revealSetting(var item)

  function findSetting(item, name) {
    if (item.objectName === name) return item
    for (var child of item.children || []) {
      var result = findSetting(child, name)
      if (result) return result
    }
    return null
  }
  function openSearchResult(result) {
    if (!result || result.section === "server" && !bridge.canManageServer) return
    section = result.section
    searchField.text = ""
    // Wait for the section's layout before moving the outer settings scroll view.
    Qt.callLater(function() {
      var target = root.findSetting(root, result.target)
      if (target) {
        for (var ancestor=target; ancestor && ancestor!==root; ancestor=ancestor.parent)
          if (typeof ancestor.expandForSearch === "function") ancestor.expandForSearch()
        root.revealedTarget = result.target
        Qt.callLater(function() { target.forceActiveFocus(Qt.TabFocusReason); root.revealSetting(target) })
      }
    })
  }

  Connections {
    target: root.bridge
    function onCanManageServerChanged() {
      if (!root.bridge.canManageServer && root.section === "server") root.section = "media"
    }
  }

  width: parent ? parent.width : 0
  spacing: root.theme.spacing.lg

  Column {
    width: parent.width
    spacing: root.theme.spacing.xs

    Text {
      text: "Settings"
      color: root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.title
      font.weight: Font.DemiBold
    }

    Text {
      width: parent.width
      text: "Changes save automatically unless a Save button is shown."
      color: root.theme.muted
      wrapMode: Text.WordWrap
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Row {
    width: parent.width; spacing: root.theme.spacing.sm
    TextField {
      id: searchField; objectName: "settingsSearch"
      width: parent.width - (clearSearch.visible ? clearSearch.width + parent.spacing : 0)
      placeholderText: "Search settings…"
      Accessible.name: "Search settings"
      selectByMouse: true
      color: root.theme.foreground; placeholderTextColor: root.theme.muted
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      background: Rectangle { color: root.theme.background; border.width: 1; border.color: searchField.activeFocus ? root.theme.accent : root.theme.separator; radius: root.theme.cornerRadius }
      onAccepted: root.openSearchResult(root.searchResults[0])
      Keys.onEscapePressed: text = ""
    }
    ChatButton {
      id: clearSearch; objectName: "clearSettingsSearch"
      theme: root.theme; text: "clear"; visible: searchField.text !== ""
      onClicked: { searchField.text = ""; searchField.forceActiveFocus() }
    }
  }

  Column {
    width: parent.width; spacing: root.theme.spacing.xs
    visible: root.searching
    Text {
      visible: root.searchResults.length === 0
      width: parent.width; wrapMode: Text.Wrap
      text: "No matching settings. Try another word."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Repeater {
      model: root.searchResults
      ChatButton {
        required property var modelData
        objectName: "settingsResult-" + modelData.target
        width: parent.width; theme: root.theme; textAlignment: Text.AlignLeft
        text: modelData.label + " · " + Search.sectionLabel(modelData.section)
        Accessible.name: text
        onClicked: root.openSearchResult(modelData)
        ToolTip.visible: hovered; ToolTip.text: text
      }
    }
  }

  Flow {
    visible: !root.searching
    width: parent.width
    spacing: root.theme.spacing.sm
    Repeater {
      model: {
        var tabs = [{id:"profile",label:"Profile"},{id:"media",label:"Audio"},{id:"video",label:"Video"},{id:"soundboard",label:"Soundboard"},{id:"appearance",label:"Appearance"},{id:"notifications",label:"Notifications"},{id:"privacy",label:"Privacy"},{id:"devices",label:"Devices"},{id:"updates",label:"Updates"}]
        if (root.bridge.canManageServer) tabs.push({id:"server",label:"Server"})
        return tabs
      }
      SettingsTab {
        required property var modelData
        theme: root.theme; text: modelData.label
        iconName: ({updates:"download",soundboard:"volume",profile:"profile",media:"microphone",video:"camera",appearance:"palette",notifications:"bell",privacy:"lock",devices:"screen",server:"settings"})[modelData.id]
        objectName: "settingsTab-" + modelData.id
        primary: root.section === modelData.id
        onClicked: root.section = modelData.id
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "updates"
    width: parent.width
    height: visible ? updateSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius; color: root.theme.background
    border.width: 1; border.color: root.theme.separator
    Loader {
      id: updateSettings; active: parent.visible
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: root.theme.spacing.xxl
      sourceComponent: UpdatesSettingsView { bridge: root.bridge; theme: root.theme; width: updateSettings.width }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "soundboard"
    width: parent.width
    height: visible ? soundboardSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    Loader {
      id: soundboardSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: SoundboardView {
        width: soundboardSettings.width
        bridge: root.bridge; theme: root.theme; serverId: root.bridge.activeServer.id
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "profile"
    width: parent.width
    height: visible ? profileSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    Loader {
      id: profileSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: ProfileSettingsView {
        width: profileSettings.width
        bridge: root.bridge; theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "server" && root.bridge.canManageServer
    width: parent.width
    height: visible ? serverSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    Loader {
      id: serverSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: ServerSettingsView {
        width: serverSettings.width
        bridge: root.bridge; theme: root.theme
      }
    }
  }

  Rectangle {
    // Tab content stays instantiated so changing tabs never resets controls.
    visible: !root.searching && root.section === "appearance"
    width: parent.width
    height: appearanceSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator
    Loader {
      id: appearanceSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: AppearanceSettingsView {
        width: appearanceSettings.width
        theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "privacy"
    width: parent.width
    height: privacySettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: 1; border.color: root.theme.separator
    Loader {
      id: privacySettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: PrivacySettingsView {
        width: privacySettings.width
        bridge: root.bridge; theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "notifications"
    width: parent.width
    height: notificationSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator
    Loader {
      id: notificationSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: NotificationSettingsView {
        width: notificationSettings.width
        bridge: root.bridge
        theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "devices"
    width: parent.width
    height: deviceSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator

    Loader {
      id: deviceSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: DeviceSettingsView {
        width: deviceSettings.width
        bridge: root.bridge
        theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "video"
    width: parent.width
    height: videoSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator

    Loader {
      id: videoSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: VideoSettingsView {
        width: videoSettings.width
        bridge: root.bridge
        theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "media"
    width: parent.width
    height: audioSettings.implicitHeight + root.theme.spacing.xxl * 2
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator

    Loader {
      id: audioSettings
      property bool visited: false
      active: parent.visible || visited
      onLoaded: visited = true
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      sourceComponent: AudioSettingsView {
        width: audioSettings.width
        bridge: root.bridge
        theme: root.theme
      }
    }
  }

  Rectangle {
    visible: !root.searching && root.section === "appearance" && !!root.anchorController
    width: parent.width
    height: visible ? desktopSettings.implicitHeight + root.theme.spacing.xxl * 2 : 0
    radius: root.theme.cornerRadius
    color: root.theme.tui ? root.theme.background : root.theme.alpha(root.theme.foreground, 0.035)
    border.width: root.theme.tui && !root.theme.comfortable ? 1 : 0
    border.color: root.theme.separator

    Column {
      id: desktopSettings
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: root.theme.spacing.xxl
      spacing: root.theme.spacing.lg

      SettingsSection {
        theme: root.theme; title: "Tray position"; summary: "Choose where the tray panel opens"; sectionIcon: "layout"
        objectName: "settingsDesktopPosition"
        WispComboBox {
          id: desktopPosition; theme: root.theme; width: parent.width; textRole: "label"
          Accessible.name: "Tray panel position"
          model: [{value:"auto",label:"Automatic"},{value:"bottom-right",label:"Bottom right"},{value:"bottom-left",label:"Bottom left"},{value:"top-right",label:"Top right"},{value:"top-left",label:"Top left"}]
          currentIndex: {for(var i=0;i<model.length;i++) if(root.anchorController && root.anchorController.anchor===model[i].value)return i;return 0}
          onActivated: root.anchorController.setAnchor(model[currentIndex].value)
        }
        Text {
          width: parent.width; wrapMode: Text.WordWrap
          text: "Automatic follows the tray icon, or uses the bottom-right corner if its position is unavailable."
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
      }

      Text {
        text: root.anchorController && root.anchorController.screen
          ? "Panel display: " + root.anchorController.screen.name
          : "Panel display unavailable"
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }
}
