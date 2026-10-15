import test from 'node:test';
import assert from 'node:assert/strict';
import { bindWorkflowView, renderWorkflowView } from '../preview/workflow-view.mjs';
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

function filterRoot(attribute) {
  const listeners = new Map();
  const document = { activeElement: null, addEventListener() {}, removeEventListener() {} };
  const root = {
    ownerDocument: document,
    input: null,
    set innerHTML(html) {
      const match = html.match(new RegExp(`<input[^>]*${attribute}[^>]*value="([^"]*)"`));
      if (!match) { this.input = null; return; }
      const input = {
        value: match[1], selectionStart: 0, selectionEnd: 0,
        matches: selector => selector === `[${attribute}]`,
        focus: () => { document.activeElement = input; },
        setSelectionRange: (start, end) => { input.selectionStart = start; input.selectionEnd = end; },
      };
      this.input = input;
    },
    querySelector: selector => selector === `[${attribute}]` ? root.input : null,
    querySelectorAll: () => [],
    addEventListener: (type, listener) => listeners.set(type, listener),
    removeEventListener: type => listeners.delete(type),
    fire: (type, event) => listeners.get(type)?.(event),
  };
  return root;
}

function workflowMenuRoot() {
  const listeners = new Map();
  const receiver = { closest: () => null, hasAttribute: () => false };
  const popup = { removed: false, remove() { this.removed = true; } };
  const trigger = { dataset: { workflowMenu: 'canvas' }, closest: () => trigger, hasAttribute: () => false, getAttribute: () => null, focus() {} };
  const menuItem = { closest: selector => selector === '[data-workflow-context]' ? popup : null, focus() {} };
  const document = {
    activeElement: null, hidden: false, documentElement: { dataset: {} },
    addEventListener() {}, removeEventListener() {},
    defaultView: { addEventListener() {}, removeEventListener() {}, matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }) },
  };
  const root = {
    ownerDocument: document, html: '', renders: 0,
    set innerHTML(value) { this.html = value; this.renders += 1; }, get innerHTML() { return this.html; },
    addEventListener(type, listener) { listeners.set(type, listener); },
    removeEventListener(type, listener) { if (listeners.get(type) === listener) listeners.delete(type); },
    querySelector(selector) {
      if (selector === '[data-workflow-context]') return this.html.includes('data-workflow-context') ? popup : null;
      if (selector === '[data-workflow-context] button') return this.html.includes('data-workflow-context') ? menuItem : null;
      if (selector.includes('[data-workflow-menu="canvas"]')) return trigger;
      return null;
    },
    querySelectorAll() { return []; },
  };
  return { root, document, listeners, receiver, popup, trigger, menuItem };
}

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

test('shows local Apply and Cancel recovery while pending fields block other workflow actions', () => {
  const fieldEdit = { target: { kind: 'node', index: 1 }, patch: { title: 'Changed locally' } };
  const native = renderWorkflowView(state({ fieldEdit }));
  const playground = renderWorkflowView(state({ playground: true, bridgeAvailable: false, status: 'playground', fieldEdit }));
  for (const html of [native, playground]) {
    assert.match(html, /workflow-field-recovery/);
    assert.match(html, /Apply or Cancel before choosing another module/);
    assert.match(html, /data-workflow-apply/);
    assert.match(html, /data-workflow-cancel/);
    assert.match(html.match(/data-workflow-menu="node:1"[^>]*>/)[0], /disabled/);
    assert.match(html.match(/data-workflow-select-node="1"[^>]*>/)[0], /disabled/);
    assert.match(html.match(/data-workflow-select-edge="control:0"[^>]*>/)[0], /disabled/);
    assert.match(html.match(/workflow-incident[^]*?data-workflow-select-edge="control:0"[^>]*>/)[0], /disabled/);
    const outline = renderWorkflowView(state({ tab: 'outline', fieldEdit }));
    assert.match(outline.match(/data-workflow-select-edge="control:0"[^>]*>/)[0], /disabled/);
    assert.match(outline.match(/data-workflow-select-edge="data:0"[^>]*>/)[0], /disabled/);
    assert.match(html.match(/workflow-status[^>]*>/)[0], /aria-live="polite"/);
  }
});

test('filters only loaded workflow labels and fixed node types with escaped accessible no-match states', () => {
  const loaded = state({ workflows: [{ id: 'workflow:alpha', label: 'Alpha review', intent: 'draft', revisionCount: 1 }] });
  const match = renderWorkflowView(loaded, { libraryFilter: 'branch' });
  assert.doesNotMatch(match, /Alpha review/);
  assert.match(match, /data-workflow-add="branch"/);
  assert.doesNotMatch(match, /data-workflow-add="input"/);
  const hostile = renderWorkflowView(loaded, { libraryFilter: '"<none & missing> 🦀' });
  assert.match(hostile, /No saved workflow matches “&quot;&lt;none &amp; missing&gt; 🦀”/);
  assert.match(hostile, /No node type matches “&quot;&lt;none &amp; missing&gt; 🦀”/);
  assert.doesNotMatch(hostile, /"<none & missing> 🦀/);
  assert.match(renderWorkflowView(state({ status: 'loading', workflows: [] }), { libraryFilter: 'missing' }), /Loading saved workflow records/);
  assert.match(renderWorkflowView(state({ status: 'ready', workflows: [] }), { libraryFilter: 'missing' }), /No saved workflows are available/);
  assert.match(renderWorkflowView(state({ status: 'unavailable', bridgeAvailable: false, workflows: [] }), { libraryFilter: 'missing' }), /Native workflow records are unavailable/);
  assert.equal(loaded.workflows.length, 1);
});

test('keeps the palette query in renderer input without changing controller state', () => {
  const source = state({ workflows: [{ id: 'workflow:alpha', label: 'Alpha review' }] });
  const first = renderWorkflowView(source, { libraryFilter: 'alpha' });
  const rerender = renderWorkflowView(source, { libraryFilter: 'alpha' });
  assert.match(first, /data-workflow-library-filter value="alpha"/);
  assert.match(rerender, /data-workflow-library-filter value="alpha"/);
  assert.deepEqual(source.workflows, [{ id: 'workflow:alpha', label: 'Alpha review' }]);
});

test('bound palette input keeps focus and caret through controller rerenders without invoking a controller mutation', () => {
  const snapshot = state({ workflows: [{ id: 'workflow:alpha', label: 'Alpha review' }] });
  const root = filterRoot('data-workflow-library-filter');
  const binding = bindWorkflowView(root, { getState: () => snapshot });
  root.input.value = 'alpha'; root.input.focus(); root.input.setSelectionRange(2, 4);
  root.fire('input', { target: root.input });
  assert.equal(root.input.value, 'alpha');
  assert.equal(root.ownerDocument.activeElement, root.input);
  assert.deepEqual([root.input.selectionStart, root.input.selectionEnd], [2, 4]);
  binding.render();
  assert.equal(root.input.value, 'alpha');
  assert.equal(root.ownerDocument.activeElement, root.input);
  assert.deepEqual([root.input.selectionStart, root.input.selectionEnd], [2, 4]);
  let stopped = false;
  root.fire('keydown', { key: 'Escape', preventDefault() {}, stopPropagation() { stopped = true; } });
  assert.equal(root.input.value, '');
  assert.equal(stopped, true);
  assert.equal(snapshot.workflows.length, 1);
  binding.dispose();
});

test('workflow menu focus exit removes only its popup and retains the receiving control', () => {
  const fixture = workflowMenuRoot();
  const binding = bindWorkflowView(fixture.root, { getState: () => state(), subscribe: () => () => {} });
  fixture.listeners.get('click')({ target: fixture.trigger });
  const rendersBeforeExit = fixture.root.renders;
  fixture.document.activeElement = fixture.receiver;
  fixture.listeners.get('focusout')({ target: fixture.menuItem, relatedTarget: fixture.receiver });
  assert.equal(fixture.popup.removed, true);
  assert.equal(fixture.root.renders, rendersBeforeExit);
  assert.equal(fixture.document.activeElement, fixture.receiver);
  binding.dispose();
  assert.equal(fixture.listeners.has('focusout'), false);
});

test('renders a synthetic in-memory playground without native controls and escapes authored text', () => {
  const definition = structuredClone(state().definition);
  definition.title = '<Synthetic & hostile>';
  definition.nodes[0].title = '<Synthetic & hostile>';
  const html = renderWorkflowView(state({
    playground: true,
    bridgeAvailable: false,
    status: 'playground',
    definition,
    workflows: [{ id: 'workflow:should-not-render' }],
    capabilities: [{ id: capabilityId, title: 'Should not render' }],
    candidate: { revision: { intent: 'validated' } },
    receipt: { workflowId: 'workflow:should-not-render' },
  }));
  assert.match(html, /SYNTHETIC WORKFLOW PLAYGROUND/);
  assert.match(html, /Synthetic, in memory, not saved, and not inspected/);
  assert.match(html, /&lt;Synthetic &amp; hostile&gt;/);
  assert.doesNotMatch(html, /<Synthetic & hostile>/);
  assert.doesNotMatch(html, /data-workflow-refresh|data-workflow-inspect|data-workflow-commit|data-workflow-compare|data-workflow-capability/);
  assert.doesNotMatch(html, /data-workflow-tab="diff"|Saved workflow history|Native report/);
});

test('keeps native unavailable refusal and the standalone synthetic link', () => {
  const html = renderWorkflowView(state({ bridgeAvailable: false, definition: null, layout: null, selection: null, dirty: false }));
  assert.match(html, /Native workflow authoring is unavailable in this preview/);
  assert.match(html, /href="workflow-playground.html"/);
  assert.match(html.match(/data-workflow-new[^>]*>/)[0], /disabled/);
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
  assert.doesNotMatch(saved.replace(/<[^>]*>/g, ''), /approval|ready to run/i);
});

test('uses six distinct closed glyphs across palette, 2D nodes, and 3D labels', () => {
  const kinds = ['input', 'capability', 'check', 'branch', 'checkpoint', 'output'];
  const glyphs = ['import', 'skill', 'check', 'branch', 'approval', 'bundle'];
  const definition = structuredClone(state().definition);
  definition.nodes = kinds.map((kind, index) => ({ id: kind, title: `${kind} node`, operation: { kind }, inputs: [], outputs: [] }));
  definition.controlEdges = []; definition.dataEdges = [];
  const source = state({ definition, layout: { positions: kinds.map((_, index) => ({ nodeIndex: index, x: index * 40, y: 0 })) } });
  for (const html of [renderWorkflowView(source), renderWorkflowView(source, { sceneMode: 'depth' })]) {
    for (const glyph of glyphs) assert.match(html, new RegExp(`foldline-icon--${glyph}`));
  }
});

test('keeps SVG-only action controls named and local edit history truthful', () => {
  const html = renderWorkflowView(state(), { live: { source: 'local-workspace-definition', omitted: 0, events: [] } });
  for (const name of ['Pan left', 'Pan right', 'Zoom out', 'Zoom in', 'Reset canvas view', 'Canvas actions', 'Actions for &lt;Input &amp; hostile&gt;']) {
    assert.match(html, new RegExp(`aria-label="${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"`));
  }
  assert.match(html, /Local edit history/);
  assert.doesNotMatch(html, /Live design changes/);
  assert.match(html, /Runtime feed: disconnected/);
  assert.match(html, /data-workflow-details="live"/);
});

test('keeps selected module fields primary and flat connections outside the canvas', () => {
  const html = renderWorkflowView(state());
  assert.match(html, /data-workflow-details="configuration" open/);
  assert.match(html, /SELECTED MODULE · capability/);
  assert.ok(html.indexOf('data-workflow-details="connections"') > html.indexOf('data-workflow-graph-stage'));
  assert.match(html, /data-workflow-details="connections"><summary/);
  assert.doesNotMatch(html, /data-workflow-details="connections" open/);
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

test('renders local flexible workspace controls and explicit bounded Fit', () => {
  const html = renderWorkflowView(state(), { workspaceLayout: { desktop: true, left: 240, right: 320, leftMax: 360, rightMax: 440, leftCollapsed: false, rightCollapsed: false } });
  assert.match(html, /data-workspace-workbench/);
  assert.match(html, /--workspace-palette-width:240px/);
  assert.match(html, /data-workspace-resize="left"[^>]*role="separator"[^>]*aria-valuenow="240"/);
  assert.match(html, /data-workspace-reset/);
  assert.match(html, /data-workflow-fit/);
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
test('alternate 3D uses exact shared selection, ports and menus without copying controller records', () => {
  const source = state(); const before = structuredClone(source);
  const html = renderWorkflowView(source, { sceneMode: 'depth' });
  assert.match(html, /data-workflow-scene-canvas/);
  assert.match(html, /data-workflow-scene-id="source"/);
  assert.match(html, /data-workflow-select-node="1" aria-pressed="true"/);
  assert.match(html, /data-workflow-details="ports"/);
  assert.match(html, /data-workflow-data-target="1:text"/);
  assert.match(html, /Design only · no execution/);
  assert.doesNotMatch(html, /<Input & hostile>/);
  assert.deepEqual(source, before);
  const unavailable = renderWorkflowView(source, { sceneMode: 'depth', sceneUnavailable: 'Graphics context lost.' });
  assert.match(unavailable, /Graphics context lost/);
  assert.doesNotMatch(unavailable, /data-workflow-scene-canvas/);
  assert.match(unavailable, /data-workflow-graph-stage/);
});

test('keeps 3D secondary in View controls and preserves escaped full labels and ports', () => {
  const definition = structuredClone(state().definition);
  definition.nodes[0].title = '<Source & hostile>';
  definition.nodes[0].outputs = [{ name: '<payload &>', dataType: 'json' }];
  definition.nodes[1].inputs = [{ name: '<target &>', dataType: 'json' }];
  definition.dataEdges = [{ fromNode: 'source', fromPort: '<payload &>', toNode: 'capability', toPort: '<target &>' }];
  const flat = renderWorkflowView(state({ definition }));
  assert.match(flat, /View controls/);
  assert.match(flat, /data-workflow-mode="depth"[^>]*>Open 3D editor/);
  assert.match(flat, /&lt;Source &amp; hostile&gt;/);
  assert.match(flat, /&lt;payload &amp;&gt;/);
  assert.match(flat, /&lt;target &amp;&gt;/);
  assert.match(flat, /data-workflow-data-source="0:&lt;payload &amp;&gt;"/);
  assert.match(flat, /data-workflow-data-target="1:&lt;target &amp;&gt;"/);
  assert.doesNotMatch(flat, /<Source & hostile>/);
  const depth = renderWorkflowView(state({ definition }), { sceneMode: 'depth' });
  assert.match(depth, /data-workflow-mode="flat"[^>]*>2D editor/);
  assert.match(depth, /data-workflow-scene-overview[^>]*>Overview/);
  assert.match(depth, /data-workflow-scene-focus[^>]*>Focus selection/);
  assert.equal((depth.match(/data-workflow-mode="flat"/g) ?? []).length, 1);
  assert.match(depth, /data-workflow-scene-canvas/);
  assert.doesNotMatch(depth, /data-workflow-camera|Orbit|Zoom 3D|Reset 3D|drag background to orbit/);
  assert.doesNotMatch(depth, /data-workflow-fit/);
});

test('uses named guided scene sections without changing flat viewport controls', () => {
  const flat = renderWorkflowView(state());
  assert.match(flat, /data-workflow-fit/);
  assert.match(flat, /data-workflow-mode="depth"[^>]*>Open 3D editor/);
  const empty = renderWorkflowView(state({ selection: null }), { sceneMode: 'depth' });
  assert.match(empty.match(/data-workflow-scene-focus[^>]*>/)[0], /disabled/);
  const pending = renderWorkflowView(state({ pending: 'inspect' }), { sceneMode: 'depth' });
  assert.match(pending.match(/data-workflow-scene-focus[^>]*>/)[0], /disabled/);
  const selected = renderWorkflowView(state({ selection: { kind: 'data', index: 0 } }), { sceneMode: 'depth' });
  assert.doesNotMatch(selected.match(/data-workflow-scene-focus[^>]*>/)[0], /disabled/);
  assert.match(selected, /Fixed orientation. Select a module or connection, then use Focus selection/);
});

test('keeps 3D fallback local to View controls while flat preview remains available', () => {
  const narrow = '3D unavailable in narrow or forced-color view.';
  const flat = renderWorkflowView(state({ selection: { kind: 'data', index: 0 } }), { sceneMode: 'flat', sceneNarrow: true, sceneUnavailable: narrow });
  assert.doesNotMatch(flat, /2D and Outline remain usable/);
  assert.match(flat, /data-workflow-mode="depth"[^>]*disabled[^>]*>Open 3D editor/);
  assert.match(flat, /3D unavailable in narrow or forced-color view/);
  assert.doesNotMatch(flat.match(/data-workflow-preview[^>]*>/)[0], /disabled/);
  const depth = renderWorkflowView(state({ selection: { kind: 'data', index: 0 } }), { sceneMode: 'depth', sceneUnavailable: 'Graphics context lost.' });
  assert.match(depth, /Graphics context lost\. 2D and Outline remain usable/);
  assert.match(depth, /data-workflow-scene-retry/);
});
