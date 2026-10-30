import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRemoteInventoryModel, INVENTORY_LIMITS } from '../preview/remote-inventory-model.mjs';
const fixture = JSON.parse(readFileSync(new URL('./fixtures/remote-inventory.v1.json', import.meta.url)));
const clone = value => structuredClone(value), json = value => JSON.stringify(value);
function setup(scope = fixture.scope) {
  let current = 1000000;
  const model = createRemoteInventoryModel({ now: () => current });
  assert.deepEqual(model.selectScope(json(scope)), { ok: true });
  return { model, at(value) { current = value; } };
}
function report(request, overrides = {}) {
  return { ...clone(fixture.pages[0]), adapter: clone(request.adapter), access: clone(request.access), binding: { ...request.binding }, sessionId: request.sessionId, snapshotId: request.snapshotId, requestId: request.requestId, afterCursor: request.afterCursor, ...overrides };
}
function deliver(model, overrides = {}) { const started = model.beginPage(); assert.equal(started.ok, true); return model.acceptPage(started.request, json(report(started.request, overrides))); }
function row(index) { return { ...clone(fixture.pages[0].items[0]), fileId: `file-${index}`, relativePath: `project-${index}/AGENTS.md` }; }

test('fixture pages preserve identities, native revision reports and separate observation/receipt clocks', () => {
  const { model } = setup();
  const first = model.beginPage(); assert.equal(model.acceptPage(first.request, json(fixture.pages[0])).ok, true);
  const second = model.beginPage(); assert.equal(second.request.afterCursor, 'cursor-1'); assert.equal(model.acceptPage(second.request, json(fixture.pages[1])).ok, true);
  const state = model.getState(); assert.equal(state.records.length, 3); assert.equal(state.mode, 'offline_unverified'); assert.equal(state.window, 'complete'); assert.equal(state.reportedCoverage, 'complete');
  assert.equal(state.pages[0].observedAtMs, 990000); assert.equal(state.pages[0].receivedAtMs, 1000000); assert.equal(state.pages[0].freshness, 'recent_report'); assert.equal(state.records[0].revisionId, 'native-revision-1');
  assert.equal(model.beginPage().error.code, 'complete'); assert.match(state.provenance, /No connection, authenticated identity, approval or execution/);
});

test('operation coverage is independent of reported provider capabilities', () => {
  const scope = clone(fixture.scope); scope.adapter.operations.push('source.inspect', 'history.inspect');
  const { model } = setup(scope); assert.equal(model.beginPage('source.inspect').error.code, 'unsupported'); assert.equal(model.beginPage('execute').error.code, 'unsupported'); assert.equal(model.getState().records.length, 0);
});

test('every binding component, session and snapshot is exact and cross-host cursors cannot be imported', () => {
  for (const key of Object.keys(fixture.scope.binding)) {
    const { model } = setup(); const request = model.beginPage().request; const page = report(request); page.binding[key] += '-other';
    assert.equal(model.acceptPage(request, json(page)).error.code, 'scope_mismatch', key); assert.equal(model.getState().records.length, 0);
  }
  for (const key of ['sessionId', 'snapshotId']) {
    const { model } = setup(); const request = model.beginPage().request; assert.equal(model.acceptPage(request, json(report(request, { [key]: 'other' }))).error.code, 'scope_mismatch');
  }
});

test('cursor skip, rewind, self-loop and consumed request replay fail atomically', () => {
  const { model } = setup(); const first = model.beginPage().request;
  assert.equal(model.acceptPage(first, json(report(first))).ok, true);
  assert.equal(model.acceptPage(first, json(report(first))).error.code, 'stale_request');
  for (const overrides of [{ afterCursor: null }, { nextCursor: 'cursor-1' }, { requestId: 999 }]) {
    const request = model.beginPage().request;
    assert.equal(model.acceptPage(request, json(report(request, overrides))).error.code, 'cursor_conflict'); assert.equal(model.getState().records.length, 2);
  }
  assert.equal(deliver(model, { items: [row(3)], nextCursor: 'cursor-2' }).ok, true);
  assert.equal(deliver(model, { items: [row(4)], nextCursor: 'cursor-1' }).error.code, 'cursor_conflict');
});

test('a request token is instance-bound; a clone cannot consume or replace the admitted request', () => {
  const { model } = setup(); const request = model.beginPage().request;
  assert.equal(model.beginPage().error.code, 'busy'); assert.equal(model.acceptPage(clone(request), json(report(request))).error.code, 'stale_request');
  assert.equal(model.getState().pending, true); assert.equal(model.acceptPage(request, json(report(request))).ok, true);
});

test('duplicate file identity and path within/across pages preserve accepted metadata', () => {
  for (const variant of ['id', 'path', 'within']) {
    const { model } = setup(); assert.equal(deliver(model).ok, true); const request = model.beginPage().request;
    const bad = variant === 'id' ? { ...row(3), fileId: 'file-1' } : variant === 'path' ? { ...row(3), relativePath: 'AGENTS.md' } : row(3);
    assert.equal(model.acceptPage(request, json(report(request, { items: variant === 'within' ? [bad, clone(bad)] : [row(4), bad], nextCursor: null }))).error.code, 'duplicate');
    assert.deepEqual(model.getState().records, fixture.pages[0].items);
  }
});

test('relative-path grammar refuses traversal, drives, encoding, unsafe names and unsupported files', () => {
  for (const relativePath of [...fixture.unsafePaths, 'a/\u202eb.md', 'a/\ud800.md', 'a/'.repeat(600) + 'a.md']) {
    const { model } = setup(); const request = model.beginPage().request;
    assert.equal(model.acceptPage(request, json(report(request, { items: [{ ...row(1), relativePath }] }))).error.code, 'invalid', relativePath);
  }
});

test('Unicode/case distinctions are preserved exactly, never used as filesystem aliases', () => {
  const { model } = setup(); const names = ['Skills.md', 'skills.md', 'café.md', 'cafe\u0301.md', '.cursor/rules/example.MD'];
  assert.equal(deliver(model, { items: names.map((relativePath, i) => ({ ...row(i), relativePath })), nextCursor: null }).ok, true);
  assert.deepEqual(model.getState().records.map(item => item.relativePath), names);
});

test('strict recursive schemas reject source payloads, role assertions and prototype-like keys', () => {
  for (const mutate of [p => p.content = 'secret source', p => p.binding.role = 'admin', p => p.items[0].content = 'source', p => p.items[0].constructor = 'spoof', p => Object.defineProperty(p.binding, '__proto__', { value: 'spoof', enumerable: true }), p => p.provenance = { trusted: true }]) {
    const { model } = setup(); const request = model.beginPage().request; const page = report(request); mutate(page);
    assert.equal(model.acceptPage(request, json(page)).error.code, 'invalid'); assert.equal(model.getState().records.length, 0);
  }
});

test('duplicate JSON keys including escaped aliases, malformed JSON and excessive depth are rejected', () => {
  const { model } = setup();
  for (const payload of ['null', '{}', '[]', '{bad}', ' '.repeat(131073), '{"x":1,"\\u0078":2}', '['.repeat(17) + '0' + ']'.repeat(17)]) {
    const request = model.beginPage().request; assert.equal(model.acceptPage(request, payload).error.code, 'invalid');
  }
  const request = model.beginPage().request; const valid = json(report(request));
  assert.equal(model.acceptPage(request, valid.replace('"requestId":', '"requestId":99,"requestId":')).error.code, 'invalid');
});

test('numeric and digest boundaries reject unsafe, negative and malformed metadata', () => {
  for (const [key, values] of [['byteLength', [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, Infinity, '333']], ['modifiedAtMs', [-1, 1.1, 8640000000000001, 'today']], ['sha256', ['a'.repeat(63), 'A'.repeat(64), '<hash>', 1]], ['revisionId', ['', {}, 'parent/rev']]]) {
    for (const value of values) {
      const { model } = setup(); assert.equal(deliver(model, { items: [{ ...row(1), [key]: value }] }).error.code, 'invalid', `${key}:${value}`);
    }
  }
  const { model } = setup(); assert.equal(deliver(model, { items: [{ ...row(1), byteLength: Number.MAX_SAFE_INTEGER, modifiedAtMs: null, sha256: null, revisionId: null }], nextCursor: null }).ok, true);
});

test('record/page caps bound working memory and never imply complete inventory', () => {
  const { model } = setup();
  for (let page = 0; page < 20; page++) assert.equal(deliver(model, { items: Array.from({ length: 50 }, (_, i) => row(page * 50 + i)), nextCursor: `cursor-${page}` }).ok, true);
  const state = model.getState(); assert.equal(state.records.length, 1000); assert.equal(state.window, 'capacity'); assert.equal(state.finished, false); assert.equal(model.beginPage().error.code, 'capacity');
  const empty = setup().model;
  for (let page = 0; page < INVENTORY_LIMITS.pages; page++) assert.equal(deliver(empty, { items: [], nextCursor: `cursor-${page}` }).ok, true);
  assert.equal(empty.getState().pages.length, 64); assert.equal(empty.getState().window, 'capacity'); assert.equal(empty.beginPage().error.code, 'capacity');
});

test('oversized pages reject as a unit without truncation', () => {
  const { model } = setup(); assert.equal(deliver(model, { items: Array.from({ length: 51 }, (_, i) => row(i)) }).error.code, 'invalid'); assert.equal(model.getState().records.length, 0);
});

test('terminal paging and reported completeness are independent, consistent snapshot facts', () => {
  for (const coverage of ['partial', 'unknown', 'complete']) {
    const { model } = setup(); assert.equal(deliver(model, { items: [], nextCursor: null, coverage }).ok, true);
    const state = model.getState(); assert.equal(state.window, 'complete'); assert.equal(state.reportedCoverage, coverage); assert.equal(state.mode, 'offline_unverified');
  }
  const { model } = setup(); assert.equal(deliver(model, { coverage: 'partial' }).ok, true);
  assert.equal(deliver(model, { coverage: 'complete', items: [row(3)], nextCursor: null }).error.code, 'snapshot_conflict'); assert.equal(model.getState().window, 'partial'); assert.equal(model.getState().reportedCoverage, 'partial');
});

test('stale, unknown and skewed producer observations stay separate from receipt age', () => {
  for (const [observedAtMs, expected] of [[null, 'unknown'], [100, 'stale'], [1000001, 'clock_skew'], [1500000, 'clock_skew'], [990000, 'recent_report']]) {
    const { model } = setup(); assert.equal(deliver(model, { observedAtMs }).ok, true); assert.equal(model.getState().pages[0].freshness, expected);
  }
  const { model, at } = setup(); assert.equal(deliver(model, { observedAtMs: 1000000 }).ok, true); at(1400001); assert.equal(model.getState().pages[0].freshness, 'stale');
});

test('expiry purges retained metadata and fences late callbacks at exact grant/consent deadlines', () => {
  for (const field of ['grantExpiresAtMs', 'consentExpiresAtMs']) {
    const scope = clone(fixture.scope); scope.access[field] = 1000100; const { model, at } = setup(scope); assert.equal(deliver(model).ok, true); const request = model.beginPage().request;
    at(1000100); assert.equal(model.acceptPage(request, json(report(request))).error.code, 'expired'); const state = model.getState(); assert.equal(state.status, 'expired'); assert.equal(state.scope, null); assert.equal(state.records.length, 0);
  }
});

test('clock rollback and invalid local time clear scope rather than extend a reported lease', () => {
  for (const value of [999999, NaN, Infinity, -1]) {
    const { model, at } = setup(); assert.equal(deliver(model).ok, true); at(value); assert.equal(model.beginPage().error.code, 'clock_changed'); assert.equal(model.getState().records.length, 0);
  }
});

test('lock, disconnect, revocation, cancellation and identity changes purge/fence the entire view', () => {
  for (const reason of ['locked', 'disconnected', 'revoked', 'cancelled', 'identity_changed']) {
    const { model } = setup(); assert.equal(deliver(model).ok, true); const request = model.beginPage().request; model.invalidate(reason);
    assert.equal(model.getState().status, reason); assert.equal(model.getState().scope, null); assert.equal(model.getState().pages.length, 0); assert.equal(model.acceptPage(request, json(report(request))).ok, false);
    assert.equal(model.selectScope(json(fixture.scope)).ok, true); assert.equal(model.acceptPage(request, json(report(request))).error.code, 'stale_request');
  }
});

test('new enrollment/snapshot and multiple hosts never reuse old inventory or request custody', () => {
  const { model } = setup(); const old = model.beginPage().request;
  for (const host of ['host-c', 'host-d']) {
    const scope = clone(fixture.scope); scope.binding.installationId = host; scope.binding.installationEpoch = 'new-enrollment'; scope.snapshotId = 'new-snapshot';
    assert.equal(model.selectScope(json(scope)).ok, true); const request = model.beginPage().request;
    assert.equal(model.acceptPage(old, json(report(old))).error.code, 'stale_request'); assert.equal(model.acceptPage(request, json(report(request, { nextCursor: null }))).ok, true);
    assert.equal(model.getState().scope.binding.installationId, host); assert.equal(model.getState().records.length, 2);
  }
});

test('scope validation refuses spoofed authority, empty scope, expired grants and incompatible versions', () => {
  for (const mutate of [s => s.version = 'other.v1', s => s.adapter.version = '', s => s.adapter.operations = [], s => s.adapter.operations = ['source.inspect'], s => s.authenticated = true, s => s.binding.admin = true, s => s.access.grantExpiresAtMs = 1000000, s => s.access.consentExpiresAtMs = 999999]) {
    const { model } = setup(); const scope = clone(fixture.scope); mutate(scope); assert.equal(model.selectScope(json(scope)).ok, false); assert.equal(model.getState().scope.binding.installationId, 'host-b');
  }
});

test('public snapshots are defensive copies and disposal is terminal', () => {
  const { model } = setup(); assert.equal(deliver(model).ok, true); const state = model.getState(); state.records[0].relativePath = 'changed.md'; state.scope.binding.rootId = 'other'; state.pages[0].coverage = 'complete';
  assert.equal(model.getState().records[0].relativePath, 'AGENTS.md'); assert.equal(model.getState().scope.binding.rootId, 'root-1');
  model.dispose(); assert.equal(model.selectScope(json(fixture.scope)).error.code, 'unavailable'); assert.equal(model.getState().status, 'disposed'); assert.equal(model.getState().records.length, 0);
});


test('adapter pin and explicit grant/consent scopes bind every page independently of capabilities', () => {
  for (const [part, key, value] of [['adapter', 'id', 'other-adapter'], ['adapter', 'product', 'Other product'], ['adapter', 'version', '0.2.0'], ['access', 'consentId', 'other-consent'], ['access', 'grantExpiresAtMs', 2100000], ['access', 'consentExpiresAtMs', 1900000]]) {
    const { model } = setup(); const request = model.beginPage().request; const page = report(request); page[part][key] = value;
    assert.equal(model.acceptPage(request, json(page)).error.code, 'scope_mismatch'); assert.equal(model.getState().records.length, 0);
  }
  for (const mutate of [s => s.access.grantOperations = ['source.inspect'], s => s.access.consentOperation = 'source.inspect', s => delete s.access.consentId, s => s.access.grantOperations = []]) {
    const { model } = setup(); const scope = clone(fixture.scope); mutate(scope); assert.equal(model.selectScope(json(scope)).error.code, 'invalid');
  }
});

test('payload limits measure UTF-8 bytes and accept an exact bounded ASCII payload', () => {
  const { model } = setup(); const request = model.beginPage().request; const payload = json(report(request, { nextCursor: null }));
  assert.equal(model.acceptPage(request, payload + ' '.repeat(INVENTORY_LIMITS.payloadBytes - payload.length)).ok, true);
  const next = setup().model; const token = next.beginPage().request;
  const page = report(token, { items: Array.from({ length: 50 }, (_, i) => ({ ...row(i), relativePath: `${'界'.repeat(220)}/${'界'.repeat(220)}/${'界'.repeat(220)}/${'界'.repeat(220)}/file-${i}.md` })) });
  const encoded = json(page); assert.ok(encoded.length < INVENTORY_LIMITS.payloadBytes); assert.ok(Buffer.byteLength(encoded) > INVENTORY_LIMITS.payloadBytes);
  assert.equal(next.acceptPage(token, encoded).error.code, 'invalid'); assert.equal(next.getState().records.length, 0);
});
