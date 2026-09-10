"""Run against the generated-audio daemon and private sink in test-audio-test.sh."""
import array
import json
import math
from pathlib import Path
import socket
import subprocess
import sys
import time
import wave

folder, monitor = Path(sys.argv[1]), sys.argv[2]
connection = socket.socket(socket.AF_UNIX)
connection.connect(str(folder / "wispd.sock"))
connection.settimeout(20)
reader = connection.makefile("r")
serial = 0

def send(name, args=None):
    global serial
    serial += 1
    ident = f"sound-{serial}"
    connection.sendall((json.dumps({"v":1,"id":ident,"type":"command","name":name,"args":args or {}})+"\n").encode())
    return ident

def receive(ident, success=True):
    while True:
        value = json.loads(reader.readline())
        if value.get("type") == "result" and value.get("id") == ident:
            assert bool(value["ok"]) == success, f"Unexpected outcome for {ident}"
            return value.get("value", {})

def command(name, args=None, success=True):
    return receive(send(name, args), success)

status = command("status")
server = status["servers"][0]["id"]
args = {"server_id":server}
source = folder / "clip.wav"
with wave.open(str(source), "wb") as wav:
    wav.setparams((1,2,48000,0,"NONE","not compressed"))
    pcm = array.array("h", (int(15000*math.sin(2*math.pi*440*i/48000)) for i in range(48000)))
    wav.writeframes(pcm.tobytes())
clips = []
for extension in ["wav","mp3","ogg","flac"]:
    file = folder / f"clip.{extension}"
    if extension != "wav":
        subprocess.run(["ffmpeg","-v","error","-nostdin","-i",str(source),"-y",str(file)],check=True,capture_output=True)
    clip = command("soundboard_upload",dict(args,name=f"test-{extension}",path=str(file)))
    assert 990 <= clip["duration_ms"] <= 1100
    clips.append(clip["id"])
assert len(command("soundboard_list",args)["sounds"]) == 4
play = dict(args,sound_id=clips[0],volume=70)
command("soundboard_play",play,success=False)
with (folder / "sound-preview.pcm").open("wb") as recording:
    capture = subprocess.Popen(["parec",f"--device={monitor}","--format=s16le","--rate=48000","--channels=1","--latency-msec=20"],stdout=recording,stderr=subprocess.DEVNULL)
    try:
        time.sleep(.2)
        assert command("soundboard_preview",play)["previewing"]
        time.sleep(1.4)
        assert not command("soundboard_status")["previewing"]
    finally:
        capture.terminate();capture.wait(timeout=5)
pcm = array.array("h");pcm.frombytes((folder / "sound-preview.pcm").read_bytes())
assert max(map(abs,pcm),default=0) > 3000, "Preview did not reach the private speaker"
# Both requests arrive together: Stop must cancel even a not-yet-polled download.
preview_id = send("soundboard_preview", play)
stop_id = send("soundboard_stop")
received = {}
while len(received) < 2:
    value = json.loads(reader.readline())
    if value.get("id") in [preview_id,stop_id] and value.get("type") == "result": received[value["id"]] = value
assert received[stop_id]["ok"]
time.sleep(.2)
stopped = command("soundboard_status")
assert not stopped["playing"] and not stopped["previewing"]
command("set_deafened",{"deafened":True})
command("soundboard_preview",play,success=False)
command("set_deafened",{"deafened":False})
for clip in clips: command("soundboard_remove",dict(args,sound_id=clip))
assert not command("soundboard_list",args)["sounds"]
status = command("status")["self"]
assert status["hangout_id"] is None and not status["media"]["microphone_published"] and not status["media"]["livekit_connected"]
connection.close()
print("Soundboard WAV/MP3/Ogg/FLAC upload, audible private preview, Stop cancellation, deafen, removal and room isolation passed.")
