#!/usr/bin/env python3
"""Exercise plugin lifecycle and encrypted pack sharing with two isolated clients.
Build release wispd, wisp-server and the Valheim guest first. --online additionally
resolves the pinned BepInEx package from Thunderstore and installs a temp profile.
No real accounts, game processes, or installed profiles are used.
"""
import argparse,json,os,socket,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--online',action='store_true')
args=parser.parse_args()
processes=[]
with tempfile.TemporaryDirectory(prefix='wisp-plugins-test-') as temporary:
    root=Path(temporary)
    env={k:v for k,v in os.environ.items() if not k.startswith('WISP_')}
    for key in ['DBUS_SESSION_BUS_ADDRESS','DISPLAY','WAYLAND_DISPLAY']:env.pop(key,None)
    env.update(XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),WISP_E2EE_KEY='isolated-plugin-test-encryption-key',RUST_LOG='error')
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    url=f'http://127.0.0.1:{port}'
    def start(name,command,extra):
        log=(root/(name+'.log')).open('w')
        p=subprocess.Popen([str(v) for v in command],env=env|extra,stdout=log,stderr=subprocess.STDOUT,cwd=ROOT)
        log.close();processes.append(p)
    def call(who,name,values=None):
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(320);sock.connect(str(root/(who+'.sock')))
            sock.sendall((json.dumps({'v':1,'type':'command','id':'plugin-fixture','name':name,'args':values or {}})+'\n').encode())
            with sock.makefile() as source:
                for line in source:
                    value=json.loads(line)
                    if value.get('id')=='plugin-fixture' and value.get('type')=='result':
                        if not value['ok']:raise RuntimeError(value.get('error',{}).get('message','command failed'))
                        return value.get('value')
            raise RuntimeError('No command result')
    def wait(description,fn):
        deadline=time.monotonic()+30
        while time.monotonic()<deadline:
            if any(p.poll() is not None for p in processes):raise RuntimeError('Fixture process exited: '+description)
            try:
                result=fn()
                if result:return result
            except (OSError,RuntimeError,ValueError):pass
            time.sleep(.1)
        raise RuntimeError('Timed out: '+description)
    def rejected(fn):
        try:fn()
        except RuntimeError:return
        raise AssertionError('Expected rejection')
    try:
        start('server',[ROOT/'target/release/wisp-server','--addr',f'127.0.0.1:{port}','--database-url',f'sqlite://{root}/server.sqlite3'],{})
        time.sleep(.3)
        for who,profile in [('owner','Owner'),('member','MemberA')]:
            (root/who).mkdir()
            start(who,[ROOT/'target/release/wispd','--server-url',url,'--profile',profile,'--socket',root/(who+'.sock'),'--disable-media','--disable-surfaces'],{'XDG_CONFIG_HOME':str(root/who/'config'),'XDG_DATA_HOME':str(root/who/'data'),'WISP_ACCOUNTS_FILE':str(root/who/'accounts.json'),'WISP_REQUIRE_CHAT_E2EE':'true'})
            wait(who,lambda:call(who,'privacy_status').get('configured'))
        conversation=call('owner','open_direct',{'friend':'MemberA'})['id']
        wait('conversation',lambda:any(c['id']==conversation for c in call('member','status')['conversations']))
        package=str(ROOT/'target/plugins/games.valheim')
        for who in ['owner','member']:
            review=call(who,'plugin_inspect',{'path':package})
            rejected(lambda:call(who,'plugin_install',{'path':package,'digest':'unreviewed'}))
            call(who,'plugin_install',{'path':package,'digest':review['digest']})
        base={'plugin_id':'games.valheim','conversation_id':conversation}
        opened=call('owner','plugin_open',base)
        assert opened['view']['title']=='Valheim Modpacks'
        rejected(lambda:call('owner','plugin_action',base|{'generation':opened['generation']+1,'action':'save','fields':{}}))
        rejected(lambda:call('owner','plugin_open',base|{'conversation_id':'not-a-member'}))
        if args.online:
            opened=call('owner','plugin_action',base|{'generation':opened['generation'],'action':'save','fields':{'title':'Our Valheim modpack','packages':'denikson-BepInExPack_Valheim-5.4.2351\nValheimModding-Jotunn-2.30.2'}})
            opened=call('owner','plugin_resolve',base|{'generation':opened['generation']})
            assert len(opened['view']['files'])>=2
            assert any(f['package']=='denikson-BepInExPack_Valheim-5.4.2351' for f in opened['view']['files'])
            parent=root/'profiles';parent.mkdir()
            opened=call('owner','plugin_apply',base|{'generation':opened['generation'],'digest':opened['digest'],'path':str(parent)})
            folder=Path(opened['folder']);assert (folder/'BepInEx/core/BepInEx.dll').is_file()
            assert (folder/'start_game_bepinex.sh').is_file()
            assert any((folder/'BepInEx/plugins/ValheimModding-Jotunn').rglob('*.dll'))
            opened=call('owner','plugin_share',base|{'generation':opened['generation']})
            def shared():
                view=call('member','plugin_open',base)
                return view if view['shared'] else None
            incoming=wait('encrypted shared pack',shared)
            imported=call('member','plugin_import',base|{'generation':incoming['generation'],'message_id':incoming['shared'][0]['id']})
            assert imported['view']['files']==opened['view']['files']
            assert imported['digest']==opened['digest']
            other=call('member','plugin_open',{'plugin_id':'games.valheim'})
            assert other['view']['files']==[], 'Conversation state leaked into a personal draft'
            # Server only stores the encrypted attachment, not pack contents.
            assert b'Our Valheim modpack' not in (root/'server.sqlite3').read_bytes()
            print('PASS: Thunderstore pin -> verified profile -> encrypted share -> peer import; scope isolation',flush=True)
        call('owner','plugin_enable',{'plugin_id':'games.valheim','enabled':False})
        rejected(lambda:call('owner','plugin_open',base))
        call('owner','plugin_enable',{'plugin_id':'games.valheim','enabled':True})
        call('owner','plugin_remove',{'plugin_id':'games.valheim'})
        assert call('owner','plugin_list')['plugins']==[]
        for who in ['owner','member']:
            state=call(who,'status')['self']
            assert not state['hangout_id'] and not state['sharing'] and not state['media']['microphone_published']
        print('PASS: reviewed installation, stale-state rejection, disable/enable/remove, no voice or game launch',flush=True)
    except Exception:
        for log in root.glob('*.log'):
            print(log.name+': '+log.read_text()[-4000:])
        raise
    finally:
        for process in reversed(processes):
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
