import QtQuick

Rectangle {
  id: root

  required property var theme
  required property string title
  required property string previewUrl
  required property bool starting
  required property bool active
  required property var viewers
  property bool mirrored: false
  property Item dragTarget: null
  property real dragMinimumX: 0
  property real dragMaximumX: 0
  property real dragMinimumY: 0
  property real dragMaximumY: 0
  property string actionMode: "popout"

  signal actionRequested()

  readonly property string viewerNames: {
    if (!active || viewers.length === 0) return "No one is watching"
    return viewers.join("\n")
  }
  readonly property int previewHeight: Math.round(width * 9 / 16)

  visible: starting || active
  implicitHeight: starting
    ? root.theme.space(42)
    : header.height + previewHeight
  radius: root.theme.cornerRadius
  color: root.theme.surface
  border.width: 1
  border.color: root.theme.alpha(root.theme.accent, 0.42)

  Rectangle {
    id: header
    z: 2
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    height: root.theme.space(32)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.accent, 0.12)

    // Keep the lower corners square against the video surface.
    Rectangle {
      visible: root.active
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      height: root.theme.cornerRadius
      color: parent.color
    }

    Text {
      id: titleText
      anchors.left: parent.left
      anchors.leftMargin: root.theme.spacing.lg
      anchors.right: viewerBadge.left
      anchors.rightMargin: root.theme.spacing.sm
      anchors.verticalCenter: parent.verticalCenter
      text: root.starting ? "Starting " + root.title.toLowerCase() + "…" : root.title
      color: root.theme.foreground
      elide: Text.ElideRight
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.DemiBold
    }

    MouseArea {
      id: dragArea
      anchors.left: parent.left
      anchors.right: viewerBadge.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      enabled: root.dragTarget !== null
      cursorShape: enabled ? Qt.SizeAllCursor : Qt.ArrowCursor
      drag.target: root.dragTarget
      drag.axis: Drag.XAndYAxis
      drag.minimumX: root.dragMinimumX
      drag.maximumX: root.dragMaximumX
      drag.minimumY: root.dragMinimumY
      drag.maximumY: root.dragMaximumY
    }

    Rectangle {
      id: viewerBadge
      anchors.right: actionButton.left
      anchors.rightMargin: root.theme.spacing.sm
      anchors.verticalCenter: parent.verticalCenter
      width: viewerCount.implicitWidth + root.theme.spacing.md * 2
      height: root.theme.space(23)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground,
        viewerMouse.containsMouse ? 0.18 : 0.09)

      Text {
        id: viewerCount
        anchors.centerIn: parent
        text: "◉ " + String(root.active ? root.viewers.length : 0)
        color: root.viewers.length > 0 ? root.theme.foreground : root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
      }

      MouseArea {
        id: viewerMouse
        anchors.fill: parent
        enabled: root.active
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
      }

      Rectangle {
        id: viewerTooltip
        visible: viewerMouse.containsMouse
        z: 20
        anchors.right: parent.right
        anchors.top: parent.bottom
        anchors.topMargin: root.theme.spacing.sm
        width: Math.max(root.theme.space(122), viewerNamesText.implicitWidth
          + root.theme.spacing.lg * 2)
        height: viewerNamesText.implicitHeight + root.theme.spacing.lg * 2
        radius: root.theme.cornerRadius
        color: root.theme.background
        border.width: 1
        border.color: root.theme.alpha(root.theme.foreground, 0.24)

        Text {
          id: viewerNamesText
          anchors.centerIn: parent
          text: root.viewerNames
          color: root.viewers.length > 0 ? root.theme.foreground : root.theme.muted
          horizontalAlignment: Text.AlignHCenter
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }
    }

    Rectangle {
      id: actionButton
      anchors.right: parent.right
      anchors.rightMargin: root.theme.spacing.sm
      anchors.verticalCenter: parent.verticalCenter
      width: root.theme.space(23)
      height: width
      radius: root.theme.cornerRadius
      color: actionMouse.containsMouse
        ? root.theme.alpha(root.theme.accent, 0.32)
        : root.theme.alpha(root.theme.foreground, 0.09)

      Text {
        anchors.centerIn: parent
        text: root.actionMode === "dock" ? "↙" : "↗"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        font.weight: Font.Bold
      }

      MouseArea {
        id: actionMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.actionRequested()
      }
    }
  }

  Rectangle {
    visible: root.active
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: header.bottom
    height: root.previewHeight
    radius: root.theme.cornerRadius
    color: root.theme.alpha("#000000", 0.72)
    clip: true

    Image {
      id: previewImage
      anchors.fill: parent
      source: root.previewUrl
      cache: false
      asynchronous: true
      fillMode: Image.PreserveAspectFit
      mirror: root.mirrored
    }

    Text {
      anchors.centerIn: parent
      visible: previewImage.status !== Image.Ready
      text: "Preparing preview…"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }
}
