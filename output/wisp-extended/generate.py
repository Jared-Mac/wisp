#!/usr/bin/env python3
"""Generate Wisp cell art and rasterize real TTFX frames; stdlib + ttfx + ffmpeg + magick."""
import hashlib
import json
import math
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import random
import struct
import wave

OUT = Path(__file__).resolve().parent
COLS = 152
MARGIN_BLOCKS = 4
ROWS = 36
CELL = 5
CENTER = 18
DURATION = 6
SAMPLE_RATE = 12000
WIDTH = 960
HEIGHT = 400
OX = 100
OY = 110
BG = (12, 15, 24)
PINK = (239, 112, 189)
CYAN = (53, 223, 223)
VIOLET = (166, 139, 250)
WHITE = (224, 255, 250)
# Custom cut-corner glyphs, in the same 76 x 18 block footprint as the signal.
GLYPHS = [
    (18, [(0,0),(4,0),(4,14),(7,13),(8,13),(8,12),(10,12),(10,13),(11,13),(14,14),(14,0),(18,0),
          (18,15),(15,18),(11,18),(9,14),(7,18),(3,18),(0,15)], []),
    (14, [(1,0),(13,0),(14,1),(14,3),(9,3),(9,15),(14,15),(14,17),
          (13,18),(1,18),(0,17),(0,15),(5,15),(5,3),(0,3),(0,1)], []),
    (16, [(3,0),(15,0),(16,1),(16,4),(5,4),(4,5),(4,7),(5,8),(12,8),
          (16,11),(16,15),(13,18),(1,18),(0,17),(0,14),(11,14),
          (12,13),(12,12),(11,11),(4,11),(0,8),(0,3)], []),
    (16, [(0,0),(12,0),(16,4),(16,9),(12,13),(4,13),(4,18),(0,18)],
         [[(4,4),(11,4),(12,5),(12,8),(11,9),(4,9)]]),
]


def blend(a, b, t):
    return tuple(round(x + (y-x)*t) for x, y in zip(a, b))


def gradient(x):
    t = max(0, min(1, x / (COLS-1)))
    return blend(PINK, CYAN, t*2) if t < .5 else blend(CYAN, VIOLET, t*2-1)


def inside_polygon(x, y, polygon):
    inside = False
    for (ax,ay),(bx,by) in zip(polygon,polygon[1:]+polygon[:1]):
        if (ay>y)!=(by>y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
            inside = not inside
    return inside


LETTER_BLOCKS = set()
left = 0
for width, outline, holes in GLYPHS:
    for y in range(18):
        for x in range(width):
            if inside_polygon(x+.5,y+.5,outline) and not any(inside_polygon(x+.5,y+.5,h) for h in holes):
                LETTER_BLOCKS.add((left+x,y))
    left += width+4
assert (left-4)*2 == COLS
MASK = {(x*2+dx,y*2+dy) for x,y in LETTER_BLOCKS for dx in (0,1) for dy in (0,1)}
BASE = {}
for x, y in LETTER_BLOCKS:
    # A restrained directional edge gives the letter shape clear structure.
    color = blend((184,201,222),(127,150,178),y/17)
    if (x,y-1) not in LETTER_BLOCKS:
        color = (225,239,250)
    elif (x,y+1) not in LETTER_BLOCKS:
        color = (95,120,150)
    elif (x-1,y) not in LETTER_BLOCKS:
        color = (198,217,236)
    for dx in (0,1):
        for dy in (0,1):
            BASE[x*2+dx,y*2+dy] = color


def make_signal():
    """A reproducible speech-like test signal, not a recording or intelligible speech."""
    rng = random.Random(42)
    syllables = [(.35,.24,.55),(.65,.18,.34),(.91,.40,.95),(1.40,.31,.62),
                 (2.12,.23,.42),(2.44,.38,1.0),(2.87,.19,.40),(3.14,.33,.83),
                 (3.57,.28,.61),(4.47,.22,.37),(4.77,.36,.78),(5.21,.23,.46)]
    samples, oscillator = [], 0.0
    for i in range(SAMPLE_RATE*DURATION):
        t = i/SAMPLE_RATE
        env = 0.0
        for onset, length, strength in syllables:
            u = (t-onset)/length
            if 0 < u < 1:
                # Fast onset and longer tail, with irregular syllable strengths.
                env += strength*(1-math.exp(-u*25))*(1-u)**1.5
        frequency = 137+19*math.sin(t*4.1)+7*math.sin(t*11.3)
        oscillator += 2*math.pi*frequency/SAMPLE_RATE
        voiced = (.61*math.sin(oscillator)+.24*math.sin(oscillator*2+.8)
                  +.10*math.sin(oscillator*5)+.06*math.sin(oscillator*9))
        samples.append(env*(voiced+.09*rng.uniform(-1,1)))
    peak = max(abs(v) for v in samples)
    return [v/peak*.9 for v in samples]


SAMPLES = make_signal()


def make_envelope():
    # Meter-style attack/release, sampled at 120 Hz. Each fixed bar reads the
    # same envelope with a small increasing delay; no sample history is shifted.
    step = SAMPLE_RATE//120
    rms = [math.sqrt(sum(v*v for v in SAMPLES[i:i+step])/step)
           for i in range(0,len(SAMPLES),step)]
    scale = max(rms)
    result, level = [], 0.0
    for value in rms:
        target = value/scale
        tau = .018 if target > level else .095
        alpha = 1-math.exp(-1/(120*tau))
        level += alpha*(target-level)
        result.append(level)
    return result


ENVELOPE = make_envelope()


def make_bar_profiles():
    # Fixed irregular tall/short profiles, with a deliberate contrast between
    # neighbors instead of a smooth curve across the columns.
    rng = random.Random(84)
    heights = [7]
    while len(heights) < COLS//2:
        choices = [h for h in range(3,9) if abs(h-heights[-1]) >= 3]
        heights.append(rng.choice(choices))
    responses = [rng.uniform(.50,.78) for _ in heights]
    return heights, responses


BAR_HEIGHTS, BAR_RESPONSES = make_bar_profiles()


def bar_amplitudes(t):
    result = []
    for bar, maximum in enumerate(BAR_HEIGHTS):
        # A 550 ms stagger crosses the word from W to P. The bar's horizontal
        # position and individual peak profile are permanent.
        delay = .55*bar/(len(BAR_HEIGHTS)-1)
        cursor = ((t-delay)%DURATION)*120
        index = int(cursor)
        fraction = cursor-index
        amplitude = ENVELOPE[index]*(1-fraction)+ENVELOPE[(index+1)%len(ENVELOPE)]*fraction
        # Height is the extra reach above/below a two-block resting band.
        height = 0 if amplitude < .04 else round((maximum-1)*amplitude**BAR_RESPONSES[bar])
        result.append(max(0,min(7,height)))
    return result


def waveform(phase=0):
    # Waveform blocks are 2x2 letter-grid cells: 9 px artwork on a 10 px pitch.
    blocks = set()
    time = 1.22 + phase/(2*math.pi)*DURATION
    for bar, height in enumerate(bar_amplitudes(time)):
        blocks.update((bar,y) for y in range((CENTER-2)//2-height,(CENTER-2)//2+height+2))
    # Four extra anchored columns at either end, tapering toward the outside.
    # Original columns keep their exact heights, timing, position, and color.
    for distance in range(1,MARGIN_BLOCKS+1):
        delay = .55*distance/(len(BAR_HEIGHTS)-1)
        strength = 1-.12*distance
        left_height = round(bar_amplitudes(time+delay)[0]*strength)
        right_height = round(bar_amplitudes(time-delay)[-1]*strength)
        for bar,height in [(-distance,left_height),(COLS//2-1+distance,right_height)]:
            blocks.update((bar,y) for y in range((CENTER-2)//2-height,(CENTER-2)//2+height+2))
    return {(x*2+dx,y*2+dy) for x,y in blocks for dx in (0,1) for dy in (0,1)}


def artwork(phase=0, colors=None):
    pixels = dict(BASE)
    for x, y in waveform(phase):
        # TTFX applies a gentle global color pulse. No gradient travels sideways.
        bx, by = x//2*2, y//2*2
        color_x = max(0,min(COLS-2,bx))
        color = blend(gradient(bx), colors[color_x,by], .12) if colors else gradient(bx)
        pixels[x, y] = blend(color,WHITE,.10) if by in (CENTER-2,CENTER) else color
    return pixels


def artwork_rects(pixels):
    for (x,y), color in pixels.items():
        if x%2 or y%2:
            continue
        yield (OX+x*CELL, OY+y*CELL, CELL*2-1, CELL*2-1, color)


def hexcolor(c):
    return '#%02x%02x%02x' % c


def rect_svg(x, y, w, h, color):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{hexcolor(color)}"/>'


def fixed_rects():
    rects = []
    # Quiet terminal crop marks; no faux application controls.
    for x, y, sx, sy in [(34,34,1,1),(WIDTH-34,34,-1,1),(34,HEIGHT-34,1,-1),(WIDTH-34,HEIGHT-34,-1,-1)]:
        rects.extend([(min(x,x+sx*16), y, 17, 1, (53,65,83)),
                      (x, min(y,y+sy*16), 1, 17, (53,65,83))])
    return rects


FIXED = fixed_rects()


def write_svg(pixels, name, poster):
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-labelledby="title desc" shape-rendering="crispEdges">',
             '<title id="title">WISP — extended waveform</title>',
             '<desc id="desc">Custom pixel WISP lettering with an anchored waveform extending four blocks beyond each end of the text.</desc>']
    if poster:
        parts.append(rect_svg(0, 0, WIDTH, HEIGHT, BG))
        parts.extend(rect_svg(*r) for r in FIXED)
    parts.extend(rect_svg(*r) for r in artwork_rects(pixels))
    parts.append('</svg>\n')
    (OUT / name).write_text('\n'.join(parts))


def fill(buf, x, y, w, h, c):
    line = bytes(c)*w
    for row in range(y, y+h):
        start = (row*WIDTH+x)*3
        buf[start:start+w*3] = line


def background():
    buf = bytearray(bytes(BG)*(WIDTH*HEIGHT))
    for r in FIXED:
        fill(buf, *r)
    return buf


BACKGROUND = background()


def render_cells(cells):
    buf = bytearray(BACKGROUND)
    pixels = {(x//2,y): c for x,y,char,c in cells if x%2 == 0 and char == '█'}
    for r in artwork_rects(pixels):
        fill(buf,*r)
    return buf


def cells_for(pixels):
    return [(x*2+d, y, '█', c) for (x, y), c in pixels.items() for d in (0, 1)]


def parse_frames(stream):
    frames = []
    # TTFX drops leading blank input lines before north-west anchoring.
    top_padding = min(y for x, y in artwork())
    # ttfx 0.3.2 redraws the whole canvas, restoring/saving its cursor between frames.
    for chunk in stream.split('\x1b8\x1b7')[1:]:
        chunk = re.sub(r'^\x1b\[\d+A', '', chunk)
        chunk = chunk.split('\x1b[?25h')[0]
        cells, x, y, color = [], 0, 0, WHITE
        for match in re.finditer(r'\x1b\[([0-9;?]*)([A-Za-z])|([^\x1b])', chunk):
            args, command, char = match.groups()
            if command:
                if command == 'm':
                    params = [int(p) for p in args.split(';')]
                    if params[:2] == [38, 2]:
                        color = tuple(params[2:5])
                    elif params == [0]:
                        color = WHITE
                    else:
                        raise ValueError(f'Unsupported SGR: {args}')
                else:
                    raise ValueError(f'Unsupported cursor command: {command}')
            elif char == '\n':
                x, y = 0, y+1
            elif char == '\r':
                x = 0
            else:
                if char != ' ' and x < COLS*2 and y < ROWS:
                    cells.append((x, y+top_padding, char, color))
                x += 1
        frames.append(cells)
    if not frames:
        raise ValueError('TTFX produced no recognized frames')
    return frames


def write_terminal(pixels, stem):
    lines, colored = [], []
    for y in range(ROWS):
        line, ansi = '', ''
        for x in range(-MARGIN_BLOCKS*2,COLS+MARGIN_BLOCKS*2) if stem == 'wisp' else range(COLS):
            c = pixels.get((x, y))
            line += '██' if c else '  '
            ansi += ('\x1b[38;2;%d;%d;%dm██\x1b[0m' % c) if c else '  '
        lines.append(line)
        colored.append(ansi)
    (OUT/f'{stem}.txt').write_text('\n'.join(lines)+'\n')
    (OUT/f'{stem}.ansi').write_text('\n'.join(colored)+'\n')


def main():
    for executable in ['ttfx', 'ffmpeg', 'magick']:
        if not shutil.which(executable):
            raise SystemExit(f'Required executable not found: {executable}')
    # Give the real TTFX effect the waveform's full motion envelope. Composite
    # its colored cells through a moving audio-bar mask over stationary letters.
    envelope = {(x, y): gradient(x) for x in range(COLS) for y in range(ROWS)
                if x % 2 == 0}
    write_terminal(envelope, 'wave-envelope')
    with wave.open(str(OUT/'synthetic-voice-signal.wav'),'wb') as audio:
        audio.setparams((1,2,SAMPLE_RATE,0,'NONE','not compressed'))
        audio.writeframes(b''.join(struct.pack('<h',round(v*32767)) for v in SAMPLES))

    args = ['ttfx', '--frame-rate', '0', '--seed', '42',
            '--canvas-width', str(COLS*2), '--canvas-height', str(ROWS),
            '--anchor-text', 'nw', '--ignore-terminal-dimensions',
            'colorshift', '--gradient-stops', 'ef70bd', '35dfdf', 'a68bfa',
            '--gradient-steps', '24', '--gradient-frames', '2',
            '--cycles', '2', '--no-travel', '--skip-final-gradient']
    result = subprocess.run(args, input=(OUT/'wave-envelope.txt').read_text(),text=True,capture_output=True,check=True,timeout=45)
    (OUT/'ttfx-colorshift.ansi').write_text(result.stdout)
    frames = parse_frames(result.stdout)
    captured_frame_count = len(frames)
    # Capture two cycles so the chosen seam precedes TTFX's final teardown.
    loop_end = next(i for i in range(2, len(frames)) if frames[i] == frames[0])
    frames = frames[:loop_end+1]
    expected = {(x, y) for x, y, char, color in cells_for(envelope)}
    assert all({(x,y) for x,y,char,color in frame} == expected for frame in frames)
    assert frames[0] == frames[-1], 'TTFX color cycle must close'
    sequence = []
    frame_count = DURATION*30+1
    motion = [bar_amplitudes(1.22+i/30) for i in range(frame_count-1)]
    variations = [max(row[bar] for row in motion)-min(row[bar] for row in motion)
                  for bar in range(len(BAR_HEIGHTS))]
    assert all(change >= 2 for change in variations), 'Every waveform column must visibly spike'
    for i in range(frame_count):
        frame = frames[round(i*(len(frames)-1)/(frame_count-1))]
        # Merge pairs of terminal columns into one square art cell.
        colors = {(x//2, y): c for x, y, char, c in frame if x % 2 == 0}
        phase = 2*math.pi*i/(frame_count-1) if i < frame_count-1 else 0
        pixels = artwork(phase, colors)
        assert MASK <= pixels.keys(), 'Letter silhouettes must remain visible'
        assert all((x,CENTER-1) in pixels for x in range(-MARGIN_BLOCKS*2,COLS+MARGIN_BLOCKS*2))
        sequence.append(cells_for(pixels))
        if i == 0:
            write_terminal(pixels, 'wisp')
            write_svg(pixels, 'wisp.svg', False)
            write_svg(pixels, 'wisp-poster.svg', True)
            write_svg(BASE, 'lettering.svg', False)
            write_svg(BASE, 'lettering-poster.svg', True)
    assert sequence[0] == sequence[-1], 'Composited loop endpoints must match'
    resting_phase = (6.15-1.22)/DURATION*2*math.pi
    assert all(h == 0 for h in bar_amplitudes(6.15)), 'Resting waveform must be exactly two blocks tall'
    write_svg(artwork(resting_phase), 'wisp-resting.svg', True)
    for source in ['wisp', 'wisp-poster', 'lettering', 'lettering-poster', 'wisp-resting']:
        subprocess.run(['magick','-background','none',str(OUT/f'{source}.svg'),str(OUT/f'{source}.png')],check=True)
    playback = args[:]
    playback[playback.index('0')] = '30'
    playback[playback.index('--canvas-width')+1] = str((COLS+MARGIN_BLOCKS*4)*2)
    (OUT/'play-terminal.sh').write_text('#!/usr/bin/env bash\nset -euo pipefail\ncd -- "$(dirname -- "${BASH_SOURCE[0]}")"\n'+shlex.join(playback)+' < wisp.ansi\n')

    # Terminal replay applies the same actual effect to the complete text;
    # the GIF separately keeps letter colors fixed and pulses anchored bars.
    with tempfile.TemporaryDirectory(prefix='wisp-art-') as tmp:
        video = Path(tmp)/'frames.mkv'
        command = ['ffmpeg','-v','error','-y','-f','rawvideo','-pixel_format','rgb24','-video_size',f'{WIDTH}x{HEIGHT}',
                   '-framerate','30','-i','-','-c:v','ffv1',str(video)]
        proc = subprocess.Popen(command, stdin=subprocess.PIPE)
        for cells in sequence:
            proc.stdin.write(render_cells(cells))
        proc.stdin.close()
        if proc.wait() != 0:
            raise SystemExit('Frame encoding failed')
        subprocess.run(['ffmpeg','-v','error','-y','-i',str(video),'-filter_complex',
                        '[0:v]split[a][b];[a]palettegen=max_colors=128:stats_mode=full[p];[b][p]paletteuse=dither=none',
                        '-loop','0',str(OUT/'wisp.gif')],check=True)
        indices = [round(i*(len(sequence)-1)/5) for i in range(6)]
        for i, idx in enumerate(indices):
            ppm = Path(tmp)/f'{i}.ppm'
            ppm.write_bytes(f'P6\n{WIDTH} {HEIGHT}\n255\n'.encode()+render_cells(sequence[idx]))
        subprocess.run(['magick','montage',*[str(Path(tmp)/f'{i}.ppm') for i in range(6)],
                        '-geometry','480x200+8+8','-tile','2x3','-background','#0c0f18',str(OUT/'animation-contact-sheet.png')],check=True)
    manifest = {'ttfx_version': subprocess.check_output(['ttfx','--version'],text=True).strip(),
                'ttfx_sha256': hashlib.sha256(Path(shutil.which('ttfx')).read_bytes()).hexdigest(),
                'command': args, 'ttfx_frames': captured_frame_count, 'gif_frames': len(sequence),
                'fps': 30, 'size': [WIDTH,HEIGHT], 'loop_endpoints_identical': True,
                'continuous_waveform': True, 'letters_visible_every_frame': True,
                'compositing': 'Fixed-position bars driven by an audio envelope, staggered left to right; restrained non-traveling TTFX colorshift',
                'signal': 'Deterministic synthetic speech-like signal; no recordings or microphone input',
                'left_to_right_stagger_seconds': .55, 'attack_seconds': .018,
                'release_seconds': .095, 'sample_rate': SAMPLE_RATE,
                'waveform_block_pixels': 9, 'letter_block_pixels': 9, 'waveform_bars': len(BAR_HEIGHTS)+MARGIN_BLOCKS*2,
                'waveform_extension_each_side_pixels': MARGIN_BLOCKS*CELL*2,
                'lettering': 'Custom cut-corner glyphs; W center lowered four blocks and narrowed from four to two blocks',
                'resting_waveform_height_blocks': 2,
                'all_waveform_columns_animate': True, 'minimum_height_variation_blocks': min(variations),
                'minimum_neighbor_peak_difference_blocks': min(abs(a-b) for a,b in zip(BAR_HEIGHTS,BAR_HEIGHTS[1:])),
                'bar_profile_seed': 84,
                'horizontal_translation': False}
    (OUT/'generation.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2))


if __name__ == '__main__':
    main()
