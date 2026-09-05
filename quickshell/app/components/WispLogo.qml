import QtQuick

// Themeable cell-art wordmark. The letter mask and waveform share one grid so
// the signal is part of the logo rather than an image placed over it.
Canvas {
  id: root
  objectName: "wispWordmark"

  required property var theme
  property color letterColor: theme.foreground
  property color waveStartColor: theme.secondaryAccent
  property color waveMiddleColor: theme.accent
  property color waveEndColor: theme.onlineIndicator
  property bool waveformVisible: true
  property real cellGapRatio: 0.14

  readonly property var glyphs: [
    ["##.....##", "##.....##", "##.....##", "##.....##", "##.....##", "##..#..##", "##.###.##", "####.####", ".##...##."],
    ["#######", "#######", "..###..", "..###..", "..###..", "..###..", "..###..", "#######", "#######"],
    [".######.", "########", "###.....", "###.....", ".######.", ".....###", ".....###", "########", ".######."],
    ["#######.", "########", "##....##", "##....##", "########", "#######.", "##......", "##......", "##......"]
  ]
  readonly property int glyphGap: 2
  readonly property int horizontalPaddingCells: 1
  readonly property int gridRows: 9
  readonly property int wordColumns: {
    var columns = 0
    for (var i = 0; i < glyphs.length; i++) columns += glyphs[i][0].length
    return columns + glyphGap * (glyphs.length - 1)
  }
  readonly property int gridColumns: wordColumns + horizontalPaddingCells * 2
  readonly property int waveCenterRow: Math.floor(gridRows / 2)

  // One amplitude per horizontal cell, including a little breathing room at
  // both ends. Each value expands equally above and below the center line.
  // Center the I's peak on its stem. A tall peak along its left edge fills
  // the counterspace and makes the combined silhouette read as an E.
  property var waveformLevels: [
    0, 1, 0, 2, 1, 0, 1, 2, 1, 0,
    1, 0, 0, 0, 2, 3, 2, 0, 0, 0,
    2, 1, 3, 2, 1, 0, 0, 0, 0, 0,
    2, 3, 1, 0, 1, 2, 1, 0, 1, 0
  ]

  implicitHeight: theme.space(26)
  implicitWidth: Math.round(implicitHeight * gridColumns / gridRows)
  antialiasing: false

  function letterAt(column, row) {
    var localColumn = column - horizontalPaddingCells
    if (localColumn < 0 || localColumn >= wordColumns || row < 0 || row >= gridRows)
      return false
    for (var glyphIndex = 0; glyphIndex < glyphs.length; glyphIndex++) {
      var glyph = glyphs[glyphIndex]
      var glyphWidth = glyph[0].length
      if (localColumn < glyphWidth) return glyph[row][localColumn] === "#"
      localColumn -= glyphWidth
      if (glyphIndex < glyphs.length - 1) {
        if (localColumn < glyphGap) return false
        localColumn -= glyphGap
      }
    }
    return false
  }

  function waveAt(column, row) {
    if (!waveformVisible || column < 0 || column >= waveformLevels.length) return false
    var amplitude = Math.max(0, Math.min(waveCenterRow, Number(waveformLevels[column]) || 0))
    return Math.abs(row - waveCenterRow) <= amplitude
  }

  readonly property int letterCellCount: {
    var count = 0
    for (var row = 0; row < gridRows; row++)
      for (var column = 0; column < gridColumns; column++)
        if (letterAt(column, row)) count++
    return count
  }
  readonly property int waveformCellCount: {
    var count = 0
    for (var row = 0; row < gridRows; row++)
      for (var column = 0; column < gridColumns; column++)
        if (waveAt(column, row)) count++
    return count
  }

  function repaint() { requestPaint() }
  onWidthChanged: repaint()
  onHeightChanged: repaint()
  onLetterColorChanged: repaint()
  onWaveStartColorChanged: repaint()
  onWaveMiddleColorChanged: repaint()
  onWaveEndColorChanged: repaint()
  onWaveformVisibleChanged: repaint()
  onWaveformLevelsChanged: repaint()
  onCellGapRatioChanged: repaint()
  Component.onCompleted: repaint()

  onPaint: {
    var context = getContext("2d")
    context.clearRect(0, 0, width, height)

    var cellSize = Math.min(width / gridColumns, height / gridRows)
    if (cellSize <= 0) return
    var drawingWidth = cellSize * gridColumns
    var drawingHeight = cellSize * gridRows
    var originX = Math.round((width - drawingWidth) / 2)
    var originY = Math.round((height - drawingHeight) / 2)
    var gap = Math.max(0, Math.min(cellSize * 0.42, cellSize * cellGapRatio))
    var inset = gap / 2
    var tileSize = Math.max(0.5, cellSize - gap)

    context.fillStyle = String(letterColor)
    for (var row = 0; row < gridRows; row++) {
      for (var column = 0; column < gridColumns; column++) {
        if (!letterAt(column, row)) continue
        context.fillRect(originX + column * cellSize + inset,
                         originY + row * cellSize + inset,
                         tileSize, tileSize)
      }
    }

    if (!waveformVisible) return
    var waveGradient = context.createLinearGradient(originX, 0, originX + drawingWidth, 0)
    waveGradient.addColorStop(0, String(waveStartColor))
    waveGradient.addColorStop(0.52, String(waveMiddleColor))
    waveGradient.addColorStop(1, String(waveEndColor))
    context.fillStyle = waveGradient
    for (var waveRow = 0; waveRow < gridRows; waveRow++) {
      for (var waveColumn = 0; waveColumn < gridColumns; waveColumn++) {
        if (!waveAt(waveColumn, waveRow)) continue
        context.fillRect(originX + waveColumn * cellSize + inset,
                         originY + waveRow * cellSize + inset,
                         tileSize, tileSize)
      }
    }
  }
}
