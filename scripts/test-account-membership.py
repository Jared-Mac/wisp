#!/usr/bin/env python3
"""Production-auth signup, encrypted invite and live membership on isolated services."""
import base64
import json
import os
import re
from pathlib import Path
import socket
import sys
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BIN = Path(os.environ.get('WISP_TEST_BIN_DIR', Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')) / 'release'))


def eventually(fn):
    for _ in range(150):
        try:
            result = fn()
            if result:
                return result
        except (OSError, ValueError, KeyError, subprocess.SubprocessError):
            pass
        time.sleep(.1)
    raise AssertionError('Isolated account test timed out')


with tempfile.TemporaryDirectory(prefix='wisp-accounts-') as directory:
    root = Path(directory)
    clean = {k: v for k, v in os.environ.items() if not k.startswith('WISP_')}
    processes, logs = [], []
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    origin = f'http://127.0.0.1:{port}'

    def environment(name):
        base = root / name
        (base / 'runtime').mkdir(parents=True, mode=0o700)
        return dict(clean, XDG_CONFIG_HOME=str(base / 'config'), XDG_DATA_HOME=str(base / 'data'),
                    XDG_RUNTIME_DIR=str(base / 'runtime'), WISP_DISABLE_TRAY='1',
                    WISP_ACCOUNT_SERVER=origin, WISP_PROFILE='Test account', WISP_HYPR_CONFIG_DIR=str(base / 'hypr'))

    def start(args, env, name):
        log = (root / (name + '.log')).open('w')
        logs.append(log)
        proc = subprocess.Popen(args, env=env, stdout=log, stderr=log)
        processes.append(proc)
        return proc

    def helper(env, body, ok=True):
        result = subprocess.run([str(BIN / 'wisp-account')], input=json.dumps(body)+'\n',
                                text=True, env=env, capture_output=True, timeout=25)
        assert (result.returncode == 0) == ok, 'Account helper result did not match expected success'
        return json.loads(result.stdout) if ok else None

    def registry(env):
        return json.loads((Path(env['XDG_CONFIG_HOME']) / 'wisp/accounts.json').read_text())

    def request(path, body=None, token=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        req = urllib.request.Request(origin + path, data=None if body is None else json.dumps(body).encode(), headers=headers)
        with urllib.request.urlopen(req, timeout=3) as response:
            return json.load(response)

    def session(env):
        account = registry(env)['servers'][0]
        return request('/v1/sessions', dict(device_id=account['device_id'], device_token=account['device_token'], protocol_version=1))['token']

    def ipc(env, name, args=None):
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(3)
            client.connect(str(Path(env['XDG_RUNTIME_DIR']) / 'wisp/wispd.sock'))
            client.sendall((json.dumps(dict(v=1,id='test',type='command',name=name,args=args or {}))+'\n').encode())
            with client.makefile() as stream:
                for line in stream:
                    value = json.loads(line)
                    if value.get('id') == 'test':
                        assert value.get('ok'), 'Daemon account operation failed: ' + name
                        return value.get('value')
        raise AssertionError('Missing command result')

    def status(env):
        return json.loads(subprocess.check_output([str(BIN / 'wispctl'), 'status'], env=env, stderr=subprocess.DEVNULL, timeout=3))

    try:
        owner, recipient = environment('owner'), environment('recipient')
        start([str(BIN / 'wisp-server'), '--addr', f'127.0.0.1:{port}', '--database-url', f'sqlite://{root}/server.sqlite3', '--invite-url', origin, '--public-url', origin, '--allow-dev-sessions', 'false', '--bootstrap-token', 'test-bootstrap-only'], clean, 'server')
        eventually(lambda: request('/healthz')['ok'])
        helper(owner, dict(action='bootstrap', server_url=origin, username='owner', display_name='Example owner', password='test-only-password-123', bootstrap_token='test-bootstrap-only', device_name='Test owner', media_key='test-only-shared-media-key'))
        helper(recipient, dict(action='register', username='new-user', display_name='New account', password='test-only-password-123', device_name='Test recipient'))
        before = registry(recipient)
        assert before['servers'][0]['media_key'] is None
        token = session(recipient)
        assert not request('/v1/snapshot', token=token)['server_member']
        assert request('/v1/people', token=token)['people'][0]['relationship'] == 'self'
        owner_daemon = start([str(BIN / 'wispd'), '--disable-media', '--disable-surfaces'], owner, 'owner-daemon')
        recipient_daemon = start([str(BIN / 'wispd'), '--disable-media', '--disable-surfaces'], recipient, 'recipient-daemon')
        eventually(lambda: status(owner)['self']['display_name'] == 'Example owner')
        eventually(lambda: status(recipient)['self']['display_name'] == 'New account')
        initial = status(recipient)
        assert initial['servers'] == [] and len(initial['server_states']) == 1
        invite = ipc(owner, 'create_server_invite', {'expires_in_minutes':30})
        assert re.fullmatch(re.escape(origin)+r'/[a-z]+(?:-[a-z]+){0,3}[0-9]{12}', invite['uri']) and invite['qr'].startswith('data:image/svg+xml;base64,')
        assert len(invite['uri']) < 150
        preview = helper(recipient, dict(action='preview_invite', invite_code=invite['uri']))
        assert preview['saved_account'] and preview['server_name'] and 'media_key' not in preview
        assert 'code' not in preview
        assert not request('/v1/snapshot', token=token)['server_member'], 'Preview must not grant membership'
        wrapped = 'wisp-invite:v2.' + base64.urlsafe_b64encode(invite['uri'].encode()).decode().rstrip('=')
        helper(recipient, dict(action='accept_invite', invite_code=wrapped))
        after = registry(recipient)
        assert after['servers'][0]['device_id'] == before['servers'][0]['device_id'], 'Reuse saved device'
        assert after['servers'][0]['media_key'] == 'test-only-shared-media-key'
        eventually(lambda: len(status(recipient)['servers']) == 1)
        joined = status(recipient)
        assert recipient_daemon.poll() is None and joined['self']['hangout_id'] is None
        assert not joined['self']['sharing'] and not joined['self']['media']['camera']['active']
        assert not joined['friends'], 'Server invite must not add the inviter as a friend'
        # A consumed or malformed link must not rewrite existing credentials.
        helper(recipient, dict(action='accept_invite', invite_code=wrapped), ok=False)
        helper(recipient, dict(action='preview_invite', invite_code='https://invalid.example/other/#v2.bad'), ok=False)
        assert registry(recipient) == after
        ipc(recipient, 'leave_server')
        eventually(lambda: status(recipient)['servers'] == [])
        assert recipient_daemon.poll() is None
        assert not request('/v2/accounts/me', token=token)['server_member']
        helper(recipient, dict(action='login', username='new-user', password='test-only-password-123', device_name='Second test device'))
        assert registry(recipient)['servers'][0]['media_key'] == 'test-only-shared-media-key'
        assert not request('/v1/snapshot', token=session(recipient))['server_member']
        with urllib.request.urlopen(origin+'/join/') as response:
            assert 'no-store' in response.headers['Cache-Control']
            assert "frame-ancestors 'none'" in response.headers['Content-Security-Policy']
            assert response.headers['Referrer-Policy'] == 'no-referrer'
        print('Server-free signup/login, encrypted preview, existing-account acceptance, live join/leave, key retention and no media publication passed')
    finally:
        if sys.exc_info()[0] is not None:
            for log in logs:
                log.flush()
                print(Path(log.name).name, Path(log.name).read_text()[-4000:], file=sys.stderr)
        for proc in reversed(processes):
            proc.terminate()
        for proc in reversed(processes):
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill(); proc.wait()
        for log in logs:
            log.close()
