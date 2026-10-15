import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkflowLiveFeed } from '../preview/workflow-live.mjs';
import { createWorkflowController } from '../preview/workflow-model.mjs';

test('records actual controller changes only; values excluded, undo visible and source distinct', async () => {
  const feed = createWorkflowLiveFeed({ clock: () => 1000 });
  const controller = createWorkflowController({ playground: true, onChange: state => feed.observe(state) });
  await controller.beginNew();
  controller.addNode('input', { x: 0, y: 0 }); controller.addNode('output', { x: 300, y: 50 });
  controller.connectControl(0, 'next', 1); controller.connectData(0, 'text', 1, 'text');
  const before = feed.snapshot();
  controller.select({ kind: 'node', index: 0 }); controller.setViewport({ zoom: 2 }); controller.setTab('outline');
  assert.equal(controller.editFields({ kind: 'node', index: 0 }, { title: 'private configuration text' }), true);
  assert.deepEqual(feed.snapshot(), before);
  controller.applyFields(); controller.moveNode(0, 10, 40); controller.undo();
  const result = feed.snapshot();
  assert.equal(result.source, 'synthetic-editor'); assert.equal(result.runtime, 'disconnected');
  assert.equal(result.events.filter(event => event.kind === 'node-moved').length, 2);
  assert.deepEqual(result.events.find(event => event.kind === 'node-updated').fields, ['title']);
  assert.doesNotMatch(JSON.stringify(result), /private configuration text/);
  assert.ok(result.events.every(event => event.observedAt === 1000));
});

test('retention, source changes, unknown clock and disposal are bounded and honest', () => {
  const feed = createWorkflowLiveFeed({ clock: () => NaN });
  const state = { session: { draftId: 'draft-one' }, definition: { title: 'Title', nodes: [], controlEdges: [], dataEdges: [] }, layout: { positions: [] } };
  for (let index = 0; index < 50; index++) { state.definition.title = `Title ${index}`; feed.observe(state); }
  assert.equal(feed.snapshot().events.length, 40); assert.equal(feed.snapshot().omitted, 10);
  assert.equal(feed.snapshot().source, 'local-workspace-definition');
  assert.ok(feed.snapshot().events.every(event => event.observedAt === null));
  state.session.draftId = 'draft-two'; feed.observe(state);
  assert.equal(feed.snapshot().events.length, 1); assert.equal(feed.snapshot().events[0].kind, 'graph-opened');
  feed.dispose(); const disposed = feed.snapshot(); feed.observe(state);
  assert.deepEqual(feed.snapshot(), disposed); assert.equal(disposed.source, 'disconnected'); assert.equal(disposed.events.length, 0);
});
