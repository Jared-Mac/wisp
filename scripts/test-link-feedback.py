"""Real shared QML: browser feedback, duplicate clicks, retry and safe links."""
import os
from pathlib import Path
import sys
import time

os.environ.setdefault('QT_QPA_PLATFORM', 'offscreen')
from PySide6.QtCore import QUrl
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

ROOT = Path(__file__).resolve().parents[1]
app = QGuiApplication([])
engine = QQmlApplicationEngine()
source = '''import QtQuick
import QtQuick.Controls
import "COMPONENTS" as Wisp
Item {
  width: 240; height: 300
  property int launches: 0
  property bool succeeds: true
  property alias feedback: notice.message
  property alias toastWidth: notice.width
  property alias toastHeight: notice.height
  QtObject {
    id: style
    property int cornerRadius: 6
    property color surface: "#222"
    property color accent: "#8ac"
    property color foreground: "white"
    property var font: ({family: "sans-serif", caption: 12})
    function space(value) { return value }
  }
  Wisp.LinkFeedback {
    id: notice; theme: style
    launchUrl: function(url) { launches++; return succeeds }
  }
  function click(url) { notice.open(url) }
  function age() { notice.lastOpened = 0 }
}
'''.replace('COMPONENTS', QUrl.fromLocalFile(str(ROOT/'quickshell/app/components')).toString())
warnings = []
engine.warnings.connect(lambda errors: warnings.extend(e.toString() for e in errors))
engine.loadData(source.encode(), QUrl.fromLocalFile(str(ROOT/'windows/tests/link-fixture.qml')))
assert engine.rootObjects(), warnings
root = engine.rootObjects()[0]
root.click('https://example.com/first')
assert root.property('feedback') == 'Opened in browser'
root.click('https://example.com/first'); root.click('https://example.com/first')
assert root.property('launches') == 1, 'Repeat clicks opened duplicate browser tabs'
root.click('https://example.com/second')
assert root.property('launches') == 2, 'Different links must still open immediately'
root.age(); root.click('https://example.com/second')
assert root.property('launches') == 3, 'Intentional later reopening was blocked'
root.click('javascript:alert(1)'); root.click('file:///secret')
assert root.property('launches') == 3
root.setProperty('succeeds', False)
root.click('https://example.com/failure')
assert 'Could not open browser' in root.property('feedback')
root.setProperty('succeeds', True); root.click('https://example.com/failure')
assert root.property('launches') == 5 and root.property('feedback') == 'Opened in browser'
assert 0 < root.property('toastWidth') <= 240 and root.property('toastHeight') > 16
deadline = time.monotonic() + 3
while root.property('feedback') and time.monotonic() < deadline:
    app.processEvents(); time.sleep(.01)
assert not root.property('feedback'), 'Feedback did not dismiss itself'
assert not warnings, warnings
print('PASS shared link feedback, duplicate suppression, failures/retry and dismissal')
