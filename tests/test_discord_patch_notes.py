import importlib.util
import json
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location('patch_notes', Path(__file__).parents[1] / 'scripts/discord-patch-notes.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PatchNotesTests(unittest.TestCase):
    def test_payload_is_bounded_disables_mentions_and_does_not_claim_deployment(self):
        body = module.payload('@everyone\n' + 'A' * 5000, 'example/wisp', 'a' * 40)
        self.assertEqual(body['allowed_mentions'], {'parse': []})
        self.assertLessEqual(len(body['embeds'][0]['description']), 4096)
        self.assertEqual(body['embeds'][0]['title'], 'Wisp · merged to main')
        self.assertNotIn('deployed', json.dumps(body))

    def test_added_release_notes_take_precedence_over_commit_subjects(self):
        with patch.object(module, 'git', side_effect=['docs/patch-notes/new.md', 'User-facing changes']) as git:
            self.assertEqual(module.notes('a' * 40, 'b' * 40), ('User-facing changes', False))
            self.assertEqual(git.call_count, 2)
        with patch.object(module, 'git', side_effect=['', 'Fix voice\nFix voice\nFix chat']):
            self.assertEqual(module.notes('a' * 40, 'b' * 40), ('- Fix voice\n- Fix chat', False))

    def test_here_requires_explicit_note_metadata_and_only_notifies_once(self):
        documents = ['docs/patch-notes/a.md\ndocs/patch-notes/b.md',
                     module.NOTIFY_HERE + '\n# Android test\nInstall steps.',
                     module.NOTIFY_HERE + '\nMore changes. @everyone <@123>']
        with patch.object(module, 'git', side_effect=documents):
            body, notify = module.notes('a' * 40, 'b' * 40)
        self.assertTrue(notify)
        self.assertNotIn(module.NOTIFY_HERE, body)
        message = module.payload(body, 'example/wisp', 'b' * 40, notify_here=notify)
        self.assertEqual(message['content'], '@here')
        self.assertEqual(message['allowed_mentions'], {'parse': ['everyone']})
        with patch.object(module, 'git', side_effect=['docs/patch-notes/a.md', '# Ordinary notes\n' + module.NOTIFY_HERE]):
            self.assertFalse(module.notes('a' * 40, 'b' * 40)[1])
        with patch.object(module, 'git', side_effect=['', module.NOTIFY_HERE]):
            self.assertFalse(module.notes('a' * 40, 'b' * 40)[1])
        self.assertNotIn('content', module.payload('@here\n@everyone', 'example/wisp', 'b' * 40))

    def test_here_confirmation_failure_does_not_resend_delivered_notes(self):
        message = module.payload('Install steps', 'example/wisp', 'a' * 40, notify_here=True)
        with patch.object(module.urllib.request, 'urlopen', return_value=io.BytesIO(b'{"id":"synthetic","mention_everyone":false}')) as network:
            with self.assertRaisesRegex(RuntimeError, 'delivered the notes'):
                module.post('https://discord.com/api/webhooks/1/token', message)
            self.assertEqual(network.call_count, 1)
        with patch.object(module.urllib.request, 'urlopen', return_value=io.BytesIO(b'{"id":"synthetic","mention_everyone":true}')):
            module.post('https://discord.com/api/webhooks/1/token', message)

    def test_bad_destinations_are_rejected_before_network_access(self):
        with patch.object(module.urllib.request, 'urlopen') as network:
            for url in ['http://discord.com/api/webhooks/1/token', 'https://example.com/api/webhooks/1/token', 'https://discord.com@example.com/api/webhooks/1/token']:
                with self.assertRaises(ValueError):
                    module.post(url, {})
            network.assert_not_called()

    def test_network_errors_do_not_expose_the_secret(self):
        with patch.object(module.urllib.request, 'urlopen', side_effect=urllib.error.URLError('secret URL here')):
            with self.assertRaises(RuntimeError) as error:
                module.post('https://discord.com/api/webhooks/1/token', {})
        self.assertNotIn('secret', str(error.exception))
        self.assertNotIn('token', str(error.exception))


if __name__ == '__main__':
    unittest.main()
