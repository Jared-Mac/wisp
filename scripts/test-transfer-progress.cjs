const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const progress = {};
vm.createContext(progress);
vm.runInContext(fs.readFileSync(path.join(__dirname, '../quickshell/app/TransferProgress.js'), 'utf8'), progress);
const event = (id, extra = {}) => ({id, direction:'upload', bytes:1, total:10, ...extra});
const start = 1_000_000;
let entries = {};
// Simulate an arbitrarily long stream of individually valid, small IPC events.
for (let index = 0; index < 20_000; index++) {
  entries = progress.receive(entries, event(String(index)), start);
  assert(Object.keys(entries).length <= 128, 'distinct IDs must never grow the retained map beyond 128');
}
assert.equal(Object.keys(entries).length, 128);
assert.equal(entries['upload:0'], undefined);
assert.equal(entries['upload:19999'].bytes, 1);
assert.equal(Object.keys(progress.prune(entries, start + 300_000)).length, 0, 'stalled entries expire without another event');

// Updating an existing entry makes it the most recent even within one clock tick.
entries = progress.receive(entries, event('19872', {bytes:2}), start);
entries = progress.receive(entries, event('next'), start);
assert.equal(entries['upload:19872'].bytes, 2);
assert.equal(entries['upload:19873'], undefined);
assert.equal(Object.keys(entries).length, 128);

entries = progress.receive({}, event('large', {bytes:3_000_000_000, total:6_000_000_000}), start);
entries = progress.receive(entries, event('large', {direction:'download', bytes:5}), start + 1);
assert.equal(Object.keys(entries).length, 2, 'upload and download IDs remain independent');
assert.equal(entries['upload:large'].bytes, 3_000_000_000, 'multi-gigabyte counters are not truncated to 32 bits');
entries = progress.receive(entries, event('large', {bytes:6_000_000_000, total:6_000_000_000}), start + 2);
assert.equal(entries['upload:large'].expires, start + 5002);
entries = progress.receive(entries, event('large', {bytes:6_000_000_000, total:6_000_000_000}), start + 4000);
assert.equal(entries['upload:large'].expires, start + 5002, 'repeated completion does not extend expiry');
assert.equal(progress.prune(entries, start + 5001)['upload:large'].bytes, 6_000_000_000);
assert.equal(progress.prune(entries, start + 5002)['upload:large'], undefined);
assert(progress.prune(entries, start + 5002)['download:large']);
assert.equal(Object.keys(progress.prune(entries, start + 300_001)).length, 0);

entries = progress.receive({}, event('empty', {bytes:0, total:0}), start);
assert.equal(Object.keys(progress.prune(entries, start + 5000)).length, 0, 'empty completed transfers expire too');
for (let index = 0; index < 20_000; index++) {
  entries = progress.receive(entries, event(String(index), {bytes:10}), start);
  assert(Object.keys(entries).length <= 128, 'completed IDs share the same hard cap');
}
assert.equal(Object.keys(progress.prune(entries, start + 5000)).length, 0);

// Large extra fields from valid transport frames must not become retained state.
entries = progress.receive({}, event('projection', {payload:'x'.repeat(64 * 1024), expires:Number.MAX_SAFE_INTEGER}), start);
assert.deepEqual(Object.keys(entries['upload:projection']).sort(), ['bytes', 'expires', 'total']);
assert.equal(entries['upload:projection'].expires, start + 300_000);
for (const invalid of [null, {}, event(''), event('x'.repeat(257)), event(7),
  event('bad', {direction:'other'}), event('bad', {direction:{}}),
  event('bad', {bytes:-1}), event('bad', {bytes:11}), event('bad', {bytes:1.5}),
  event('bad', {bytes:'1'}), event('bad', {total:NaN}), event('bad', {total:Infinity}),
  event('bad', {total:Number.MAX_SAFE_INTEGER + 1})]) {
  assert.equal(Object.keys(progress.receive({}, invalid, start)).length, 0);
}
console.log('Transfer progress: repeated-event cap, eviction, active/completed expiry, projection and validation passed');
