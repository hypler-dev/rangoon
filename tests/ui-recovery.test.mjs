import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createSkillsController } from '../preview/skills-model.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const capabilityId = `capability:${'3'.repeat(64)}`;
const revisionId = `revision:${'4'.repeat(64)}`;
const content = 'Keep text inert.\n';
const revision = { id: revisionId, parentRevisionId: null, title: 'Rules', content, sha256: 'a'.repeat(64), createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } };
const revisionSummary = ({ content: ignored, ...value }) => value;
const detail = { schemaVersion: 'rangoon.capability.v1', id: capabilityId, origin: { kind: 'source', sourceId, fragmentId, sourceName: 'rules.md', span: { startByte: 0, endByte: new TextEncoder().encode(content).length, startLine: 1, endLine: 1 }, originalText: content }, latestRevisionId: revisionId, revision, history: [revisionSummary(revision)], authority: 'none' };

test('retained hidden draft remains an exit-warning candidate until its own saved revision clears it', async () => {
  const nextId = `revision:${'5'.repeat(64)}`;
  const savedRevision = { ...revision, id: nextId, parentRevisionId: revisionId, content: 'Saved local edit.\n', sha256: 'b'.repeat(64), createdAtMs: 2 };
  const controller = createSkillsController({ invoke: async command => {
    if (command === 'open_capability') return { outcome: 'opened', capability: detail, alreadyApplied: false };
    if (command === 'revise_capability') return { outcome: 'opened', capability: { ...detail, latestRevisionId: nextId, revision: savedRevision, history: [detail.history[0], revisionSummary(savedRevision)] }, alreadyApplied: false };
    return { outcome: 'listed', capabilities: [] };
  } });
  await controller.open(capabilityId);
  controller.updateDraft({ content: 'Saved local edit.\n' });
  assert.equal(controller.hasUnsavedDrafts(), true);
  assert.equal(controller.setLibraryFilters({ query: 'missing' }), true);
  assert.equal(controller.hasUnsavedDrafts(), true);
  await controller.save();
  assert.equal(controller.hasUnsavedDrafts(), false);
});

test('Command Center sends the primary import action to the local analysis workbench and retains Atlas sample entry', async () => {
  const source = await readFile(new URL('../preview/app.mjs', import.meta.url), 'utf8');
  const command = source.match(/function command\(\) \{([\s\S]*?)\n\}/)?.[1] ?? '';
  assert.match(command, /href="analyze\.html#analysis"/);
  assert.match(command, /Open local file analysis/);
  assert.match(command, /Open Atlas sample/);
  assert.match(command, /button\('Open Atlas sample','open-atlas-sample'\)/);
  assert.match(source, /case 'open-atlas-sample':state\.imported=true;state\.scenario='sample';return navigate\('import'\)/);
  assert.doesNotMatch(command, /Import a project/);
});
