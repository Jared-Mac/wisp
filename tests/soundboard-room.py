"""Synthetic end-to-end soundboard audio over a loopback LiveKit room."""
import array
import json
import math
import os
from pathlib import Path
import random
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.request
import wave

repo = Path(__file__).resolve().parents[1]
folder = Path(tempfile.mkdtemp(prefix="wisp-soundboard-room-"))
processes, modules, clients = [], [], []
serial = 0

class Client:
    def __init__(self, path):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(str(path)); self.socket.settimeout(30)
        self.reader = self.socket.makefile("r")
    def command(self, name, args=None, success=True):
        global serial
        serial += 1; ident = str(serial)
        self.socket.sendall((json.dumps({"v":1,"id":ident,"type":"command","name":name,"args":args or {}})+"\n").encode())
        while True:
            response = json.loads(self.reader.readline())
            if response.get("type") == "result" and response.get("id") == ident:
                assert bool(response["ok"]) == success, f"Unexpected outcome: {name}"
                return response.get("value", {})

def wait_for(predicate, message, seconds=25):
    deadline = time.monotonic()+seconds
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value: return value
        except (OSError, KeyError): pass
        time.sleep(.1)
    raise AssertionError(message)

def launch(name, argv, env):
    log = (folder / f"{name}.log").open("wb")
    process = subprocess.Popen(argv, env=env, stdout=log, stderr=log)
    log.close(); processes.append(process)
    return process

def record(name, monitor, action=None):
    path = folder / f"{name}.pcm"
    with path.open("wb") as output:
        capture = subprocess.Popen(["parec",f"--device={monitor}","--format=s16le","--rate=48000","--channels=1","--latency-msec=20"],stdout=output,stderr=subprocess.DEVNULL)
        try:
            time.sleep(.2)
            if action: action()
            time.sleep(2.4)
        finally: capture.terminate();capture.wait(timeout=5)
    samples = array.array("h"); samples.frombytes(path.read_bytes())
    return samples

def tone_amplitude(samples):
    # A 100 ms window contains an exact number of periods at 880 Hz.
    sine = [math.sin(2*math.pi*880*i/48000) for i in range(4800)]
    cosine = [math.cos(2*math.pi*880*i/48000) for i in range(4800)]
    peaks = []
    for offset in range(0,len(samples)-4800,4800):
        frame = samples[offset:offset+4800]
        peaks.append(2*math.hypot(sum(a*b for a,b in zip(frame,sine)),sum(a*b for a,b in zip(frame,cosine)))/4800)
    return max(peaks,default=0)

passed = False
try:
    port = random.randrange(22000,26000)
    web = random.randrange(30000,38000)
    rtc = random.randrange(42000,50000)
    config = (repo / "infra/local/livekit.yaml").read_text().replace("port: 7880",f"port: {port}").replace("tcp_port: 7881",f"tcp_port: {port+1}").replace("port_range_start: 50000",f"port_range_start: {rtc}").replace("port_range_end: 50100",f"port_range_end: {rtc+50}")
    (folder / "livekit.yaml").write_text(config)
    env = dict(os.environ,WISP_E2EE_KEY="wisp-integration-e2ee-key-32-bytes",WISP_TEST_MICROPHONE_TONE="1",WISP_DISABLE_TRAY="1",WISP_SERVER_URL=f"http://127.0.0.1:{web}")
    launch("livekit",[str(repo/".tools/livekit/livekit-server"),"--node-ip","127.0.0.1","--config",str(folder/"livekit.yaml")],env)
    launch("server",[str(repo/"target/release/wisp-server")],dict(env,WISP_SERVER_ADDR=f"127.0.0.1:{web}",WISP_DATABASE_URL=f"sqlite://{folder}/server.sqlite3",WISP_LIVEKIT_URL=f"ws://127.0.0.1:{port}"))
    wait_for(lambda:json.load(urllib.request.urlopen(env["WISP_SERVER_URL"]+"/healthz",timeout=1))["ok"],"Temporary server not ready")
    for index,profile in enumerate(["Owner","MemberA"]):
        sink=f"wisp_soundboard_{os.getpid()}_{index}"
        module=subprocess.check_output(["pactl","load-module","module-null-sink",f"sink_name={sink}",f"sink_properties=device.description={sink}"],text=True).strip();modules.append(module)
        path=folder/f"{index}.sock"
        launch(profile,[str(repo/"target/release/wispd"),"--profile",profile,"--disable-surfaces","--socket",str(path)],dict(env,XDG_CONFIG_HOME=str(folder/f"config-{index}"),WISP_ACCOUNTS_FILE=str(folder/f"accounts-{index}.json"),PULSE_SINK=sink,PULSE_SOURCE=sink+".monitor"))
        wait_for(path.exists,"Temporary daemon not ready")
        client=Client(path);clients.append(client)
        wait_for(lambda:client.command("status")["self"]["connection"]=="available","Client did not connect")
        devices=client.command("refresh_audio_devices")
        output=next(d["id"] for d in devices["output_devices"] if d["name"]==sink)
        client.command("set_output_device",{"id":output})
    owner,member=clients
    member.command("set_muted",{"muted":True})
    status=owner.command("status");server=status["servers"][0]["id"];room=status["spots"][0]["id"]
    for client in clients: client.command("join_spot",{"server_id":server,"spot_id":room})
    for client in clients: wait_for(lambda:client.command("status")["self"]["media"]["livekit_connected"],"Voice did not connect")
    wait_for(lambda:"Owner" in member.command("status")["self"]["media"]["remote_audio_participants"],"Receiver did not subscribe")
    time.sleep(1)
    path=folder/"effect.wav"
    with wave.open(str(path),"wb") as wav:
        wav.setparams((1,2,48000,0,"NONE","not compressed"))
        wav.writeframes(array.array("h",(int(16000*math.sin(2*math.pi*880*i/48000)) for i in range(96000))).tobytes())
    clip=owner.command("soundboard_upload",{"server_id":server,"name":"Voice effect","path":str(path)})
    play={"server_id":server,"sound_id":clip["id"],"volume":100}
    owner.command("soundboard_preview",play,success=False)
    monitor=f"wisp_soundboard_{os.getpid()}_1.monitor"
    baseline=tone_amplitude(record("baseline",monitor))
    def play_effect():
        assert owner.command("soundboard_play",play)["playing"]
        time.sleep(.4)
    received=tone_amplitude(record("effect",monitor,play_effect))
    assert received > max(1500,baseline*4),f"Remote effect missing: baseline={baseline:.1f}, played={received:.1f}"
    owner.command("soundboard_play",play);time.sleep(.2);owner.command("set_muted",{"muted":True})
    assert not owner.command("soundboard_status")["playing"]
    owner.command("soundboard_play",play,success=False)
    owner.command("set_muted",{"muted":False});time.sleep(1)
    owner.command("soundboard_play",play);owner.command("leave")
    assert not owner.command("soundboard_status")["playing"]
    for client in clients:
        client.command("leave")
        status=client.command("status")["self"]
        assert not status["media"]["microphone_published"] and not status["sharing"]
    print(f"Remote soundboard playback verified over LiveKit (880 Hz amplitude {received:.0f}; baseline {baseline:.0f}). Mute and leave stop playback.")
    passed=True
finally:
    for client in clients:
        try:client.command("leave")
        except Exception:pass
        client.socket.close()
    for process in reversed(processes):
        process.terminate()
        try:process.wait(timeout=8)
        except subprocess.TimeoutExpired:process.kill();process.wait()
    for module in modules:subprocess.run(["pactl","unload-module",module],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    if passed:shutil.rmtree(folder)
    else:print(f"Isolated soundboard logs retained: {folder}")
