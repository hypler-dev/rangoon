import test from 'node:test';
import assert from 'node:assert/strict';
import { createModelAssistanceController, DEFAULT_LOCAL_MODEL_PROFILE } from '../preview/model-assistance-model.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const runId = `run:${'2'.repeat(64)}`;
const preparedId = `prepared:${'3'.repeat(64)}`;
const requestId = '4'.repeat(64);
const common = (outcome, extra = {}) => ({ schemaVersion: 'rangoon.local-session-result.v1', generation: '1', runId: null, authority: 'none', outcome, ...extra });
const profile = { ...DEFAULT_LOCAL_MODEL_PROFILE, model: 'synthetic:v1' };
const profileRecord = () => ({ config: { schemaVersion: 'rangoon.local-profile.v1', adapter: 'ollama-loopback.v1', ...profile }, profileSha256: 'b'.repeat(64) });
const session = (active = null, configured = false) => ({ generation: '1', profile: configured ? profileRecord() : null, active, persistence: 'session_only', processingLocation: 'unknown', retention: 'unknown', authority: 'none' });
const clearedSession = active => ({ generation: '2', profile: null, active, persistence: 'session_only', processingLocation: 'unknown', retention: 'unknown', authority: 'none' });
const bodyJson = JSON.stringify({ task: 'classify_v1', target: { model: profile.model, maxOutputTokens: profile.maxOutputTokens } });
const wireJson = JSON.stringify({model:profile.model,messages:[{role:"system",content:"Synthetic fixed instructions"},{role:"user",content:bodyJson}],options:{temperature:0,num_predict:profile.maxOutputTokens},stream:false,think:false,format:"json"});
const prepared = () => ({ generation: '1', preparedId, requestId, origin: 'http://127.0.0.1:11434', model: profile.model, bodyJson: wireJson, bodyBytes: new TextEncoder().encode(wireJson).length, inputs: [{ input: { kind: 'source', sourceId, sha256: 'a'.repeat(64) }, observedHead: null, byteLength: 8 }], pack: { schemaVersion: 'rangoon.context-pack.v1', packId: `pack:${'c'.repeat(64)}`, bodyJson, bodySha256: 'd'.repeat(64), bodyBytes: new TextEncoder().encode(bodyJson).length, selectedBytes: 8, uniqueTextBytes: 8, omittedBytes: 0, tokenAccounting: 'unknown', authority: 'none' }, processingLocation: 'unknown', retention: 'unknown', authority: 'none' });
const asyncResult = (outcome, extra = {}) => common(outcome, { runId, ...extra });
const completion = (overrides = {}) => {
  const view = prepared(); const responseSha256 = 'e'.repeat(64);
  return { schemaVersion: 'rangoon.local-completion.v1', requestId: view.requestId, profileSha256: profileRecord().profileSha256, packId: view.pack.packId, responseSha256, observedModel: profile.model, serverCreatedAt: '2026-10-06T00:00:00Z', usageSource: 'server_reported', usage: { totalDuration: null, loadDuration: null, promptEvalCount: null, promptEvalCachedCount: null, promptEvalDuration: null, evalCount: null, evalDuration: null }, proposals: { schemaVersion: 'rangoon.validated-analysis.v1', packId: view.pack.packId, responseSha256, task: 'classify_v1', proposals: [], uncertainties: [], contentKind: 'model_authored', authority: 'none' }, processingLocation: 'unknown', authority: 'none', ...overrides };
};

test('load enters explicitly through profile, saved snapshots, and capability lists', async () => {
  const calls = [];
  const controller = createModelAssistanceController({ invoke: async command => {
    calls.push(command);
    if (command === 'get_local_model_profile') return common('unconfigured', { session: session() });
    if (command === 'list_snapshots') return { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] };
    return { outcome: 'listed', capabilities: [] };
  } });
  await controller.load();
  assert.deepEqual(calls.sort(), ['get_local_model_profile', 'list_capabilities', 'list_snapshots']);
  assert.equal(controller.getState().snapshots[0].sourceId, sourceId);
  controller.toggleSource(sourceId);
  assert.deepEqual(controller.getState().inputs, [{ kind: 'source', sourceId }]);
  controller.setTask('compare_v1');
  assert.deepEqual(controller.getState().inputs, []);
});

test('configure, prepare, send, and cancel use raw closed JSON bytes', async () => {
  const calls = [];
  const controller = createModelAssistanceController({ invoke: async (command, arg) => {
    calls.push([command, arg]);
    if (command === 'configure_local_model') return common('configured', { session: session(null, true) });
    if (command === 'prepare_local_model') return asyncResult('prepared', { prepared: prepared() });
    if (command === 'send_local_model') return asyncResult('cancelled');
    if (command === 'cancel_local_model') return common('cancel_requested', { session: session({ runId, kind: 'send' }, true) });
    if (command === 'get_local_model_profile') return common('configured', { session: session({ runId, kind: 'send' }, true) });
    return command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] };
  } });
  await controller.configure(profile);
  controller.getState().snapshots.length || await controller.load();
  controller.toggleSource(sourceId);
  await controller.prepare();
  assert.notEqual(controller.getState().prepared.bodyJson,controller.getState().prepared.pack.bodyJson);
  assert.notEqual(controller.getState().prepared.bodyBytes,controller.getState().prepared.pack.bodyBytes);
  await controller.send();
  for (const name of ['configure_local_model', 'prepare_local_model', 'send_local_model']) {
    const found = calls.find(([command]) => command === name);
    assert.ok(found, JSON.stringify({ name, calls: calls.map(([command]) => command), state: controller.getState() }));
    const arg = found[1];
    assert.ok(arg instanceof Uint8Array);
  }
  const send = JSON.parse(new TextDecoder().decode(calls.find(([command]) => command === 'send_local_model')[1]));
  assert.deepEqual(send, { schemaVersion: 'rangoon.local-send.v1', preparedId, requestId });
  assert.doesNotMatch(JSON.stringify(send), /approv/i);
});

test('late prepare reply cannot restore a request after invalidation', async () => {
  let resolvePrepare;
  const controller = createModelAssistanceController({ invoke: command => {
    if (command === 'prepare_local_model') return new Promise(resolve => { resolvePrepare = resolve; });
    if (command === 'configure_local_model') return Promise.resolve(common('configured', { session: session(null, true) }));
    if (command === 'get_local_model_profile') return Promise.resolve(common('configured', { session: session({ runId, kind: 'prepare' }, true) }));
    return Promise.resolve(command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] });
  } });
  await controller.load();
  await controller.configure(profile);
  controller.toggleSource(sourceId);
  const work = controller.prepare();
  controller.invalidate();
  resolvePrepare(asyncResult('prepared', { prepared: prepared() }));
  await work;
  assert.equal(controller.getState().prepared, null);
  assert.equal(controller.getState().pending, null);
});

test('polling discovers only an active native run and cancel sends its exact raw selector', async () => {
  let resolvePrepare;
  const calls = [];
  const controller = createModelAssistanceController({ invoke: (command, arg) => {
    calls.push([command, arg]);
    if (command === 'configure_local_model') return Promise.resolve(common('configured', { session: session(null, true) }));
    if (command === 'prepare_local_model') return new Promise(resolve => { resolvePrepare = resolve; });
    if (command === 'get_local_model_profile') return Promise.resolve(common('configured', { session: session({ runId, kind: 'prepare' }, true) }));
    if (command === 'cancel_local_model') return Promise.resolve(common('cancel_requested', { session: session({ runId, kind: 'prepare' }, true) }));
    return Promise.resolve(command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] });
  } });
  await controller.load();
  await controller.configure(profile);
  controller.toggleSource(sourceId);
  const work = controller.prepare();
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(controller.getState().activeRunId, runId);
  await controller.cancel();
  const cancel = calls.find(([command]) => command === 'cancel_local_model')[1];
  assert.ok(cancel instanceof Uint8Array);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(cancel)), { schemaVersion: 'rangoon.local-cancel.v1', runId });
  resolvePrepare(asyncResult('cancelled'));
  await work;
});

test('unknown envelope fails closed and browser never invokes a model endpoint', async () => {
  const browser = createModelAssistanceController();
  await browser.load();
  assert.equal(browser.getState().status, 'unavailable');
  assert.match(browser.getState().message, /unavailable/i);
  const controller = createModelAssistanceController({ invoke: async command => command === 'get_local_model_profile' ? { ...common('mystery'), outcome: 'mystery' } : { outcome: 'listed', snapshots: [], capabilities: [] } });
  await controller.load();
  assert.equal(controller.getState().status, 'failed');
  assert.equal(controller.getState().error.code, 'session_unavailable');
});

test('configured-null and malformed pack facts fail closed', async () => {
  const invalidConfigured = createModelAssistanceController({ invoke: async command => command === 'get_local_model_profile' ? common('configured', { session: session() }) : { outcome: 'listed', snapshots: [], capabilities: [] } });
  await invalidConfigured.load();
  assert.equal(invalidConfigured.getState().status, 'failed');
  const badPack = createModelAssistanceController({ invoke: async command => {
    if (command === 'get_local_model_profile') return common('configured', { session: session(null, true) });
    if (command === 'configure_local_model') return common('configured', { session: session(null, true) });
    if (command === 'prepare_local_model') return asyncResult('prepared', { prepared: { ...prepared(), pack: { packId: 'pack:test' } } });
    return command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] };
  } });
  await badPack.load(); await badPack.configure(profile); badPack.toggleSource(sourceId); await badPack.prepare();
  assert.equal(badPack.getState().prepared, null);
  assert.equal(badPack.getState().error.code, 'session_unavailable');
});

test('wrong completion request cannot become an advisory result', async () => {
  const controller = createModelAssistanceController({ invoke: async command => {
    if (command === 'get_local_model_profile' || command === 'configure_local_model') return common('configured', { session: session(null, true) });
    if (command === 'prepare_local_model') return asyncResult('prepared', { prepared: prepared() });
    if (command === 'send_local_model') return asyncResult('completed', { completion: completion({ requestId: 'f'.repeat(64) }), freshness: 'current' });
    return command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] };
  } });
  await controller.load(); await controller.configure(profile); controller.toggleSource(sourceId); await controller.prepare(); await controller.send();
  assert.equal(controller.getState().completion, null);
  assert.equal(controller.getState().error.code, 'proposal_invalid');
});

test('clear suppresses old reply but retains native cancel tracking while its prompt drains', async () => {
  let resolveSend;
  let clearedPolls = 0;
  const controller = createModelAssistanceController({ invoke: (command) => {
    if (command === 'get_local_model_profile') {
      if (clearedPolls) return Promise.resolve({ ...common('cleared', { session: clearedSession(clearedPolls++ === 1 ? { runId, kind: 'send' } : null) }), generation: '2' });
      return Promise.resolve(common('configured', { session: session({ runId, kind: 'send' }, true) }));
    }
    if (command === 'configure_local_model') return Promise.resolve(common('configured', { session: session(null, true) }));
    if (command === 'prepare_local_model') return Promise.resolve(asyncResult('prepared', { prepared: prepared() }));
    if (command === 'send_local_model') return new Promise(resolve => { resolveSend = resolve; });
    if (command === 'clear_local_model') { clearedPolls = 1; return Promise.resolve({ ...common('cleared', { session: clearedSession({ runId, kind: 'send' }) }), generation: '2' }); }
    if (command === 'cancel_local_model') return Promise.resolve({ ...common('cancel_requested', { session: clearedSession({ runId, kind: 'send' }) }), generation: '2' });
    return Promise.resolve(command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] });
  } });
  await controller.load(); await controller.configure(profile); controller.toggleSource(sourceId); await controller.prepare();
  const pending = controller.send(); await new Promise(resolve => setTimeout(resolve, 0));
  await controller.clear();
  assert.equal(controller.getState().prepared, null);
  assert.equal(controller.getState().draining, true);
  assert.equal(controller.getState().activeRunId, runId);
  await controller.cancel();
  resolveSend(asyncResult('cancelled'));
  await pending;
  assert.equal(controller.getState().completion, null);
});

test('idle-first inspection keeps polling so selection invalidation can still cancel its native run', async () => {
  let resolvePrepare;
  let inspections = 0;
  const calls = [];
  const controller = createModelAssistanceController({ invoke: (command, arg) => {
    calls.push([command, arg]);
    if (command === 'configure_local_model') return Promise.resolve(common('configured', { session: session(null, true) }));
    if (command === 'prepare_local_model') return new Promise(resolve => { resolvePrepare = resolve; });
    if (command === 'get_local_model_profile') {
      inspections += 1;
      return Promise.resolve(common('configured', { session: session(inspections <= 2 ? null : { runId, kind: 'prepare' }, true) }));
    }
    if (command === 'cancel_local_model') return Promise.resolve(common('cancel_requested', { session: session({ runId, kind: 'prepare' }, true) }));
    return Promise.resolve(command === 'list_snapshots' ? { outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] } : { outcome: 'listed', capabilities: [] });
  } });
  await controller.load(); await controller.configure(profile); controller.toggleSource(sourceId);
  const work = controller.prepare();
  await new Promise(resolve => setTimeout(resolve, 270));
  assert.equal(controller.getState().activeRunId, runId);
  controller.invalidate();
  await controller.cancel();
  assert.ok(calls.some(([command]) => command === 'cancel_local_model'));
  resolvePrepare(asyncResult('cancelled'));
  await work;
  assert.equal(controller.getState().prepared, null);
});

test('failed inventory can be explicitly retried without restarting the app', async () => {
  let fail = true;
  const controller = createModelAssistanceController({invoke: async command => {
    if (command === 'get_local_model_profile') return common('unconfigured',{session:session()});
    if (command === 'list_snapshots') return fail ? {outcome:'failed'} : {outcome:'listed',snapshots:[]};
    return {outcome:'listed',capabilities:[]};
  }});
  await controller.load(); assert.equal(controller.getState().status,'failed');
  fail = false; await controller.load(); assert.equal(controller.getState().status,'ready');
});

test('oversized model name is rejected before native configuration', async () => {
  let calls = 0;
  const controller = createModelAssistanceController({invoke: async () => {calls++;}});
  await controller.configure({...profile,model:`${'x'.repeat(128)}:v1`});
  assert.equal(calls,0); assert.equal(controller.getState().error.code,'invalid_profile');
});

test('unchanged active polls preserve live controls instead of publishing rerenders', async () => {
  let resolveCheck;
  let publications = 0;
  let polls = 0;
  const controller = createModelAssistanceController({
    onChange: () => { publications++; },
    invoke: async command => {
      if (command === 'configure_local_model') return common('configured', { session: session(null, true) });
      if (command === 'check_local_model') return new Promise(resolve => { resolveCheck = resolve; });
      if (command === 'get_local_model_profile') {
        polls++;
        return common('configured', { session: session({ runId, kind: 'check' }, true) });
      }
    },
  });
  await controller.configure(profile);
  const work = controller.check();
  await new Promise(resolve => setTimeout(resolve, 0));
  const stablePublications = publications;
  await new Promise(resolve => setTimeout(resolve, 550));
  assert.ok(polls >= 3);
  assert.equal(publications, stablePublications);
  assert.equal(controller.getState().activeRunId, runId);
  resolveCheck(asyncResult('cancelled'));
  await work;
  assert.ok(publications > stablePublications);
});
