const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const markup = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../quickshell/app/ChatMarkup.js'), 'utf8'), markup);
const uri = 'https://wisp.you/fixture123456789012';
const payload = `Server invitation: Test community\nExpires: 2099-01-01T00:00:00Z\n${uri}`;
const parse = (text, type='text/plain') => markup.serverInvitation({content_type:type,payload:text});
assert.equal(parse(payload).uri, uri);
assert.equal(parse(payload).server_name, 'Test community');
assert.equal(parse(payload.replace(uri, 'https://example.test/join/#v2.'+'a'.repeat(43))).server_name, 'Test community');
for (const invalid of [payload.replace(uri,'https://example.test/phishing'), payload.replace(uri,'javascript:alert(1)'),
  payload+'\nadditional text', payload.replace('2099-01-01T00:00:00Z','invalid date'), 'ordinary chat message']) {
  assert.equal(parse(invalid), null);
}
assert.equal(parse(payload,'application/octet-stream'), null);
assert.equal(parse({text:payload}), null);
assert.equal(markup.serverInvitation(null), null);
console.log('PASS signed text invitation recognition and ordinary-message compatibility');
