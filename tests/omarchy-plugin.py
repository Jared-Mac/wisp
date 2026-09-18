#!/usr/bin/env python3
"""Exercise the public export boundary and untrusted release archive handling."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[1]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


exporter = module('exporter', REPO/'scripts/export-omarchy-plugin.py')
updater = module('updater', REPO/'packaging/omarchy-plugin/.github/update.py')


def archive(extra=None, commit='released'):
    files = {'manifest.json': '{"id":"dev.wisp"}', 'release.json': json.dumps({'commit':commit,'version':'0.1+test'}),
             'Panel.qml':'Item {}', 'app/WispBridge.qml':'Item {}', 'README.md':'public', 'LICENSE':'license',
             '.github/update.py':'must not replace publisher'}
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w:gz') as tar:
        for name, value in files.items():
            data = value.encode()
            item = tarfile.TarInfo('dev.wisp/'+name)
            item.size = len(data)
            tar.addfile(item, io.BytesIO(data))
        if extra:
            tar.addfile(extra, io.BytesIO(b''))
    return output.getvalue()


class PluginTests(unittest.TestCase):
    def test_committed_export_excludes_development_and_local_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)/'source'; root.mkdir()
            def git(*args):
                return subprocess.check_output(['git','-C',str(root),*args], stderr=subprocess.DEVNULL)
            git('init','-q'); git('config','user.name','Test'); git('config','user.email','test@example.invalid')
            files = {'quickshell/manifest.json':'{"id":"dev.wisp","version":"0.1.0"}',
                     'quickshell/app/assets/PRESENCE-ICONS-LICENSE.txt':'attribution',
                     'quickshell/Panel.qml':'committed', 'quickshell/app/WispBridge.qml':'public', 'LICENSE':'license',
                     'AGENTS.md':'private', 'docs/android-handoff.md':'private', '.github/workflows/private.yml':'private'}
            for name in exporter.PUBLIC:
                files['packaging/omarchy-plugin/'+name]='public'
            for name, value in files.items():
                p=root/name; p.parent.mkdir(parents=True, exist_ok=True); p.write_text(value)
            git('add','.'); git('commit','-qm','fixture')
            (root/'quickshell/Panel.qml').write_text('uncommitted')
            (root/'quickshell/app/local.qml').write_text('local')
            destination=Path(temporary)/'export'
            with patch.object(exporter,'REPO',root):
                commit=exporter.export('HEAD',destination)
                self.assertEqual((destination/'Panel.qml').read_text(),'committed')
                self.assertFalse((destination/'app/local.qml').exists())
                self.assertFalse((destination/'AGENTS.md').exists())
                self.assertEqual((destination/'app/assets/PRESENCE-ICONS-LICENSE.txt').read_text(),'attribution')
                self.assertFalse((destination/'docs').exists())
                self.assertEqual(json.loads((destination/'release.json').read_text())['commit'],commit)
                self.assertTrue(json.loads((destination/'manifest.json').read_text())['version'].endswith(commit[:12]))
                (root/'quickshell/Panel.qml').write_text('wisp-vps')
                git('add','.'); git('commit','-qm','private fixture')
                with self.assertRaisesRegex(ValueError,'Private material'):
                    exporter.export('HEAD',Path(temporary)/'rejected')
                self.assertFalse((Path(temporary)/'rejected').exists())
                (root/'quickshell/Panel.qml').unlink(); (root/'quickshell/Panel.qml').symlink_to('/etc/passwd')
                git('add','.'); git('commit','-qm','symlink fixture')
                with self.assertRaisesRegex(ValueError,'regular files'):
                    exporter.export('HEAD',Path(temporary)/'rejected')

    def test_archive_rejects_unsafe_paths_links_and_duplicates(self):
        for name, kind in [('dev.wisp/../escape',tarfile.REGTYPE),('/escape',tarfile.REGTYPE),
                           ('dev.wisp/app/link',tarfile.SYMTYPE),('dev.wisp/.git/config',tarfile.REGTYPE),
                           ('dev.wisp/Panel.qml',tarfile.REGTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                member=tarfile.TarInfo(name);member.type=kind;member.linkname='/etc/passwd'
                with self.assertRaises(ValueError):
                    updater.unpack(archive(member),Path(temporary))

    def test_publisher_waits_for_matching_client_and_preserves_workflow(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary); (root/'.github').mkdir(); (root/'.github/update.py').write_text('trusted')
            (root/'Panel.qml').write_text('previous'); (root/'obsolete.qml').write_text('old')
            for commit, expected in [('pending','previous'),('released','Item {}')]:
                data=archive(commit=commit)
                assets={updater.ARCHIVE:data,updater.ARCHIVE+'.sha256':hashlib.sha256(data).hexdigest().encode(),
                        'wisp-main-linux-x86_64.update.json':b'{"commit":"released"}'}
                with patch.object(updater,'ROOT',root), patch.object(updater,'download',side_effect=lambda name,*args:assets[name]), \
                     patch.object(updater.subprocess,'check_output',return_value=b'Panel.qml\0obsolete.qml\0.github/update.py\0'):
                    updater.main()
                self.assertEqual((root/'Panel.qml').read_text(),expected)
                self.assertEqual((root/'.github/update.py').read_text(),'trusted')
            self.assertFalse((root/'obsolete.qml').exists())
            assets[updater.ARCHIVE+'.sha256']=b'bad'
            with patch.object(updater,'download',side_effect=lambda name,*args:assets[name]), self.assertRaisesRegex(ValueError,'checksum'):
                updater.main()


if __name__ == '__main__':
    unittest.main()
