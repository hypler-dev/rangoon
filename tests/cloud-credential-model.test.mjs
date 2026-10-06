import assert from 'node:assert/strict';
import test from 'node:test';
import { createCloudCredentialController, validateCloudCustody } from '../preview/cloud-credential-model.mjs';

const revision = 'a'.repeat(64);
const result = (outcome, extra = {}) => ({
  schemaVersion: 'rangoon.cloud-custody.v1', provider: 'openai', authority: 'none', backend: 'macos_keychain', outcome,
  ...(outcome === 'stored' || outcome === 'saved' ? { revision } : {}),
  ...(outcome === 'failed' ? { code: 'unavailable' } : {}), ...extra,
});

test('constructor has unknown state and does not access native credential storage', () => {
  let calls = 0;
  const controller = createCloudCredentialController({ invoke: () => { calls += 1; } });
  assert.equal(calls, 0);
  assert.equal(controller.getState().status, 'unknown');
  assert.equal(controller.getState().revision, null);
});

test('main controller sends only exact no-argument custody commands', async () => {
  const calls = [];
  const controller = createCloudCredentialController({ invoke: async (...args) => { calls.push(args); return result(args[0] === 'inspect_cloud_credential' ? 'stored' : args[0] === 'remove_cloud_credential' ? 'removed' : 'saved'); } });
  await controller.inspect(); await controller.edit(); await controller.remove();
  assert.deepEqual(calls, [['inspect_cloud_credential'], ['edit_cloud_credential'], ['remove_cloud_credential']]);
  assert.equal(controller.getState().status, 'missing');
});

test('invalid DTO fails closed and cannot retain a prior stored revision', async () => {
  let inspect = true;
  const controller = createCloudCredentialController({ invoke: async () => inspect ? result('stored') : { ...result('saved'), provider: 'other' } });
  await controller.inspect();
  assert.equal(controller.getState().status, 'stored');
  inspect = false;
  await controller.edit();
  assert.equal(controller.getState().status, 'unknown');
  assert.equal(controller.getState().revision, null);
  assert.equal(controller.getState().error.code, 'invalid_request');
});

test('cancel keeps prior visible custody state, failed mutation invalidates it', async () => {
  const outcomes = [result('stored'), result('cancelled'), result('failed', { code: 'write_uncertain' })];
  const controller = createCloudCredentialController({ invoke: async () => outcomes.shift() });
  await controller.inspect();
  await controller.edit();
  assert.equal(controller.getState().status, 'stored');
  assert.equal(controller.getState().revision, revision);
  await controller.remove();
  assert.equal(controller.getState().status, 'unknown');
  assert.equal(controller.getState().revision, null);
  assert.equal(controller.getState().error.code, 'write_uncertain');
});

test('browser preview is fail closed and emits no provider traffic command', async () => {
  const controller = createCloudCredentialController();
  await controller.inspect();
  assert.equal(controller.getState().status, 'unknown');
  assert.match(controller.getState().message, /No provider check or send was made/);
});

test('closed DTO validator rejects extra fields and wrong result shapes', () => {
  assert.deepEqual(validateCloudCustody(result('stored')), result('stored'));
  assert.equal(validateCloudCustody({ ...result('stored'), code: 'unavailable' }), null);
  assert.equal(validateCloudCustody({ ...result('failed'), revision }), null);
  assert.equal(validateCloudCustody({ ...result('saved'), revision: 'A'.repeat(64) }), null);
});

test('uncertain result requires successful explicit inspection before another change', async () => {
  const calls = [];
  const outcomes = [result('failed', {code:'write_uncertain'}), result('missing'), result('saved')];
  const controller = createCloudCredentialController({invoke: async name => { calls.push(name); return outcomes.shift(); }});
  await controller.edit();
  assert.equal(controller.getState().needsInspection,true);
  assert.equal(await controller.edit(),false);
  assert.equal(await controller.remove(),false);
  assert.deepEqual(calls,['edit_cloud_credential']);
  await controller.inspect();
  assert.equal(controller.getState().needsInspection,false);
  await controller.edit();
  assert.deepEqual(calls,['edit_cloud_credential','inspect_cloud_credential','edit_cloud_credential']);
});
