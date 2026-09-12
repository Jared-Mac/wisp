'use strict';
fetch('/android-test/release.json', {cache:'no-store', credentials:'omit'})
  .then(response => {if (!response.ok) throw new Error('pending'); return response.json();})
  .then(release => {
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*\.apk$/.test(release.filename)
        || !/^[0-9a-f]{64}$/.test(release.sha256)
        || typeof release.version_name !== 'string'
        || !Number.isSafeInteger(release.version_code)
        || release.debuggable !== false) throw new Error('pending');
    const button = document.getElementById('download');
    button.href = '/android-test/downloads/' + encodeURIComponent(release.filename);
    button.download = release.filename;
    button.hidden = false;
    document.getElementById('release').textContent = 'Version ' + release.version_name + ' · ' + (Number(release.size_bytes)/1048576).toFixed(1) + ' MB';
    document.getElementById('version').textContent = release.version_name + ' (build ' + release.version_code + ')';
    document.getElementById('compatibility').textContent = 'Android ' + release.min_android + ' or later';
    document.getElementById('checksum').textContent = release.sha256;
    document.getElementById('details').hidden = false;
  })
  .catch(() => {document.getElementById('error').textContent = 'A verified download will appear here when ready.';});
