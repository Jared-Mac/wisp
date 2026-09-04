import QtQuick
import QtQuick.Controls

Rectangle {
  id: root
  required property var bridge
  required property var theme
  required property var video
  property bool detached: false
  signal popOutRequested()
  signal dockRequested()
  signal closeRequested()
  signal tileDragged(real px, real py)
  signal tileDropped()
  signal tileDragCanceled()
  color: theme.background
  border.width: 1; border.color: theme.accent; radius: theme.cornerRadius
  readonly property string label: video ? String(video.participant) + (video.source === "camera" ? " · Camera" : " · Screen") : "Stream ended"
  Row {
    id: toolbar
    anchors { left: parent.left; top: parent.top; right: parent.right; margins: root.theme.spacing.sm }
    spacing: root.theme.spacing.sm
    ChatButton {
      id: handle
      visible: !root.detached
      theme: root.theme; text: "⠿"; width: root.theme.space(28)
      Accessible.name: "Move stream tile"
      DragHandler {
        id: drag; target: null
        onActiveChanged: { if (!active) root.tileDropped() }
        onCentroidChanged: if (active) { var p=handle.mapToItem(root, centroid.position.x, centroid.position.y); root.tileDragged(p.x,p.y) }
        onCanceled: root.tileDragCanceled()
      }
    }
    Text {
      width: Math.max(0, toolbar.width - (handle.visible ? handle.width + toolbar.spacing : 0) - dock.width - close.width - toolbar.spacing * 2)
      height: dock.height; verticalAlignment: Text.AlignVCenter
      text: root.label; elide: Text.ElideRight
      color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    ChatButton {
      id: dock; theme: root.theme; width: root.theme.space(32)
      text: root.detached ? "⚓" : "↗"
      Accessible.name: root.detached ? "Return to main window" : "Pop out stream"
      ToolTip.visible: hovered; ToolTip.text: Accessible.name
      onClicked: root.detached ? root.dockRequested() : root.popOutRequested()
    }
    ChatButton {
      id: close; theme: root.theme; text: "×"; width: root.theme.space(28)
      Accessible.name: "Stop watching"
      onClicked: root.closeRequested()
    }
  }
  Loader {
    id: frame
    objectName: "remoteVideoRenderer"
    anchors { left: parent.left; right: parent.right; top: toolbar.bottom; bottom: parent.bottom; margins: root.theme.spacing.sm }
    source: "RemoteVideoFrame.qml"
  }
  Binding { target: frame.item; property: "socketPath"; value: root.bridge.videoSocketPath; when: !!frame.item }
  Binding { target: frame.item; property: "participant"; value: root.video ? root.video.participant : ""; when: !!frame.item }
  Binding { target: frame.item; property: "source"; value: root.video ? root.video.source : ""; when: !!frame.item }
  Text {
    anchors.centerIn: frame; width: frame.width - root.theme.spacing.lg * 2; wrapMode: Text.Wrap; horizontalAlignment: Text.AlignHCenter
    visible: frame.status === Loader.Error || !frame.item || !frame.item.ready || frame.item.error !== ""
    text: frame.status === Loader.Error ? "Stream renderer unavailable. Reinstall the Wisp UI." : frame.item && frame.item.error ? frame.item.error : "Waiting for stream…"
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
