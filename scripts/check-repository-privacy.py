#!/usr/bin/env python3
"""Reject private admin/account artifacts in the Git index without logging values.

This is a targeted publication check, not a replacement for a full secret scan.
Read staged blobs so unstaged edits cannot hide a sensitive staged version.
"""
from pathlib import PurePosixPath
import re
import subprocess
import sys

def private_path(name):
    path = PurePosixPath(name)
    if path.parts[0] in ('forge', '.private'):
        return True
    if len(path.parts) > 1 and path.parts[0] == 'output' and path.parts[1].startswith(('marketing', 'instagram')):
        return True
    base = path.name.lower()
    if base in ('.env.example', '.env.sample'):
        return False
    return (base == '.env' or base.startswith('.env.')
            or base in ('id_rsa', 'id_ed25519', 'id_ecdsa')
            or (base.endswith(('.json', '.txt')) and base.startswith(('cookies', 'storage-state', 'storage_state', 'credentials', 'secrets'))))

PATTERNS = {
    'private key': re.compile(rb'-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----'),
    'Meta account identifier': re.compile(rb'(?:business_id|asset_id)["\x27]?\s*[=:]\s*["\x27]?[0-9]{8,}', re.I),
    'Meta account configuration': re.compile(rb'MARKETING_META_(?:BUSINESS|ASSET)_ID["\x27]?\s*[=:]\s*["\x27]?[0-9]+', re.I),
    'social credential': re.compile(rb'(?:instagram|facebook|meta)[_ -]?(?:password|access[_ -]?token)["\x27]?\s*[=:]\s*["\x27][A-Za-z0-9_-]{12,}["\x27]', re.I),
}

def main():
    entries = subprocess.check_output(['git', 'ls-files', '-s', '-z']).split(b'\0')
    failures = []
    checked = 0
    for entry in entries:
        if not entry:
            continue
        metadata, raw_name = entry.split(b'\t', 1)
        mode, oid, stage = metadata.split()
        name = raw_name.decode('utf-8', errors='replace')
        checked += 1
        if private_path(name):
            failures.append((name, 'private path'))
            continue
        if mode == b'160000':
            continue
        data = subprocess.check_output(['git', 'cat-file', 'blob', oid])
        for label, pattern in PATTERNS.items():
            if pattern.search(data):
                failures.append((name, label))
    for name, label in failures:
        print(f'{name}: {label} must stay outside the source repository', file=sys.stderr)
    print(f'Checked {checked} indexed files; {len(failures)} privacy findings.')
    return bool(failures)

if __name__ == '__main__':
    sys.exit(main())
