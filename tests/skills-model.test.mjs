import test from 'node:test';
import assert from 'node:assert/strict';
import { createSkillsController, validCapabilityContent, validCapabilityTitle } from '../preview/skills-model.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const capabilityId = `capability:${'3'.repeat(64)}`;
const revisionId = `revision:${'4'.repeat(64)}`;
const content = 'Keep text inert.\n';
const revision = (overrides = {}) => ({ id: revisionId, parentRevisionId: null, title: 'Rules', content, sha256: 'a'.repeat(64), createdAtMs: 1, review: null, ...overrides });
const detail = (overrides = {}) => ({ schemaVersion: 'rangoon.capability.v0', id: capabilityId, sourceId, fragmentId, sourceName: 'rules.md', span: { startByte: 0, endByte: new TextEncoder().encode(content).length, startLine: 1, endLine: 1 }, originalText: content, latestRevisionId: revisionId, revision: revision(), history: [{ id: revisionId, parentRevisionId: null, title: 'Rules', sha256: 'a'.repeat(64), createdAtMs: 1, review: null }], authority: 'none', ...overrides });
const listed = () => ({ outcome: 'listed', capabilities: [{ id: capabilityId, sourceId, fragmentId, latestRevisionId: revisionId, title: 'Rules', reviewed: false, revisionCount: 1 }] });

test('browser mutations are unavailable no-ops and strict validation keeps authority closed', async () => {
  const controller = createSkillsController();
  controller.setSourceContext({ report: { source: { id: sourceId }, fragments: [{ id: fragmentId }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' });
  await controller.create('Rules');
  assert.equal(controller.getState().selected, null);
  assert.equal(controller.getState().listStatus, 'unavailable');
  assert.equal(validCapabilityTitle('A title'), true);
  assert.equal(validCapabilityTitle(' two'), false);
  assert.equal(validCapabilityTitle('A\u2028title'), false);
  assert.equal(validCapabilityContent('  x  '), true);
  assert.equal(validCapabilityContent(' \n '), false);
});

test('uses exact camel-case IPC arguments and preserves draft after a failed save', async () => {
  const calls = [];
  const controller = createSkillsController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'list_capabilities') return listed();
    if (command === 'create_capability') return { outcome: 'opened', capability: detail(), alreadyApplied: false };
    return { outcome: 'failed', error: { code: 'workspace_busy', message: 'Busy' } };
  } });
  await controller.list();
  controller.setSourceContext({ report: { source: { id: sourceId }, selectedFragmentId: fragmentId, fragments: [{ id: fragmentId, heading: { title: 'Rules' }, span: { startByte: 0, endByte: 1, startLine: 1, endLine: 1 } }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' });
  await controller.create('Rules');
  controller.updateDraft({ content: 'Changed content\n' });
  await controller.save();
  assert.deepEqual(calls.find(([name]) => name === 'create_capability'), ['create_capability', { sourceId, fragmentId, title: 'Rules' }]);
  assert.deepEqual(calls.find(([name]) => name === 'revise_capability'), ['revise_capability', { capabilityId, expectedRevisionId: revisionId, title: 'Rules', content: 'Changed content\n' }]);
  assert.equal(controller.getState().draft.content, 'Changed content\n');
});

test('review gates dirty drafts and accepts only local_operator history', async () => {
  const calls = [];
  const controller = createSkillsController({ invoke: async (command, args) => { calls.push([command, args]); return command === 'list_capabilities' ? listed() : { outcome: 'opened', capability: detail(), alreadyApplied: false }; } });
  await controller.open(capabilityId);
  controller.updateDraft({ content: 'Changed\n' });
  await controller.review();
  assert.equal(calls.some(([name]) => name === 'review_capability'), false);
  controller.updateDraft({ content: 'Keep text inert.\n' });
  await controller.review();
  assert.deepEqual(calls.find(([name]) => name === 'review_capability'), ['review_capability', { capabilityId, expectedRevisionId: revisionId }]);
});

test('confirmed save replaces the old dirty base with the returned current revision', async () => {
  const nextRevisionId = `revision:${'6'.repeat(64)}`;
  const next = revision({ id: nextRevisionId, parentRevisionId: revisionId, content: 'Changed content\n', sha256: 'c'.repeat(64), createdAtMs: 2 });
  const saved = detail({ latestRevisionId: nextRevisionId, revision: next, history: [detail().history[0], { id: nextRevisionId, parentRevisionId: revisionId, title: 'Rules', sha256: 'c'.repeat(64), createdAtMs: 2, review: null }] });
  const controller = createSkillsController({ invoke: async command => {
    if (command === 'list_capabilities') return listed();
    if (command === 'open_capability') return { outcome: 'opened', capability: detail(), alreadyApplied: false };
    return { outcome: 'opened', capability: saved, alreadyApplied: false };
  } });
  await controller.open(capabilityId);
  controller.updateDraft({ content: 'Changed content\n' });
  await controller.save();
  const state = controller.getState();
  assert.equal(state.draft.baseRevisionId, nextRevisionId);
  assert.equal(state.draft.content, 'Changed content\n');
  assert.equal(state.draft.dirty, false);
});

test('malformed successful details fail closed without replacing an open capability', async () => {
  const controller = createSkillsController({ invoke: async command => command === 'list_capabilities' ? listed() : { outcome: 'opened', capability: detail({ authority: 'granted' }), alreadyApplied: false } });
  await controller.open(capabilityId);
  assert.equal(controller.getState().selected, null);
  assert.equal(controller.getState().error.message, 'The local capability action could not finish.');
});

test('conflict blocks retry until explicit reload preserves the draft on a new head', async () => {
  let save;
  let reload;
  let awaitingReload = false;
  const controller = createSkillsController({ invoke: (command) => {
    if (command === 'open_capability') return awaitingReload ? new Promise(resolve => { reload = resolve; }) : Promise.resolve({ outcome: 'opened', capability: detail(), alreadyApplied: false });
    if (command === 'revise_capability') return new Promise(resolve => { save = resolve; });
    return Promise.resolve(listed());
  } });
  await controller.open(capabilityId);
  assert.equal(controller.getState().selected.authority, 'none');
  controller.updateDraft({ content: 'Changed\n' });
  const pending = controller.save();
  save({ outcome: 'failed', error: { code: 'capability_conflict', message: 'Conflict' } });
  await pending;
  assert.equal(controller.getState().needsReload, true);
  awaitingReload = true;
  const reopen = controller.reloadCurrent();
  const nextRevisionId = `revision:${'5'.repeat(64)}`;
  const next = revision({ id: nextRevisionId, parentRevisionId: revisionId, content: 'Server content\n', sha256: 'b'.repeat(64), createdAtMs: 2 });
  const nextDetail = detail({ latestRevisionId: nextRevisionId, revision: next, history: [detail().history[0], { id: nextRevisionId, parentRevisionId: revisionId, title: 'Rules', sha256: 'b'.repeat(64), createdAtMs: 2, review: null }] });
  reload({ outcome: 'opened', capability: nextDetail, alreadyApplied: false });
  await reopen;
  assert.equal(controller.getState().needsReload, false);
  assert.equal(controller.getState().draft.content, 'Changed\n');
  assert.equal(controller.getState().draft.baseRevisionId, nextRevisionId);
  assert.equal(controller.getState().draft.dirty, true);
});

test('late list response cannot replace newer library results', async () => {
  const responders = [];
  const controller = createSkillsController({ invoke: () => new Promise(resolve => responders.push(resolve)) });
  const older = controller.list();
  const newer = controller.list();
  const changed = listed();
  changed.capabilities[0].title = 'Newer rules';
  responders[1](changed);
  await newer;
  responders[0](listed());
  await older;
  assert.equal(controller.getState().capabilities[0].title, 'Newer rules');
});

test('creation title survives repeated source updates and section round trips', () => {
  const controller = createSkillsController({ invoke: async () => listed() });
  const context = { report: { source: { id: sourceId }, selectedFragmentId: fragmentId, fragments: [{ id: fragmentId, heading: { title: 'Rules' } }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' };
  controller.setSourceContext(context);
  controller.updateCreateTitle('My skill');
  controller.setSourceContext(context);
  assert.equal(controller.getState().createTitle, 'My skill');
  controller.setSourceContext({});
  controller.setSourceContext(context);
  assert.equal(controller.getState().createTitle, 'My skill');
});

test('a list response during open cannot strand the pending action', async () => {
  let resolveOpen;
  const controller = createSkillsController({ invoke: command => command === 'open_capability' ? new Promise(resolve => { resolveOpen = resolve; }) : Promise.resolve(listed()) });
  const opening = controller.open(capabilityId);
  await controller.list();
  resolveOpen({ outcome: 'opened', capability: detail(), alreadyApplied: false });
  await opening;
  assert.equal(controller.getState().pending, null);
  assert.equal(controller.getState().selected.id, capabilityId);
});

test('historical display uses its saved content while retaining the current dirty draft', async () => {
  const { renderSkillsView } = await import('../preview/skills-view.mjs');
  const nextId = `revision:${'7'.repeat(64)}`;
  const next = revision({ id: nextId, parentRevisionId: revisionId, content: 'Saved successor\n', sha256: 'b'.repeat(64) });
  const history = [...detail().history, { ...next }];
  delete history[1].content;
  const current = detail({ latestRevisionId: nextId, revision: next, history });
  const controller = createSkillsController({ invoke: async (command, args) => command === 'list_capabilities' ? listed() : { outcome: 'opened', capability: args.revisionId ? { ...current, revision: revision() } : current, alreadyApplied: false } });
  await controller.open(capabilityId);
  controller.updateDraft({ content: 'Unsaved current draft\n' });
  await controller.open(capabilityId, revisionId);
  const historical = controller.getState();
  assert.equal(historical.draft.content, 'Unsaved current draft\n');
  const html = renderSkillsView(historical);
  assert.match(html, /<textarea[^>]*readonly[^>]*>Keep text inert\.\n<\/textarea>/);
  assert.doesNotMatch(html, /<textarea[^>]*>Unsaved current draft/);
  await controller.open(capabilityId);
  assert.equal(controller.getState().draft.content, 'Unsaved current draft\n');
  assert.equal(controller.getState().draft.dirty, true);
});

test('missing span fields, invalid dates, forged review and broken history are rejected', async () => {
  const { validateCapabilityDetail } = await import('../preview/skills-model.mjs');
  const cases = [
    detail({ span: { startByte: 0, endByte: content.length } }),
    detail({ revision: revision({ createdAtMs: 8_640_000_000_000_001 }) }),
    detail({ revision: revision({ review: { reviewer: 'admin', reviewedAtMs: 1 } }) }),
    detail({ id: 'capability:wrong' }),
    detail({ history: [{ ...detail().history[0], parentRevisionId: revisionId }] }),
  ];
  for (const invalid of cases) assert.equal(validateCapabilityDetail(invalid), null);
  assert.ok(validateCapabilityDetail(detail()));
  assert.equal(validCapabilityTitle('A\u2029B'), false);
  assert.equal(validCapabilityTitle('👩‍💻 review'), true);
});

test('coverage counts source sections once and browser unknown is never presented as empty', async () => {
  const { renderSkillsView } = await import('../preview/skills-view.mjs');
  const controller = createSkillsController();
  assert.doesNotMatch(renderSkillsView(controller.getState()), /No local capabilities yet/);
  const state = { ...controller.getState(), listStatus: 'ready', capabilities: [...listed().capabilities, { ...listed().capabilities[0], id: `capability:${'8'.repeat(64)}` }], source: { sourceId, fragmentId, title: 'Rules', saved: false } };
  assert.match(renderSkillsView(state), /1 derived section/);
  assert.doesNotMatch(renderSkillsView(state), /2 derived sections/);
});
