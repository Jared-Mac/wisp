import QtQuick

Rectangle {
  id: root
  required property var hangout
  required property var bridge
  required property var theme
  signal joined()

  implicitHeight: root.theme.space(72)
  radius: root.theme.cornerRadius
  color: root.theme.alpha(root.theme.foreground, 0.055)
  border.width: 1
  border.color: root.hasActiveMember()
    ? root.theme.alpha(root.theme.accent, 0.65)
    : root.theme.alpha(root.theme.foreground, 0.10)

  function memberSpeaking(name) {
    var active = bridge.activeSpeakers || []
    for (var i = 0; i < active.length; i++)
      if (active[i] === name) return true
    return false
  }

  function hasActiveMember() {
    var members = hangout.members || []
    for (var i = 0; i < members.length; i++)
      if (memberSpeaking(members[i].display_name)) return true
    return false
  }

  function memberNames() {
    var names = []
    var members = hangout.members || []
    for (var i = 0; i < members.length; i++) {
      var name = members[i].display_name
      names.push((memberSpeaking(name) ? "● " : "") + name)
    }
    return names.join(" + ")
  }

  Column {
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.xs

    Text {
      text: root.memberNames()
      color: root.hasActiveMember() ? root.theme.accent : root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.body
      font.weight: Font.DemiBold
    }
    Text {
      text: root.hangout.label || "Hanging out"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Rectangle {
    anchors.right: parent.right
    anchors.rightMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    width: joinText.implicitWidth + root.theme.spacing.lg * 2
    height: root.theme.space(30)
    radius: root.theme.cornerRadius
    color: joinMouse.containsMouse ? Qt.lighter(root.theme.accent, 1.12) : root.theme.accent

    Text {
      id: joinText
      anchors.centerIn: parent
      text: root.bridge.selfState.hangout_id === root.hangout.id ? "HERE" : "JOIN"
      color: "white"
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }
    MouseArea {
      id: joinMouse
      anchors.fill: parent
      hoverEnabled: true
      enabled: root.bridge.selfState.hangout_id !== root.hangout.id
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: {
        root.bridge.joinHangout(root.hangout.id)
        root.joined()
      }
    }
  }
}
