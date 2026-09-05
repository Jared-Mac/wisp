#!/usr/bin/env python3
"""Exercise enrollment discovery against two isolated servers, with media disabled."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BIN = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')) / 'release'


def eventually(action):
    last = None
    for _ in range(100):
        try:
            result = action()
            if result:
                return result
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            last = type(error).__name__
        time.sleep(0.1)
    raise AssertionError(f'Isolated test timed out ({last})')


with tempfile.TemporaryDirectory(prefix='wisp-invite-discovery-') as directory:
    root = Path(directory)
    env = {key: value for key, value in os.environ.items() if not key.startswith('WISP_')}
    env.update(XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
               XDG_RUNTIME_DIR=str(root / 'runtime'), WISP_DISABLE_TRAY='1', WISP_PROFILE='Test Owner',
               WISP_ACCOUNTS_FILE=str(root / 'config/wisp/accounts.json'),
               WISP_HYPR_CONFIG_DIR=str(root / 'hypr'))
    (root / 'runtime').mkdir(mode=0o700)
    processes = []
    logs = []
    try:
        servers = []
        for number in range(2):
            with socket.socket() as sock:
                sock.bind(('127.0.0.1', 0))
                port = sock.getsockname()[1]
            url = f'http://127.0.0.1:{port}'
            log = (root / f'server-{number}.log').open('w')
            logs.append(log)
            processes.append(subprocess.Popen([str(BIN / 'wisp-server'), '--addr', f'127.0.0.1:{port}',
                '--database-url', f'sqlite://{root}/server-{number}.sqlite3', '--allow-dev-sessions', 'false',
                '--bootstrap-token', 'isolated-bootstrap-fixture'], env=env, stdout=log, stderr=log))
            eventually(lambda: urllib.request.urlopen(url + '/healthz', timeout=1).status == 200)
            servers.append(url)

        def enroll(url):
            request = dict(action='bootstrap', server_url=url, username='test_owner', display_name='Test Owner',
                           password='isolated fixture password', bootstrap_token='isolated-bootstrap-fixture',
                           device_name='Test device', media_key='isolated-test-media-key')
            subprocess.run([str(BIN / 'wisp-account')], input=json.dumps(request) + '\n', text=True,
                           env=env, capture_output=True, check=True)

        enroll(servers[0])
        log = (root / 'daemon.log').open('w')
        logs.append(log)
        daemon = subprocess.Popen([str(BIN / 'wispd'), '--disable-media', '--disable-surfaces'],
                                  env=env, stdout=log, stderr=log)
        processes.append(daemon)

        def status():
            return json.loads(subprocess.check_output([str(BIN / 'wispctl'), 'status'], env=env,
                                                      stderr=subprocess.DEVNULL, timeout=2))

        eventually(lambda: len(status()['servers']) == 1)
        subprocess.run([str(BIN / 'wispctl'), 'deafen'], env=env, capture_output=True, check=True)
        before = status()
        enroll(servers[1])
        eventually(lambda: len(status()['servers']) == 2 and all(s['connected'] for s in status()['servers']))
        after = status()
        assert daemon.poll() is None, 'original daemon must stay running'
        assert after['voice_server_id'] == before['voice_server_id'], 'voice context must not switch'
        assert after['selected_server_id'] != before['selected_server_id'], 'new server becomes visible'
        assert after['self']['hangout_id'] is None
        assert after['self']['muted'] == before['self']['muted']
        assert after['self']['deafened'] == before['self']['deafened']
        assert not after['self']['sharing']
        assert not after['self']['media']['camera']['active']
        print('New server discovered without restart, room join, media publication, or audio preference changes')
    finally:
        for process in reversed(processes):
            process.terminate()
        for process in reversed(processes):
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        for log in logs:
            log.close()
