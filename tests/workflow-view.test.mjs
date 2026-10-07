import test from 'node:test';
import assert from 'node:assert/strict';
import { renderWorkflowView } from '../preview/workflow-view.mjs';
import { createWorkflowController } from '../preview/workflow-model.mjs';

const capabilityId = `capability:${'a'.repeat(64)}`;
const revisionId = `revision:${'b'.repeat(64)}`;
const workflowId = `workflow:${'c'.repeat(64)}`;
const draftId = `workflow-draft:${'d'.repeat(64)}`;
const previewId = `workflow-preview:${'e'.repeat(64)}`;
const workflowRevisionId = `workflow-revision:${'f'.repeat(64)}`;
const workspaceId = `workspace:${'1'.repeat(64)}`;
const state = (overrides = {}) => ({
  bridgeAvailable: true, status: 'ready', pending: null, message: 'Records available.', workflows: [], capabilities: [], capabilityDetail: null,
  opened: null, comparison: null, session: { draftId: `workflow-draft:${'c'.repeat(64)}`, workflowId: `workflow:${'d'.repeat(64)}`, parentRevisionId: null },
  definition: { schemaVersion: 'rangoon.workflow-definition.v1', title: '<Unsafe & workflow>', nodes: [
    { id: 'source', title: '<Input & hostile>', operation: { kind: 'input' }, inputs: [], outputs: [{ name: 'text', dataType: 'text' }] },
    { id: 'capability', title: 'Pinned capability', operation: { kind: 'capability', capabilityId, revisionId }, inputs: [{ name: 'text', dataType: 'text' }], outputs: [{ name: 'json', dataType: 'json' }] },
  ], controlEdges: [{ fromNode: 'source', outlet: 'next', toNode: 'capability' }], dataEdges: [{ fromNode: 'source', fromPort: 'text', toNode: 'capability', toPort: 'text' }] },
  layout: { positions: [{ nodeIndex: 0, x: 0, y: 0 }, { nodeIndex: 1, x: 180, y: 20 }] }, dirty: true, candidate: null, acknowledged: false, receipt: null,
  tab: 'canvas', selection: { kind: 'node', index: 1 }, viewport: {}, fieldEdit: null, canUndo: true, canRedo: false, ...overrides,
});

test('renders escaped local graph, six real node types, and separate wire contracts', () => {
  const canvas = renderWorkflowView(state());
  const outline = renderWorkflowView(state({ tab: 'outline' }));
  assert.match(canvas, /&lt;Input &amp; hostile&gt;/);
  assert.doesNotMatch(canvas, /<Input & hostile>/);
  for (const kind of ['input', 'capability', 'check', 'branch', 'checkpoint', 'output']) assert.match(canvas, new RegExp(`data-workflow-add="${kind}"`));
  assert.match(canvas, /Control paths/);
  assert.match(canvas, /Data mappings/);
  assert.match(canvas, /workflow-wire--control/);
  assert.match(canvas, /workflow-wire--data/);
  assert.match(canvas, /workflow-wire-diagram/);
  assert.match(canvas, /workflow-svg-wire--control/);
  assert.match(canvas, /workflow-svg-wire--data/);
  assert.match(canvas, /draggable="true" class="workflow-port workflow-port--output"/);
  assert.match(outline, /data-workflow-control-form/);
  assert.match(outline, /data-workflow-data-form/);
  assert.match(canvas, /New workflow/);
  assert.doesNotMatch(canvas, /Run workflow|Deploy workflow/);
});

test('marks unavailable and pending controls closed without sample records', () => {
  const unavailable = renderWorkflowView(state({ bridgeAvailable: false, workflows: [], definition: null, layout: null, selection: null, dirty: false }));
  assert.match(unavailable, /Native workflow authoring is unavailable/);
  assert.doesNotMatch(unavailable, />0 structural issues</);
  assert.match(unavailable, /No workflow selected/);
  assert.match(unavailable, /Not inspected/);
  assert.match(unavailable.match(/data-workflow-new[^>]*>/)[0], /disabled/);
  assert.match(unavailable, /No saved workflows are available/);
  const pending = renderWorkflowView(state({ pending: 'inspect' }));
  assert.match(pending.match(/data-workflow-inspect="draft"[^>]*>/)[0], /disabled/);
});

test('names historical, candidate, and saved states without readiness claims', () => {
  const historical = renderWorkflowView(state({ opened: { id: 'workflow:history', historical: true, report: { structurallyValid: false, diagnostics: [{ code: 'input_cardinality', nodeIndex: 1 }], referenceStatus: [] } } }));
  assert.match(historical, /Draft/);
  assert.match(historical, /input_cardinality/);
  const candidate = renderWorkflowView(state({ candidate: { revision: { intent: 'validated' }, report: { structurallyValid: true, diagnostics: [] }, plan: { unresolvedReferences: 0 } } }));
  assert.match(candidate, /Validated candidate/);
  assert.match(candidate, /Save validated revision/);
  const saved = renderWorkflowView(state({ receipt: { workflowId: 'workflow:history' } }));
  assert.match(saved, /Saved revision/);
  assert.doesNotMatch(saved, /approval|ready to run/i);
});

test('shows exact capability picker history and draft save contract', () => {
  const html = renderWorkflowView(state({ capabilities: [{ id: capabilityId, title: '<Library & hostile>' }], capabilityDetail: { id: capabilityId, title: '<Capability & hostile>', history: [{ id: revisionId, title: '<Revision & hostile>' }] }, candidate: { intent: 'draft' } }));
  assert.match(html, /&lt;Library &amp; hostile&gt;/);
  assert.match(html, /data-workflow-capability/);
  assert.match(html, /data-workflow-pin=/);
  assert.match(html, /does not approve execution/);
  assert.doesNotMatch(html, /<Capability & hostile>/);
});

test('renders a candidate from the public controller state with revision intent', async () => {
  const bridge = async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') {
      const request = JSON.parse(new TextDecoder().decode(args));
      return {
        outcome: 'save_ready', schemaVersion: 'rangoon.workflow-save-session.v1', draftId, previewId,
        revision: { schemaVersion: 'rangoon.workflow-revision.v1', id: workflowRevisionId, workflowId, parentRevisionId: null, intent: request.intent, definition: request.definition, layout: request.layout },
        report: { schemaVersion: 'rangoon.workflow-validation.v1', structurallyValid: true, definitionId: `workflow-definition:${'2'.repeat(64)}`, diagnostics: [], diagnosticsTruncated: false, topologicalOrder: ['node_1', 'node_2'], dependencies: [], referenceStatus: 'unverified', executionStatus: 'unavailable', authority: 'none' },
        plan: { schemaVersion: 'rangoon.workflow-save-plan.v1', workflowId, revisionId: workflowRevisionId, expectedStateId: workspaceId, expectedHeadId: null, alreadySaved: false, unresolvedReferences: 0 },
      };
    }
    return { outcome: 'listed', workflows: [], capabilities: [] };
  };
  const controller = createWorkflowController({ bridge });
  await controller.beginNew();
  controller.addNode('input', { x: 0, y: 0 });
  controller.addNode('output', { x: 100, y: 0 });
  controller.connectControl(0, 'next', 1);
  controller.connectData(0, 'text', 1, 'text');
  await controller.inspect('draft');
  await controller.inspect('validated');
  const snapshot = controller.getState();
  assert.equal(snapshot.candidate.revision.intent, 'validated');
  assert.equal(snapshot.candidate.intent, undefined);
  const html = renderWorkflowView(snapshot);
  assert.match(html, /Validated candidate/);
  assert.match(html, /Save validated revision/);
});

test('renders exact saved history, current-head append, and comparison controls', () => {
  const older = `workflow-revision:${'3'.repeat(64)}`;
  const current = `workflow-revision:${'4'.repeat(64)}`;
  const html = renderWorkflowView(state({
    opened: {
      head: { id: workflowId, latestRevisionId: current },
      revision: { id: current },
      history: [{ id: older, intent: 'draft' }, { id: current, intent: 'validated' }],
    },
  }));
  assert.match(html, /data-workflow-open-revision=/);
  assert.match(html, /data-workflow-open-current/);
  assert.match(html, /data-workflow-append/);
  assert.match(html, /data-workflow-compare-left/);
  assert.match(html, /data-workflow-compare-right/);
});

test('uses persisted positions and only exposes legal control outlets', () => {
  const html = renderWorkflowView(state({ definition: {
    schemaVersion: 'rangoon.workflow-definition.v1', title: 'Outlet rules',
    nodes: [
      { id: 'branch', title: 'Branch', operation: { kind: 'branch' }, inputs: [], outputs: [] },
      { id: 'output', title: 'Output', operation: { kind: 'output' }, inputs: [], outputs: [] },
    ], controlEdges: [], dataEdges: [],
  }, layout: { positions: [{ nodeIndex: 0, x: 96, y: 48 }, { nodeIndex: 1, x: 320, y: 48 }] } }));
  assert.match(html, /--node-x:96px;--node-y:48px/);
  assert.match(html, /data-workflow-control-source="0:true"/);
  assert.match(html, /data-workflow-control-source="0:false"/);
  assert.doesNotMatch(html, /data-workflow-control-source="0:next"/);
  assert.doesNotMatch(html, /data-workflow-control-source="1:next"/);
});

test('renders transient viewport controls and current transform values', () => {
  const html = renderWorkflowView(state({ viewport: { x: 24, y: -12, zoom: 1.25 } }));
  assert.match(html, /data-workflow-pan="left"/);
  assert.match(html, /data-workflow-zoom="-0.1"/);
  assert.match(html, /data-workflow-viewport-reset/);
  assert.match(html, /--viewport-x:24px;--viewport-y:-12px;--viewport-zoom:1.25/);
});

test('gives canvas actions a workflow-level selection target', () => {
  const html = renderWorkflowView(state(), { menu: { target: 'canvas' } });
  assert.match(html, /data-workflow-menu-action="select:canvas"/);
});

test('labels renamed data wires with their source type', () => {
  const original = state().definition;
  const definition = structuredClone(original);
  definition.nodes[0].outputs = [{ name: 'payload', dataType: 'json' }];
  definition.dataEdges = [{ fromNode: 'source', fromPort: 'payload', toNode: 'capability', toPort: 'text' }];
  const html = renderWorkflowView(state({ definition }));
  assert.match(html, /payload · json/);
});

test('marks saved revisions read-only while keeping selection available', () => {
  const html = renderWorkflowView(state({ tab: 'outline', session: null, opened: { head: { id: workflowId, latestRevisionId: workflowRevisionId }, revision: { id: workflowRevisionId }, history: [] } }));
  assert.match(html, /Saved revisions are read-only/);
  assert.match(html.match(/data-workflow-add="input"[^>]*>/)[0], /disabled/);
  assert.match(html, /draggable="false"[^>]*data-workflow-add-kind="input"/);
  assert.match(html, /data-workflow-select-node="0"/);
  assert.match(html, /data-workflow-control-form><fieldset disabled>/);
});

test('renders ephemeral click-connect status without changing the definition', () => {
  const source = state();
  const html = renderWorkflowView(source, { connect: { kind: 'data', fromIndex: 0, fromPort: 'text' } });
  assert.match(html, /Data source text selected/);
  assert.match(html, /data-workflow-connect-cancel/);
  assert.match(html, /workflow-node--connect-source/);
  assert.equal(source.definition.dataEdges.length, 1);
});
