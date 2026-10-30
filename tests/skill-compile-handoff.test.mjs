import test from 'node:test';
import assert from 'node:assert/strict';
import { skillCompileSelection, skillCompileHint, openSkillInCompilation } from '../preview/skill-compile-handoff.mjs';

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
