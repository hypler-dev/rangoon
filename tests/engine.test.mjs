import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createEngineController, validateEngineStatus } from '../preview/engine-model.mjs';
import { renderEngineView } from '../preview/engine-view.mjs';

const fixture = JSON.parse(await readFile(new URL('../fixtures/contracts/engine-status.json', import.meta.url), 'utf8'));
const status = () => structuredClone(fixture);

test('browser and loading views disable checks and keep runtime unknown', async () => {
  const browser = createEngineController();
  const browserView = renderEngineView(browser.getState());
  assert.match(browserView, /id="engine-check"[^>]*disabled/);
  assert.match(browserView, /Browser only; no native result/);
  assert.match(browserView, /Not checked; unknown/);
  let resolve;
  const native = createEngineController({ invoke: () => new Promise(done => { resolve = done; }) });
  const pending = native.check();
  const loading = renderEngineView(native.getState());
  assert.match(loading, /aria-busy="true"/);
  assert.match(loading, /id="engine-check"[^>]*disabled/);
  resolve(status());
  await pending;
  const checked = renderEngineView(native.getState());
  assert.match(checked, /Local native response/);
  assert.match(checked, /Not checked; unknown/);
  assert.equal((checked.match(/<button\b/g) ?? []).length, 1);
  assert.doesNotMatch(checked, /id="engine-check"[^>]*disabled/);
});

test('failed rendered status offers only a local retry and escapes untrusted text', async () => {
  const controller = createEngineController({ invoke: async () => ({ ...status(), executionAuthority: 'granted' }) });
  await controller.check();
  const state = controller.getState();
  const html = renderEngineView(state);
  assert.match(html, /Integration status was not accepted/);
  assert.match(html, /Local response unavailable or not accepted; host state unknown/);
  assert.doesNotMatch(html, /No local check requested/);
  assert.match(html, /Capabilities stay unavailable/);
  assert.doesNotMatch(html, /Local native response/);
  assert.doesNotMatch(html, /id="engine-check"[^>]*disabled/);
  const escaped = renderEngineView({ ...state, message: '<img src=x onerror=alert(1)>' });
  assert.doesNotMatch(escaped, /<img src=x/);
  assert.match(escaped, /&lt;img src=x onerror=alert\(1\)&gt;/);
});

test('missing bridge stays unavailable and makes no native call', async () => {
  const controller = createEngineController();
  await controller.check();
  assert.equal(controller.getState().status, 'unavailable');
  assert.equal(controller.getState().result, null);
  assert.match(controller.getState().message, /Installation and runtime remain unknown/);
});

test('requests the exact no-argument native status command and accepts its fixed status shape', async () => {
  const calls = [];
  const controller = createEngineController({ invoke: (...args) => {
    calls.push(args);
    return Promise.resolve(status());
  } });
  await controller.check();
  assert.deepEqual(calls, [['get_engine_status']]);
  assert.equal(controller.getState().status, 'unavailable');
  assert.deepEqual(controller.getState().result, status());
});

test('bridge errors and malformed native data fail closed', async () => {
  const throwing = createEngineController({ invoke: () => Promise.reject(new Error('bridge down')) });
  await throwing.check();
  assert.equal(throwing.getState().status, 'failed');
  assert.equal(throwing.getState().result, null);

  const malformed = createEngineController({ invoke: () => Promise.resolve({ schemaVersion: 'rangoon.engine-status.v0' }) });
  await malformed.check();
  assert.equal(malformed.getState().status, 'failed');
  assert.equal(malformed.getState().result, null);
});

test('future schemas, claimed readiness, and invalid capability shapes are rejected', () => {
  const future = status();
  future.schemaVersion = 'rangoon.engine-status.v1';
  assert.equal(validateEngineStatus(future), null);

  const ready = status();
  ready.runtimeState = 'ready';
  assert.equal(validateEngineStatus(ready), null);

  const invalid = status();
  invalid.capabilities[0] = { operation: 'negotiate_contract', state: 'available', reason: 'adapter_not_implemented' };
  assert.equal(validateEngineStatus(invalid), null);
});

test('a late refresh cannot replace a newer accepted result', async () => {
  let first;
  let second;
  const controller = createEngineController({ invoke: () => new Promise(resolve => {
    if (!first) first = resolve;
    else second = resolve;
  }) });
  const older = controller.check();
  const newer = controller.check();
  const accepted = status();
  second(accepted);
  await newer;
  const stale = status();
  stale.reference.productVersion = '9.9.9';
  first(stale);
  await older;
  assert.equal(controller.getState().status, 'unavailable');
  assert.deepEqual(controller.getState().result, accepted);
});

test('the status reader rejects authority promotion, duplicates and missing boundary fields', () => {
  for (const mutate of [
    value => { value.executionAuthority = 'granted'; },
    value => { value.networkAttempted = true; },
    value => { value.reference.releaseQualified = true; },
    value => { delete value.installedVersion; },
    value => { value.capabilities[1] = value.capabilities[0]; },
    value => { value.executionAuthorized = true; },
  ]) {
    const candidate = status();
    mutate(candidate);
    assert.equal(validateEngineStatus(candidate), null);
  }
});
