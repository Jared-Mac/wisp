#!/usr/bin/env python3
"""Generate Wisp's original starter effects; no recordings or external samples."""
import array
import json
import math
from pathlib import Path
import random
import wave

RATE = 48000
ROOT = Path(__file__).resolve().parents[1] / "quickshell/app/assets/soundboard"
ROOT.mkdir(parents=True, exist_ok=True)
rng = random.Random(1781)
manifest = []

def mix(track, start, duration, sound, gain=1):
    offset = round(start*RATE)
    length = round(duration*RATE)
    for i in range(min(length,len(track)-offset)):
        t = i/RATE
        fade = min(1,i/(RATE*.003),(length-1-i)/(RATE*.02))
        track[offset+i] += sound(t)*max(0,fade)*gain

def tone(frequency, decay=4, harmonics=(1,)):
    def sample(t):
        return sum(a*math.sin(2*math.pi*frequency*(i+1)*t) for i,a in enumerate(harmonics))*math.exp(-decay*t)
    return sample

def noise_hit(decay, metallic=False):
    previous = 0
    def sample(t):
        nonlocal previous
        white = rng.uniform(-1,1)
        high = white-previous*.75
        previous = white
        metal = sum(math.sin(2*math.pi*f*t) for f in (2630,3471,4891,6173))/4 if metallic else 0
        return (high*.75+metal*.25)*math.exp(-decay*t)
    return sample

def save(name, filename, track):
    peak = max(map(abs,track))
    rms = math.sqrt(sum(v*v for v in track)/len(track))
    gain = min(.70/peak,.15/rms)
    pcm = array.array('h',(round(v*gain*32767) for v in track))
    assert max(map(abs,pcm)) <= 22938
    assert pcm[0] == 0 and pcm[-1] == 0
    if __import__('sys').byteorder != 'little': pcm.byteswap()
    with wave.open(str(ROOT/filename),'wb') as output:
        output.setparams((1,2,RATE,0,'NONE','not compressed'))
        output.writeframes(pcm.tobytes())
    manifest.append({'name':name,'file':filename,'duration_ms':round(len(track)*1000/RATE)})

def blank(seconds): return [0.] * round(seconds*RATE)

track=blank(.85)
mix(track,0,.8,tone(1046.5,6,(1,.18,.08)))
save('Ping','ping.wav',track)

track=blank(1.15)
for i,f in enumerate([261.63,329.63,392,523.25,659.25,1046.5]):
    mix(track,i*.105,.50,tone(f,6,(1,.15)),.8)
save('Level Up','level-up.wav',track)

track=blank(.9)
mix(track,0,.85,lambda t: math.sin(2*math.pi*(165*t+30*(1-math.exp(-12*t)))+3*math.sin(2*math.pi*18*t)*math.exp(-5*t))*math.exp(-5*t))
save('Boing','boing.wav',track)

track=blank(1.25)
for start,duration in [(0,.32),(.45,.64)]:
    mix(track,start,duration,lambda t: sum(math.sin(2*math.pi*f*(1+.002*math.sin(2*math.pi*5*t))*t)/h for f0 in (233.08,293.66) for h in range(1,7) for f in [f0*h])*.25)
save('Air Horn','air-horn.wav',track)

track=blank(1.35)
for start in [0,.17]:
    mix(track,start,.13,noise_hit(28),.6)
    mix(track,start,.16,tone(160,25),.45)
mix(track,.39,.82,noise_hit(7,True),.75)
mix(track,.39,.27,tone(70,18),.45)
save('Rimshot','rimshot.wav',track)

track=blank(2.1)
for i in range(24):
    mix(track,i*.058,.16,noise_hit(35),.15+.7*i/23)
    mix(track,i*.058,.11,tone(175,28),.16)
mix(track,1.48,.55,noise_hit(9,True),.95)
mix(track,1.48,.23,tone(75,16),.55)
save('Drum Roll','drum-roll.wav',track)

track=blank(2.2)
for i in range(70):
    start = .025+i*.024+rng.uniform(0,.025)
    gain = rng.uniform(.15,.45)*min(1,(i+8)/20,(78-i)/18)
    for slap in [0,.012,.026]: mix(track,start+slap,.09,noise_hit(48),gain)
save('Applause','applause.wav',track)

track=blank(2.05)
for i,f in enumerate([293.66,277.18,261.63,196]):
    duration=.32 if i<3 else .9
    def brass(t,f=f,last=i==3):
        phase=2*math.pi*(f*t-(30*t*t if last else 0))+.025*math.sin(2*math.pi*5*t)
        return sum(math.sin(phase*h)/h for h in range(1,6))*math.exp(-.7*t)
    mix(track,i*.34,duration,brass,.65)
save('Sad Trombone','sad-trombone.wav',track)
(ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'Generated {len(manifest)} original sounds ({sum((ROOT/s["file"]).stat().st_size for s in manifest):,} bytes).')
