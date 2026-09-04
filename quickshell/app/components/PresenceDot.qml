import QtQuick

Rectangle {
  required property string presence
  required property var theme
  width: 9
  height: 9
  radius: theme.tui ? 0 : 5
  color: presence === "open" ? theme.onlineIndicator
    : presence === "knock" ? "#f5b94c"
    : presence === "away" ? theme.muted
    : "transparent"
  border.width: presence === "closed" ? 1 : 0
  border.color: theme.muted
}
