import test from 'node:test';
import assert from 'node:assert/strict';
import { skillCompileSelection, skillCompileHint, openSkillInCompilation, openSkillInComposition, skillCompositionIssue } from '../preview/skill-compile-handoff.mjs';

const cap = `capability:${'a'.repeat(64)}`;
const revision = `revision:${'b'.repeat(64)}`;
const next = `revision:${'c'.repeat(64)}`;
function saved() { return { bridgeAvailable: true, pending: null, needsReload: false, viewedRevisionId: revision, selected: { id: cap, latestRevisionId: revision, revision: { id: revision } }, draft: { capabilityId: cap, baseRevisionId: revision, title: 'Saved', content: 'Original', dirty: false } }; }

test('handoff pins only saved identities and preserves source state without compiling', async () => {
  const state = saved(); const before = structuredClone(state); const calls = [];
  const compilation = { openPinnedRevision: async (...args) => { calls.push(args); return args[2].isCurrent(); } };
  assert.equal(await openSkillInCompilation({ skills: { getState: () => state }, compilation, isCurrent: () => true }), true);
  assert.equal(calls.length, 1); assert.deepEqual(calls[0].slice(0, 2), [cap, revision]);
  assert.deepEqual(state, before);
});

test('historical selection stays exact and preserves unrelated drafts', async () => {
  const state = saved(); state.selected.latestRevisionId = next; state.draft.baseRevisionId = next;
  let pin;
  const compilation = { openPinnedRevision: async (id, rev) => { pin = [id, rev]; return true; } };
  assert.equal(await openSkillInCompilation({ skills: { getState: () => state }, compilation, isCurrent: () => true }), true);
  assert.deepEqual(pin, [cap, revision]); assert.equal(state.draft.baseRevisionId, next);
  state.draft = { ...state.draft, capabilityId: `capability:${'d'.repeat(64)}`, dirty: true };
  const before = structuredClone(state.draft);
  assert.equal(await openSkillInCompilation({ skills: { getState: () => state }, compilation, isCurrent: () => true }), true);
  assert.deepEqual(state.draft, before);
});

test('historical view refuses a dirty current draft for that same skill', async () => {
  const state = saved(); state.selected.latestRevisionId = next; state.draft.baseRevisionId = next; state.draft.dirty = true;
  const before = structuredClone(state);
  assert.equal(await openSkillInCompilation({ skills: { getState: () => state }, compilation: { openPinnedRevision: () => assert.fail('no request') }, isCurrent: () => true }), false);
  assert.match(skillCompileHint(state), /Return to current.*save or discard/);
  assert.deepEqual(state, before);
});

test('unavailable, busy, conflict, dirty and unbound states never open destination', async () => {
  const values = [
    s => { s.bridgeAvailable = false; }, s => { s.pending = 'save'; }, s => { s.needsReload = true; },
    s => { s.draft.dirty = true; }, s => { s.draft.baseRevisionId = next; }, s => { s.selected = null; },
    s => { s.viewedRevisionId = next; }, s => { s.selected.id = 'untrusted'; }, s => { s.selected.revision.id = null; },
  ];
  for (const mutate of values) {
    const state = saved(); mutate(state); assert.equal(skillCompileSelection(state), null);
    assert.equal(await openSkillInCompilation({ skills: { getState: () => state }, compilation: { openPinnedRevision: () => assert.fail('no request') }, isCurrent: () => true }), false);
  }
  assert.match(skillCompileHint({ ...saved(), needsReload: true }), /Reload/);
  assert.match(skillCompileHint({ ...saved(), draft: { ...saved().draft, dirty: true } }), /Save or discard/);
});

test('route, draft and selection changes invalidate an outstanding read', async () => {
  for (const kind of ['route', 'draft', 'revision']) {
    const state = saved(); let current = true, release, retainedGuard;
    const waiting = openSkillInCompilation({ skills: { getState: () => state }, isCurrent: () => current,
      compilation: { openPinnedRevision: async (_id, _revision, { isCurrent }) => { retainedGuard = isCurrent; await new Promise(resolve => { release = resolve; }); return isCurrent(); } } });
    if (kind === 'route') current = false;
    if (kind === 'draft') state.draft.dirty = true;
    if (kind === 'revision') state.viewedRevisionId = next;
    assert.equal(retainedGuard(), false); release(); assert.equal(await waiting, false);
  }
});

const composeState = () => ({ operation: 'merge', bridgeAvailable: true, pending: null, draft: null, inputs: [] });

test('Compose handoff shares saved current/history eligibility and never changes Skills drafts', async () => {
  for (const historical of [false, true]) {
    const state = saved();
    if (historical) { state.selected.latestRevisionId = next; state.draft = { capabilityId: `capability:${'d'.repeat(64)}`, dirty: true }; }
    const before = structuredClone(state); const calls = [];
    const composition = { getState: composeState, addRevision: async (...args) => { calls.push(args); return args[2].isCurrent(); } };
    assert.equal(await openSkillInComposition({ skills: { getState: () => state }, composition, isCurrent: () => true }), true);
    assert.deepEqual(calls[0].slice(0, 2), [cap, revision]); assert.deepEqual(state, before);
  }
});

test('Compose draft, capacity, duplicate, unavailable and busy blocks do not contact destination', async () => {
  const cases = [
    [{ ...composeState(), draft: { outputs: ['Keep my work'] } }, /draft is open/],
    [{ ...composeState(), inputs: Array.from({ length: 16 }, () => ({ reference: {} })) }, /16 revisions/],
    [{ ...composeState(), inputs: [{ reference: { capabilityId: cap, revisionId: revision } }] }, /already selected/],
    [{ ...composeState(), bridgeAvailable: false }, /desktop/],
    [{ ...composeState(), pending: 'load' }, /Wait/],
    [{ ...composeState(), operation: 'split' }, /desktop/],
  ];
  for (const [destination, hint] of cases) {
    const before = structuredClone(destination);
    assert.match(skillCompositionIssue(saved(), destination), hint);
    assert.equal(await openSkillInComposition({ skills: { getState: saved }, composition: { getState: () => destination, addRevision: () => assert.fail('no read') }, isCurrent: () => true }), false);
    assert.deepEqual(destination, before);
  }
  for (const historical of [false, true]) {
    const state = saved(); state.draft.dirty = true;
    if (historical) state.selected.latestRevisionId = next;
    assert.match(skillCompositionIssue(state, composeState()), /save or discard/i);
    assert.equal(await openSkillInComposition({ skills: { getState: () => state }, composition: { getState: composeState, addRevision: () => assert.fail('no read') }, isCurrent: () => true }), false);
  }
});

test('Compose handoff invalidates route, skill, revision, conflict, busy and local edits during a read', async () => {
  for (const kind of ['route', 'skill', 'revision', 'conflict', 'busy', 'draft']) {
    const state = saved(); let active = true, release;
    const composition = { getState: composeState, addRevision: async (_id, _rev, { isCurrent }) => { await new Promise(resolve => { release = resolve; }); return isCurrent(); } };
    const pending = openSkillInComposition({ skills: { getState: () => state }, composition, isCurrent: () => active });
    if (kind === 'route') active = false;
    if (kind === 'skill') state.selected.id = `capability:${'f'.repeat(64)}`;
    if (kind === 'revision') state.viewedRevisionId = next;
    if (kind === 'conflict') state.needsReload = true;
    if (kind === 'busy') state.pending = 'list';
    if (kind === 'draft') state.draft.dirty = true;
    release(); assert.equal(await pending, false);
  }
});
