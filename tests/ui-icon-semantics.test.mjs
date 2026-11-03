import test from 'node:test';
import assert from 'node:assert/strict';
import { icon } from '../preview/icons.mjs';
import { renderWorkspaceGuard, renderWorkspaceCloseShell, renderWorkspaceLockDialog } from '../preview/workspace-lock-view.mjs';
import { renderWorkspaceView } from '../preview/workspace-view.mjs';
import { renderCompilationPage } from '../preview/compilation-view.mjs';
import { renderSkillsView } from '../preview/skills-view.mjs';
import { EMPTY_SKILLS_STATE } from '../preview/skills-model.mjs';
import { renderAuthorityView } from '../preview/authority-view.mjs';
import { createAuthorityController } from '../preview/authority-catalog.mjs';

const button = (html, id) => html.match(new RegExp(`<button\\b[^>]*id="${id}"[^>]*>[\\s\\S]*?<\\/button>`))?.[0] ?? '';
const noIcons = html => html.replace(/<svg\b[^>]*>[\s\S]*?<\/svg>/g, '');

test('lock and encrypted symbols follow existing observed state rather than decorating unavailable paths', () => {
  const open = renderWorkspaceGuard({ status: 'open', canLock: true });
  assert.match(open, /foldline-icon--encrypted/);
  assert.match(open, /Encrypted workspace is unlocked/);
  assert.match(button(open, 'workspace-lock-open'), /foldline-icon--lock/);
  assert.match(noIcons(open), /Lock workspace/);
  for (const status of ['browser', 'legacy', 'checking', 'unavailable', 'closing', 'recovery_required']) {
    const html = renderWorkspaceGuard({ status, canLock: true });
    assert.doesNotMatch(html, /workspace-lock-open|foldline-icon--encrypted/);
  }
  assert.match(renderWorkspaceGuard({ status: 'legacy' }), /foldline-icon--plaintext/);
  assert.doesNotMatch(renderWorkspaceGuard({ status: 'open', canLock: false }), /foldline-icon--encrypted/);
  assert.match(renderWorkspaceGuard({ status: 'locked' }), /foldline-icon--lock/);
  const fresh = renderWorkspaceCloseShell({ status: 'protection', layout: 'absent', canProtect: true });
  assert.match(fresh, /foldline-icon--database/);
  assert.doesNotMatch(fresh, /foldline-icon--encrypted/);
  assert.match(renderWorkspaceCloseShell({ status: 'protection', layout: 'locked', canProtect: true }), /foldline-icon--unlock/);
  assert.match(button(renderWorkspaceLockDialog(), 'workspace-lock-confirm'), /foldline-icon--lock/);
});

test('encrypted backup and readable export retain distinct symbols, labels and refusal gates', () => {
  const state = { bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Ready.', encryptedRecoverySupported: true,
    workspace: { records: {}, databaseBytes: 4096, reusableBytes: 0, sources: [], capabilities: [], workflows: [] }, dialog: null };
  const managed = renderWorkspaceView(state, { storageMode: 'managed_encrypted' });
  assert.match(button(managed, 'workspace-encrypted-export'), /foldline-icon--encrypted/);
  assert.match(button(managed, 'workspace-encrypted-restore'), /foldline-icon--restore/);
  const legacy = renderWorkspaceView(state, { storageMode: 'legacy_plaintext' });
  assert.match(button(legacy, 'workspace-export'), /foldline-icon--plaintext/);
  assert.doesNotMatch(legacy, /foldline-icon--encrypted/);
  const compile = renderCompilationPage({ bridgeAvailable: false, capabilities: [], listStatus: 'unavailable', profile: 'claude_md_v1' });
  assert.match(compile, /foldline-icon--plaintext/);
  assert.match(button(compile, 'compile-export'), /disabled[^>]*>[\s\S]*foldline-icon--export/);
  assert.match(compile, /Text format only · target loading not verified/);
  assert.match(compile, /plaintext[\s\S]*does not install or run/);
});

test('Skills keeps action names, exact disabled state and decorative non-focusable symbols', () => {
  const id = `capability:${'3'.repeat(64)}`, revisionId = `revision:${'4'.repeat(64)}`;
  const revision = { id: revisionId, title: 'Instructions', content: 'Review changes.', createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } };
  const saved = { ...EMPTY_SKILLS_STATE, bridgeAvailable: true, listStatus: 'ready', capabilities: [],
    selected: { id, latestRevisionId: revisionId, revision, history: [revision], origin: { kind: 'composition', compositionId: 'fixture', operation: 'merge', inputs: [] } },
    viewedRevisionId: revisionId, draft: { capabilityId: id, baseRevisionId: revisionId, title: revision.title, content: revision.content, dirty: false } };
  const html = renderSkillsView(saved);
  for (const [id, key, label] of [['skills-save','save','Save revision'],['skills-review','check','Mark reviewed'],['skills-discard','undo','Discard local edits'],['skills-open-compile','compile','Open in Compile'],['skills-open-compose','merge','Add to Compose']]) {
    const action = button(html, id);
    assert.match(action, new RegExp(`foldline-icon--${key}`));
    assert.ok(noIcons(action).includes(label));
    assert.match(action, /aria-hidden="true" focusable="false"/);
  }
  const dirty = renderSkillsView({ ...saved, draft: { ...saved.draft, dirty: true, content: 'Edited.' } });
  assert.match(button(dirty, 'skills-open-compile'), /disabled/);
  assert.match(button(dirty, 'skills-open-compose'), /disabled/);
});

test('vetted software marks identify products without converting identity into support', () => {
  const state = createAuthorityController().getState();
  const html = renderAuthorityView(state);
  assert.match(html, /assets\/authority\/lnsat-mark.webp/);
  for (const path of ['agentgateway-mark.svg','cerbos-mark.png','permit-mark.svg','invariant-mark.png']) assert.ok(html.includes('assets/vendors/'+path));
  assert.match(html, /foldline-icon--gate" width="40"/);
  assert.match(html, /Unavailable · 0 \/ 5 available/);
  assert.doesNotMatch(html, /cdn\.|onload=/);
  for (const name of ['lock','unlock','encrypted','plaintext','compile','edit','database']) {
    assert.ok(icon(name, { size: 16 }));
    assert.equal(icon(name, { size: 15 }), '');
  }
});
