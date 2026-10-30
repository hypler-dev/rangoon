import test from 'node:test';
import assert from 'node:assert/strict';
import { createSkillsController, filterSavedSkills, validCapabilityContent, validCapabilityTitle, validLibraryQuery } from '../preview/skills-model.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const capabilityId = `capability:${'3'.repeat(64)}`;
const revisionId = `revision:${'4'.repeat(64)}`;
const compositionId = `composition:${'5'.repeat(64)}`;
const applicationId = `composition-application:${'6'.repeat(64)}`;
const capabilityId2 = `capability:${'7'.repeat(64)}`;
const revisionId2 = `revision:${'8'.repeat(64)}`;
const content = 'Keep text inert.\n';
const ordinary = { kind: 'ordinary' };
const sourceOrigin = { kind: 'source', sourceId, fragmentId, sourceName: 'rules.md', span: { startByte: 0, endByte: new TextEncoder().encode(content).length, startLine: 1, endLine: 1 }, originalText: content };
const sourceOriginSummary = { kind: 'source', sourceId, fragmentId };
const revision = (overrides = {}) => ({ id: revisionId, parentRevisionId: null, title: 'Rules', content, sha256: 'a'.repeat(64), createdAtMs: 1, review: null, provenance: ordinary, ...overrides });
const summary = value => ({ id: value.id, parentRevisionId: value.parentRevisionId, title: value.title, sha256: value.sha256, createdAtMs: value.createdAtMs, review: value.review, provenance: value.provenance });
const detail = (overrides = {}) => ({ schemaVersion: 'rangoon.capability.v1', id: capabilityId, origin: sourceOrigin, latestRevisionId: revisionId, revision: revision(), history: [summary(revision())], authority: 'none', ...overrides });
const listed = () => ({ outcome: 'listed', capabilities: [{ id: capabilityId, origin: sourceOriginSummary, latestRevisionId: revisionId, title: 'Rules', reviewed: false, revisionCount: 1 }] });

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
  const saved = detail({ latestRevisionId: nextRevisionId, revision: next, history: [detail().history[0], summary(next)] });
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
  const nextDetail = detail({ latestRevisionId: nextRevisionId, revision: next, history: [detail().history[0], summary(next)] });
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

test('filters loaded summaries with literal Unicode query and combined review and origin facets', () => {
  const composedId = `capability:${'a'.repeat(64)}`;
  const composed = { id: composedId, origin: { kind: 'composition', compositionId, operation: 'merge', outputIndex: 0 }, latestRevisionId: revisionId2, title: 'Caf\u00e9 guide', reviewed: true, revisionCount: 1 };
  const items = [...listed().capabilities, composed];
  assert.deepEqual(filterSavedSkills(items, { query: 'CAFE\u0301', reviewFilter: 'reviewed', originFilter: 'composition' }).map(item => item.id), [composedId]);
  assert.deepEqual(filterSavedSkills(items, { query: 'source:', reviewFilter: 'unreviewed', originFilter: 'source' }).map(item => item.id), [capabilityId]);
  assert.deepEqual(filterSavedSkills(items, { query: '.*', reviewFilter: 'all', originFilter: 'all' }), []);
  assert.equal(validLibraryQuery('😀'.repeat(128)), true);
  assert.equal(validLibraryQuery('😀'.repeat(129)), false);
  assert.equal(validLibraryQuery('safe\u0000query'), false);
  assert.equal(filterSavedSkills(items, { query: 'ok', reviewFilter: 'invented', originFilter: 'all' }), null);
});

test('library controls stay local and retain selection, draft, history, conflict and pending state', async () => {
  const calls = [];
  let resolveOpen;
  const controller = createSkillsController({ invoke: command => {
    calls.push(command);
    if (command === 'list_capabilities') return Promise.resolve(listed());
    if (command === 'open_capability') return new Promise(resolve => { resolveOpen = resolve; });
    return Promise.resolve({ outcome: 'failed', error: { code: 'capability_conflict', message: 'Conflict' } });
  } });
  await controller.list();
  const opening = controller.open(capabilityId);
  assert.equal(controller.getState().pending, 'open');
  assert.equal(controller.setLibraryFilters({ query: 'rules', reviewFilter: 'unreviewed' }), true);
  assert.equal(controller.getState().pending, 'open');
  assert.equal(controller.getState().selected, null);
  assert.deepEqual(calls, ['list_capabilities', 'open_capability']);
  resolveOpen({ outcome: 'opened', capability: detail(), alreadyApplied: false });
  await opening;
  controller.updateDraft({ content: 'Unsaved work\n' });
  const before = controller.getState();
  assert.equal(controller.setLibraryFilters({ query: 'miss', originFilter: 'composition' }), true);
  const after = controller.getState();
  assert.equal(after.selected.id, before.selected.id);
  assert.equal(after.viewedRevisionId, before.viewedRevisionId);
  assert.equal(after.draft.content, 'Unsaved work\n');
  assert.equal(after.draft.dirty, true);
  await controller.save();
  assert.equal(controller.getState().needsReload, true);
  assert.equal(controller.setLibraryFilters({ query: 'rules', reviewFilter: 'unreviewed', originFilter: 'source' }), true);
  assert.equal(controller.getState().needsReload, true);
  assert.equal(controller.getState().draft.content, 'Unsaved work\n');
  assert.equal(controller.setLibraryFilters({ query: 'x'.repeat(129) }), false);
  assert.equal(controller.getState().query, 'rules');
  assert.equal(controller.setLibraryFilters(null), false);
  assert.equal(controller.setLibraryFilters(['reviewed']), false);
  assert.equal(controller.getState().query, 'rules');
  controller.clearLibraryFilters();
  assert.deepEqual([controller.getState().query, controller.getState().reviewFilter, controller.getState().originFilter], ['', 'all', 'all']);
  assert.deepEqual(calls, ['list_capabilities', 'open_capability', 'revise_capability']);
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
  assert.equal(controller.hasUnsavedDrafts(), true);
  await controller.open(capabilityId, revisionId);
  const historical = controller.getState();
  assert.equal(historical.draft.content, 'Unsaved current draft\n');
  assert.equal(controller.setLibraryFilters({ query: 'missing', reviewFilter: 'reviewed', originFilter: 'source' }), true);
  assert.equal(controller.getState().viewedRevisionId, revisionId);
  assert.equal(controller.getState().draft.content, 'Unsaved current draft\n');
  assert.equal(controller.getState().draft.dirty, true);
  const html = renderSkillsView(historical);
  assert.match(html, /<textarea[^>]*readonly[^>]*>Keep text inert\.\n<\/textarea>/);
  assert.doesNotMatch(html, /<textarea[^>]*>Unsaved current draft/);
  await controller.open(capabilityId);
  assert.equal(controller.getState().draft.content, 'Unsaved current draft\n');
  assert.equal(controller.getState().draft.dirty, true);
  assert.equal(controller.hasUnsavedDrafts(), true);
});

test('missing span fields, invalid dates, forged review and broken history are rejected', async () => {
  const { validateCapabilityDetail } = await import('../preview/skills-model.mjs');
  const cases = [
    detail({ origin: { ...sourceOrigin, span: { startByte: 0, endByte: content.length } } }),
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

test('v1 composition origins allow only operation-specific unique pinned inputs', async () => {
  const { validateCapabilityDetail, validateCapabilitySummary } = await import('../preview/skills-model.mjs');
  const sourceInput = { kind: 'source', sourceId, sha256: 'b'.repeat(64) };
  const revisionInput = { kind: 'revision', capabilityId, revisionId, sha256: 'c'.repeat(64) };
  const revisionInput2 = { kind: 'revision', capabilityId: capabilityId2, revisionId: revisionId2, sha256: 'd'.repeat(64) };
  const decompose = { kind: 'composition', compositionId, operation: 'decompose', outputIndex: 3, inputs: [sourceInput] };
  const split = { kind: 'composition', compositionId, operation: 'split', outputIndex: 3, inputs: [revisionInput] };
  const merge = { kind: 'composition', compositionId, operation: 'merge', outputIndex: 0, inputs: [revisionInput, revisionInput2] };
  const composed = { kind: 'composition', applicationId, compositionId, outputIndex: 0 };
  const composedDetail = origin => {
    const provenance = { ...composed, outputIndex: origin.outputIndex };
    const selected = revision({ provenance });
    return detail({ origin, revision: selected, history: [summary(selected)] });
  };
  for (const origin of [decompose, split, merge]) {
    assert.ok(validateCapabilityDetail(composedDetail(origin)));
  }
  assert.equal(validateCapabilityDetail(composedDetail({ ...decompose, inputs: [revisionInput] })), null);
  assert.equal(validateCapabilityDetail(composedDetail({ ...split, inputs: [sourceInput] })), null);
  assert.equal(validateCapabilityDetail(composedDetail({ ...merge, inputs: [sourceInput, revisionInput] })), null);
  assert.equal(validateCapabilityDetail(composedDetail({ ...merge, outputIndex: 1 })), null);
  assert.equal(validateCapabilityDetail(composedDetail({ ...merge, inputs: [revisionInput, { ...revisionInput, sha256: 'e'.repeat(64) }] })), null);
  assert.equal(validateCapabilityDetail(composedDetail({ ...merge, input: [] })), null);
  assert.equal(validateCapabilityDetail(detail({ revision: revision({ provenance: { kind: 'ordinary', extra: true } }) })), null);
  assert.equal(validateCapabilitySummary({ ...listed().capabilities[0], origin: { kind: 'composition', compositionId, operation: 'merge', outputIndex: 1 } }), false);
});

test('v1 birth origin agrees with root provenance while ordinary successors remain valid', async () => {
  const { validateCapabilityDetail } = await import('../preview/skills-model.mjs');
  const origin = { kind: 'composition', compositionId, operation: 'split', outputIndex: 1, inputs: [{ kind: 'revision', capabilityId, revisionId, sha256: 'b'.repeat(64) }] };
  const rootProvenance = { kind: 'composition', applicationId, compositionId, outputIndex: 1 };
  const root = revision({ provenance: rootProvenance });
  const successor = revision({ id: revisionId2, parentRevisionId: revisionId, content: 'Ordinary successor\n', sha256: 'e'.repeat(64), createdAtMs: 2, provenance: ordinary });
  assert.ok(validateCapabilityDetail(detail({ origin, latestRevisionId: revisionId2, revision: successor, history: [summary(root), summary(successor)] })));
  assert.equal(validateCapabilityDetail(detail({ revision: revision({ provenance: rootProvenance }), history: [summary(revision({ provenance: rootProvenance }))] })), null);
  assert.equal(validateCapabilityDetail(detail({ origin, revision: revision(), history: [summary(revision())] })), null);
  const wrongRoot = revision({ provenance: { ...rootProvenance, outputIndex: 0 } });
  assert.equal(validateCapabilityDetail(detail({ origin, revision: wrongRoot, history: [summary(wrongRoot)] })), null);
});

test('mismatched selected history provenance fails closed and composed views never invent source text', async () => {
  const { validateCapabilityDetail } = await import('../preview/skills-model.mjs');
  const { renderSkillsView } = await import('../preview/skills-view.mjs');
  const compositionOrigin = { kind: 'composition', compositionId, operation: 'split', outputIndex: 1, inputs: [{ kind: 'revision', capabilityId, revisionId, sha256: 'b'.repeat(64) }] };
  const composed = { kind: 'composition', applicationId, compositionId, outputIndex: 1 };
  const composedDetail = detail({ origin: compositionOrigin, revision: revision({ provenance: composed }), history: [summary(revision({ provenance: composed }))] });
  assert.ok(validateCapabilityDetail(composedDetail));
  assert.equal(validateCapabilityDetail({ ...composedDetail, history: [{ ...composedDetail.history[0], provenance: ordinary }] }), null);
  const html = renderSkillsView({ ...createSkillsController().getState(), selected: composedDetail, viewedRevisionId: revisionId, draft: { capabilityId, baseRevisionId: revisionId, title: 'Rules', content, dirty: false } });
  assert.match(html, /Birth origin/);
  assert.match(html, /Composition birth/);
  assert.match(html, /Selected revision provenance/);
  assert.match(html, /Composed revision/);
  assert.doesNotMatch(html, /Original saved section/);
});

test('confirmed deletion clears only that skill draft and prevents a late open from resurrecting it', async () => {
  let resolveOpen;
  let delayed = false;
  const controller = createSkillsController({ invoke: async command => {
    if (command === 'list_capabilities') return listed();
    if (delayed) return new Promise(resolve => { resolveOpen = resolve; });
    return { outcome: 'opened', capability: detail(), alreadyApplied: false };
  } });
  await controller.open(capabilityId);
  controller.updateDraft({ content: 'Unsaved work\n' });
  assert.equal(controller.hasUnsavedDraft(capabilityId), true);
  assert.equal(controller.hasUnsavedDrafts(), true);
  controller.forgetDeleted(`capability:${'9'.repeat(64)}`);
  assert.equal(controller.getState().draft.content, 'Unsaved work\n');
  delayed = true;
  const pending = controller.open(capabilityId);
  controller.forgetDeleted(capabilityId);
  resolveOpen({ outcome: 'opened', capability: detail(), alreadyApplied: false });
  await pending;
  assert.equal(controller.getState().selected, null);
  assert.equal(controller.getState().draft, null);
  assert.equal(controller.hasUnsavedDraft(capabilityId), false);
  assert.equal(controller.hasUnsavedDrafts(), false);
  assert.equal(controller.getState().pending, null);
});

const discardSnapshot = controller => Object.fromEntries(['capabilityId', 'baseRevisionId', 'title', 'content'].map(key => [key, controller.getState().draft?.[key]]));

test('discard is exact, local and resets only the selected unsaved draft', async () => {
  const calls = [];
  const other = detail({ id: capabilityId2, latestRevisionId: revisionId2, revision: revision({ id: revisionId2 }), history: [summary(revision({ id: revisionId2 }))] });
  const controller = createSkillsController({ invoke: async (command, args) => {
    calls.push(command); return command === 'list_capabilities' ? listed() : { outcome: 'opened', capability: args.capabilityId === capabilityId2 ? other : detail(), alreadyApplied: false };
  } });
  await controller.open(capabilityId2); controller.updateDraft({ content: 'Other draft' });
  await controller.open(capabilityId); controller.updateDraft({ title: 'My draft', content: 'Local changes' });
  const expected = discardSnapshot(controller); const before = calls.length;
  for (const key of ['capabilityId', 'baseRevisionId', 'title', 'content']) assert.equal(controller.discardDraft({ ...expected, [key]: 'stale' }), false);
  assert.equal(controller.getState().draft.content, 'Local changes');
  assert.equal(controller.discardDraft(expected), true);
  assert.equal(calls.length, before);
  assert.equal(controller.getState().draft.content, content);
  assert.equal(controller.getState().draft.dirty, false);
  assert.equal(controller.discardDraft(expected), false);
  await controller.open(capabilityId2); assert.equal(controller.getState().draft.content, 'Other draft');
});

test('discard preserves conflict reload gate and cannot revive a locked draft', async () => {
  const calls = [];
  const controller = createSkillsController({ invoke: async command => {
    calls.push(command); return command === 'open_capability' ? { outcome: 'opened', capability: detail(), alreadyApplied: false } : { outcome: 'failed', error: { code: 'capability_conflict', message: 'Conflict' } };
  } });
  await controller.open(capabilityId); controller.updateDraft({ content: 'Changed' }); await controller.save();
  assert.equal(controller.getState().needsReload, true);
  assert.equal(controller.discardDraft(discardSnapshot(controller)), true);
  assert.equal(controller.getState().needsReload, true);
  const before = calls.length; await controller.review(); await controller.save(); assert.equal(calls.length, before);
  controller.updateDraft({ content: 'Another local draft' }); const expected = discardSnapshot(controller);
  controller.purgeForWorkspaceLock(); assert.equal(controller.discardDraft(expected), false); assert.equal(controller.getState().selected, null);
});

test('pending and historical views refuse discard without changing the retained current draft', async () => {
  const next = revision({ id: revisionId2, parentRevisionId: revisionId, content: 'Current saved', createdAtMs: 2 });
  let release;
  const controller = createSkillsController({ invoke: async (command, args) => {
    if (command === 'list_capabilities') return listed();
    if (args.revisionId) return new Promise(resolve => { release = () => resolve({ outcome: 'opened', capability: detail({ latestRevisionId: revisionId2, history: [summary(revision()), summary(next)] }), alreadyApplied: false }); });
    return { outcome: 'opened', capability: detail({ latestRevisionId: revisionId2, revision: next, history: [summary(revision()), summary(next)] }), alreadyApplied: false };
  } });
  await controller.open(capabilityId); controller.updateDraft({ content: 'Retained current' }); const expected = discardSnapshot(controller);
  const opening = controller.open(capabilityId, revisionId); assert.equal(controller.discardDraft(expected), false);
  release(); await opening; assert.equal(controller.discardDraft(expected), false);
  await controller.open(capabilityId); assert.equal(controller.getState().draft.content, 'Retained current');
});
