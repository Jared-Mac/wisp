"""Generate Wisp's quiet two-note notification (no external audio assets)."""
import math
from pathlib import Path
import struct
import wave

target = Path(__file__).resolve().parents[1] / "quickshell/app/assets/message.wav"
rate = 44100
frames = []
for index in range(int(rate * 0.38)):
    t = index / rate
    value = 0.0
    for start, frequency in [(0.0, 740), (0.10, 988)]:
        age = t - start
        if 0 <= age < 0.27:
            envelope = min(age / 0.012, 1) * math.exp(-age * 18)
            envelope *= min((0.27 - age) / 0.02, 1)
            value += 0.23 * envelope * math.sin(2 * math.pi * frequency * age)
    frames.append(struct.pack("<h", int(value * 32767)))
with wave.open(str(target), "wb") as audio:
    audio.setnchannels(1)
    audio.setsampwidth(2)
    audio.setframerate(rate)
    audio.writeframes(b"".join(frames))
