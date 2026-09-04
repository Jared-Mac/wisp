import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  readonly property var camera: root.bridge.cameraState
  readonly property var video: root.bridge.videoSettings
  readonly property bool publishing: root.bridge.sharing || root.bridge.cameraActive
  CheckBox {
    id: tilePreference
    width: parent.width
    text: "Open watched streams as tiles when the main window is open"
    checked: root.bridge.workspaceLayout.streamsAsTiles
    onToggled: root.bridge.workspaceLayout.setStreamsAsTiles(checked)
    ThemeControlStyle { theme: root.theme; control: tilePreference }
    contentItem: Text {
      text: tilePreference.text; wrapMode: Text.Wrap
      leftPadding: tilePreference.indicator.width + tilePreference.spacing
      color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }

  Item {
    width: parent.width
    height: Math.max(videoHeading.implicitHeight, refreshButton.height)

    Text {
      id: videoHeading
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      text: "VIDEO"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }

    Rectangle {
      id: refreshButton
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      width: refreshText.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(25)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, refreshMouse.containsMouse ? 0.12 : 0.055)

      Text {
        id: refreshText
        anchors.centerIn: parent
        text: "Refresh"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: refreshMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.refreshVideoDevices()
      }
    }
  }

  Text {
    text: "Camera"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Repeater {
    model: root.camera.devices || []
    delegate: Rectangle {
      required property var modelData
      readonly property bool selected: String(root.camera.selected_device_id || "")
        === String(modelData.id || "")
      width: root.width
      height: root.theme.space(32)
      radius: root.theme.cornerRadius
      color: selected
        ? root.theme.alpha(root.theme.accent, 0.34)
        : root.theme.alpha(root.theme.foreground, deviceMouse.containsMouse ? 0.12 : 0.055)
      opacity: root.camera.active ? 0.58 : 1

      Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: root.theme.spacing.lg
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: String(modelData.name || modelData.id)
        elide: Text.ElideMiddle
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: deviceMouse
        anchors.fill: parent
        enabled: !root.camera.active
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: if (!parent.selected) root.bridge.setCameraDevice(String(modelData.id))
      }
    }
  }

  Text {
    visible: (root.camera.devices || []).length === 0
    text: "No camera detected"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  Text {
    topPadding: root.theme.spacing.sm
    text: "Publishing quality"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Flow {
    width: parent.width
    height: childrenRect.height
    spacing: root.theme.spacing.sm

    Repeater {
      model: [
        { "value": "balanced", "label": "Balanced · 720p30" },
        { "value": "high", "label": "High · 1080p60" },
        { "value": "ultra", "label": "Ultra · 1440p60" }
      ]
      delegate: Rectangle {
        required property var modelData
        readonly property bool selected: root.video.quality === modelData.value
        width: qualityText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: selected
          ? root.theme.alpha(root.theme.accent, 0.34)
          : root.theme.alpha(root.theme.foreground, qualityMouse.containsMouse ? 0.12 : 0.055)
        opacity: root.publishing ? 0.58 : 1

        Text {
          id: qualityText
          anchors.centerIn: parent
          text: modelData.label
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: qualityMouse
          anchors.fill: parent
          enabled: !root.publishing
          hoverEnabled: true
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: root.bridge.setVideoQuality(modelData.value)
        }
      }
    }
  }

  Text {
    topPadding: root.theme.spacing.sm
    text: "Codec"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Flow {
    width: parent.width
    height: childrenRect.height
    spacing: root.theme.spacing.sm

    Repeater {
      model: root.video.available_codecs || ["h264", "vp8", "av1"]
      delegate: Rectangle {
        required property string modelData
        readonly property bool selected: root.video.codec === modelData
        width: codecText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: selected
          ? root.theme.alpha(root.theme.accent, 0.34)
          : root.theme.alpha(root.theme.foreground, codecMouse.containsMouse ? 0.12 : 0.055)
        opacity: root.publishing ? 0.58 : 1

        Text {
          id: codecText
          anchors.centerIn: parent
          text: modelData.toUpperCase()
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: codecMouse
          anchors.fill: parent
          enabled: !root.publishing
          hoverEnabled: true
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: root.bridge.setVideoCodec(modelData)
        }
      }
    }
  }

  Text {
    width: parent.width
    topPadding: root.theme.spacing.sm
    text: root.video.hardware_acceleration
      ? "Hardware encoding: " + String(root.video.encoder_backend || "available")
      : "Hardware encoding unavailable · using software"
    color: root.video.hardware_acceleration ? root.theme.accent : root.theme.muted
    wrapMode: Text.WordWrap
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  Text {
    visible: root.publishing
    width: parent.width
    text: "Stop screen sharing and camera video before changing quality or codec."
    color: root.theme.muted
    wrapMode: Text.WordWrap
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }
}
