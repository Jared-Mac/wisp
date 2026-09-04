import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property string presence
  required property var theme
  readonly property string label: presence === "open" ? "Open" : presence === "knock" ? "Knock" : presence === "closed" ? "Closed" : "Away"
  readonly property color iconColor: presence === "open" ? theme.onlineIndicator : presence === "knock" ? theme.warning : presence === "closed" ? theme.danger : theme.muted
  // Lucide door-open, bell-dot, lock-keyhole and moon. See assets/PRESENCE-ICONS-LICENSE.txt.
  // Inline SVG lets Qt render crisp vectors in the active palette, including
  // host-provided colors, without a GPU-only tint effect or font glyph fallback.
  readonly property var shapes: ({
    open: '<path d="M10 21H2 M10 4a2 2 0 012.36-1.968l5.41.992A1.5 1.5 0 0119 4.5V21l-7.876.992A1 1 0 0110 21z M10.268 3H7a2 2 0 00-2 2v16 M14 12h.01 M22 21h-3"/>',
    knock: '<path d="M10.268 21a2 2 0 0 0 3.464 0 M11.68 2.009A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673c-.824-.85-1.678-1.731-2.21-3.348"/><circle cx="18" cy="5" r="3"/>',
    closed: '<circle cx="12" cy="16" r="1"/><rect x="3" y="10" width="18" height="12" rx="2"/><path d="M7 10V7a5 5 0 0 1 10 0v3"/>',
    away: '<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>'
  })
  implicitWidth: theme.space(16); implicitHeight: theme.space(16)
  readonly property int imageStatus: vector.status
  Image {
    id: vector
    anchors.fill: parent
    sourceSize: Qt.size(width, height)
    fillMode: Image.PreserveAspectFit
    source: "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="' + root.iconColor + '" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">' + (root.shapes[root.presence] || root.shapes.away) + '</svg>')
  }
  Accessible.role: Accessible.StaticText
  Accessible.name: label
  HoverHandler { id: hover }
  ToolTip.visible: hover.hovered
  ToolTip.text: label
}
