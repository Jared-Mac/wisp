# WISP — extended waveform

The waveform extends four blocks (40 px) past each end of the text. Eight new
anchored columns taper gently toward the outside. The existing 76 columns,
lettering, palette, timing, and two-block resting height are preserved.

- `wisp.gif`: 960 × 400 animated loop, approximately six seconds.
- `wisp-resting.png`: resting state with the new side margins.
- `wisp-poster.png` / `wisp-poster.svg`: active state on the dark background.
- `wisp.png` / `wisp.svg`: transparent active-state artwork.
- `generation.json`: exact settings and provenance.

Regenerate with `python3 output/wisp-extended/generate.py`. Requires Python 3
(standard library only), TTFX, ImageMagick, and FFmpeg. The terminal example
requires 336 columns × 36 rows to include the added margins.
The large TTFX frame capture and intermediate envelope files are generated
locally and excluded from version control.

This retains actual [TTFX](https://github.com/omacom/ttfx) `colorshift --no-travel`
frames. TTFX is an MIT-licensed port of ChrisBuilds'
[TerminalTextEffects](https://github.com/ChrisBuilds/terminaltexteffects).
See [TTFX's NOTICE](https://github.com/omacom/ttfx/blob/master/NOTICE).
No third-party code or binaries are bundled. Prior artwork and installed app
files are preserved.
