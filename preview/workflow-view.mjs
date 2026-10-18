import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';
import { createWorkflowDrag } from './workflow-drag.mjs';
import { createWorkspaceLayout, fitWorkflowGraph } from './editor-workspace-layout.mjs';
import { createWorkflowLiveFeed } from './workflow-live.mjs';
import { createWorkflowScene, frameForSelection, projectWorkflowScene, defaultCamera } from './workflow-scene.mjs';

const text = value => escapeText(String(value ?? ''));
const nodeKinds = Object.freeze(['input', 'capability', 'check', 'branch', 'checkpoint', 'output']);
const nodeIcons = Object.freeze({
  input: 'import',
  capability: 'skill',
  check: 'check',
  branch: 'branch',
  checkpoint: 'approval',
  output: 'bundle',
});
const nodeTypeDetails = Object.freeze({
  input: 'Caller-supplied data',
  capability: 'Pinned saved capability revision',
  check: 'Static text check',
  branch: 'True / false control',
  checkpoint: 'Human checkpoint declaration',
  output: 'Typed workflow output',
});
const tabs = Object.freeze(['canvas', 'outline', 'definition', 'diff']);
const bindingState = new WeakMap();

const short = value => {
  const source = String(value ?? 'Unavailable');
  return source.length > 26 ? `${source.slice(0, 12)}…${source.slice(-9)}` : source;
};
const asArray = value => Array.isArray(value) ? value : [];
const playground = state => state.playground === true;
const disabled = state => Boolean(state.pending) || (!state.bridgeAvailable && !playground(state));
const nodeList = state => asArray(state.definition?.nodes);
const controlEdges = state => asArray(state.definition?.controlEdges);
const dataEdges = state => asArray(state.definition?.dataEdges);
const nodeTitle = (node, index) => node?.title || `${node?.operation?.kind ?? node?.kind ?? 'Node'} ${index + 1}`;
const nodeKind = node => node?.operation?.kind ?? node?.kind ?? 'input';
const portName = port => typeof port === 'string' ? port : port?.name ?? 'port';
const portType = port => typeof port === 'string' ? '' : port?.dataType ?? '';
const controlLabel = edge => edge?.outlet ?? 'next';
const dataLabel = (state, edge) => {
  if (edge === undefined) { edge = state; state = {}; }
  const source = nodeList(state).find(node => node?.id === edge?.fromNode);
  const port = asArray(source?.outputs).find(item => portName(item) === edge?.fromPort);
  const name = edge?.fromPort ?? edge?.toPort ?? 'data';
  return portType(port) ? `${name} · ${portType(port)}` : name;
};
const selection = state => state.selection ?? null;
const sceneSelectionValid = state => {
  const current = selection(state);
  if (!current || !Number.isInteger(current.index)) return false;
  if (current.kind === 'node') return current.index >= 0 && current.index < nodeList(state).length;
  if (current.kind === 'control') return current.index >= 0 && current.index < controlEdges(state).length;
  if (current.kind === 'data') return current.index >= 0 && current.index < dataEdges(state).length;
  return false;
};
const isSelected = (state, kind, index) => selection(state)?.kind === kind && Number(selection(state)?.index) === index;
const actionDisabled = state => disabled(state) || state.fieldEdit ? 'disabled' : '';
const fieldActionDisabled = state => disabled(state) ? 'disabled' : '';
const structureDisabled = state => disabled(state) || state.fieldEdit || !state.session ? 'disabled' : '';
const json = value => text(value ? JSON.stringify(value, null, 2) : 'No authored definition is open.');
const fieldTarget = state => state.fieldEdit?.target ?? (selection(state)?.kind === 'node' ? { kind: 'node', index: selection(state).index } : { kind: 'workflow' });
const fieldPatch = state => state.fieldEdit?.patch ?? {};
const candidateReport = state => state.candidate?.report ?? null;
const candidateIntent = state => state.candidate?.revision?.intent ?? null;
const visibleTabs = state => playground(state) ? tabs.filter(tab => tab !== 'diff') : tabs;
const normalizedFilter = value => String(value ?? '').trim().toLocaleLowerCase();
const matchesFilter = (query, ...values) => !query || values.some(value => String(value ?? '').toLocaleLowerCase().includes(query));
const nodeIcon = node => nodeIcons[nodeKind(node)] ?? 'workflow';
const iconButton = (name, label, attributes = '') => `<button type="button" class="workflow-icon-button" aria-label="${label}" ${attributes}>${icon(name, { size: 16 })}</button>`;
const disclosure = content => `<summary><span class="workflow-disclosure-icon">${icon('chevron', { size: 16 })}</span>${content}</summary>`;

function statusLabel(state) {
  if (state.receipt) return 'Saved revision';
  if (candidateIntent(state) === 'validated') return 'Validated candidate';
  if (state.opened && !state.session) return 'Saved revision';
  return 'Draft';
}

function workflowSummary(state) {
  if (playground(state)) return `<div class="workflow-summary" aria-label="Workflow state"><span>Synthetic</span><span>In memory</span><span>Not saved</span><span>Not inspected</span><span>No native or provider action</span></div>`;
  if (!state.definition) return `<div class="workflow-summary" aria-label="Workflow state"><span>No workflow selected</span><span>Not inspected</span><span>References unavailable</span><span>${state.pending ? `Working: ${text(state.pending)}` : 'Open a saved workflow or begin a local draft'}</span></div>`;
  const report = candidateReport(state) ?? state.opened?.report;
  const issues = report ? asArray(report.diagnostics).length : null;
  const references = state.candidate?.plan ? Number(state.candidate.plan.unresolvedReferences) : null;
  return `<div class="workflow-summary" aria-label="Workflow state"><span>${text(statusLabel(state))}</span><span>${issues === null ? 'Not inspected' : `${issues} structural issue${issues === 1 ? '' : 's'}`}</span><span>${references === null ? 'References not inspected' : `${references} unresolved reference${references === 1 ? '' : 's'}`}</span><span>${state.pending ? `Working: ${text(state.pending)}` : state.dirty ? 'Local edits not saved' : 'No pending native action'}</span></div>`;
}

function library(state, ui = {}) {
  const filterValue = String(ui.libraryFilter ?? '');
  const filter = normalizedFilter(filterValue);
  const nodeCards = nodeKinds.filter(kind => matchesFilter(filter, kind, nodeTypeDetails[kind]));
  const nodeCard = kind => `<article class="workflow-node-type" draggable="${structureDisabled(state) ? 'false' : 'true'}" data-workflow-add-kind="${kind}"><div class="workflow-node-type-icon">${icon(nodeIcons[kind], { size: 16 })}</div><span><strong>${text(kind)}</strong><small>${text(nodeTypeDetails[kind])}</small></span><button type="button" class="analysis-button analysis-button--small" data-workflow-add="${kind}" ${structureDisabled(state)}>Add</button></article>`;
  const filterField = `<label class="workflow-library-filter" for="workflow-library-filter"><span>Find workflow or node type</span><input id="workflow-library-filter" type="search" data-workflow-library-filter value="${text(filterValue)}" placeholder="Find workflow or node type" autocomplete="off" spellcheck="false"></label>`;
  if (playground(state)) return `<aside class="workflow-library" aria-label="Synthetic workflow builder"><header class="workflow-panel-head"><div><p class="analysis-kicker">SYNTHETIC</p><h2>In-memory builder</h2></div></header><p class="workflow-library-note">This canvas has no native records, history, capability library, provider action, or save path. Refreshing or leaving discards it.</p>${filterField}<section class="workflow-node-library"><p class="analysis-kicker">ADD NODE</p><h3>Definition nodes</h3><p>Add and connect local synthetic nodes. Each action changes only this browser memory.</p>${nodeCards.length ? nodeCards.map(nodeCard).join('') : `<p class="workflow-empty" role="status">No node type matches “${text(filterValue)}”.</p>`}</section></aside>`;
  const workflows = asArray(state.workflows);
  const filteredWorkflows = workflows.filter(item => matchesFilter(filter, item.label, item.title, item.id));
  const recordsMessage = state.status === 'unavailable'
    ? 'Native workflow records are unavailable. No saved workflows are available.'
    : state.status === 'loading'
      ? 'Loading saved workflow records…'
      : state.status === 'failed'
        ? 'Saved workflow records are unavailable. Existing local work stays open.'
        : !workflows.length
          ? 'No saved workflows are available.'
          : filter && !filteredWorkflows.length
            ? `No saved workflow matches “${text(filterValue)}”.`
            : '';
  const workflowRows = filteredWorkflows.map(item => `<button type="button" class="workflow-library-item${state.opened?.id === item.id ? ' workflow-library-item--active' : ''}" data-workflow-open="${text(item.id)}" ${actionDisabled(state)}><strong>${text(item.label ?? item.title ?? 'Untitled workflow')}</strong><small>${text(item.intent ?? 'draft')} · ${Number(item.revisionCount ?? 0)} saved revision${Number(item.revisionCount ?? 0) === 1 ? '' : 's'}</small></button>`).join('');
  return `<aside class="workflow-library" aria-label="Workflow library"><header class="workflow-panel-head"><div><p class="analysis-kicker">LIBRARY</p><h2>Saved workflows</h2></div><button type="button" class="analysis-button analysis-button--small" data-workflow-refresh ${actionDisabled(state)}>Refresh</button></header><p class="workflow-library-note">Records shown here come from the local workspace. Historical opening is read-only.</p>${filterField}<div class="workflow-library-list">${workflowRows || `<p class="workflow-empty" role="status">${recordsMessage}</p>`}</div><section class="workflow-node-library"><p class="analysis-kicker">ADD NODE</p><h3>Definition nodes</h3><p>Drag a card to the canvas or use Add. Each action changes only the local draft.</p>${nodeCards.length ? nodeCards.map(nodeCard).join('') : `<p class="workflow-empty" role="status">No node type matches “${text(filterValue)}”.</p>`}</section></aside>`;
}

function positionFor(state, index) {
  return asArray(state.layout?.positions).find(position => Number(position?.nodeIndex) === index) ?? { x: 0, y: 0 };
}

function controlOutlets(node) {
  if (nodeKind(node) === 'branch') return ['true', 'false'];
  if (nodeKind(node) === 'output') return [];
  return ['next'];
}

function nodeCard(state, node, index, connect = null) {
  const selected = isSelected(state, 'node', index);
  const inputs = asArray(node?.inputs);
  const outputs = asArray(node?.outputs);
  const position = positionFor(state, index);
  const inputPorts = inputs.length
    ? inputs.map(port => `<button type="button" class="workflow-port workflow-port--input" data-workflow-data-target="${index}:${text(portName(port))}" ${structureDisabled(state)}>${text(portName(port))}<small>${text(portType(port))}</small></button>`).join('')
    : '<small>None</small>';
  const outputPorts = outputs.length
    ? outputs.map(port => `<button type="button" draggable="${structureDisabled(state) ? 'false' : 'true'}" class="workflow-port workflow-port--output" data-workflow-data-source="${index}:${text(portName(port))}" ${structureDisabled(state)}>${text(portName(port))}<small>${text(portType(port))}</small></button>`).join('')
    : '<small>None</small>';
  const outlets = controlOutlets(node).map(outlet => `<button type="button" draggable="${structureDisabled(state) ? 'false' : 'true'}" class="workflow-control-port" data-workflow-control-source="${index}:${outlet}" ${structureDisabled(state)}>Control · ${text(outlet)}</button>`).join('');
  const source = connect?.fromIndex === index;
  return `<article class="workflow-node${selected ? ' workflow-node--selected' : ''}${source ? ' workflow-node--connect-source' : ''}" data-workflow-node="${index}" tabindex="0" aria-label="${text(nodeTitle(node, index))}, ${text(nodeKind(node))} node${source ? ', connection source selected' : ''}" style="--node-x:${Number(position.x ?? 0)}px;--node-y:${Number(position.y ?? 0)}px"><header><button type="button" class="workflow-node-select" data-workflow-select-node="${index}" data-workflow-drag-handle ${actionDisabled(state)}><span class="workflow-node-kind-icon">${icon(nodeIcon(node), { size: 16 })}</span><strong>${text(nodeTitle(node, index))}</strong><small>${text(nodeKind(node))}</small></button>${iconButton('more', `Actions for ${text(nodeTitle(node, index))}`, `data-workflow-menu="node:${index}" ${actionDisabled(state)}`)}</header><div class="workflow-node-ports"><div><span>Inputs</span>${inputPorts}</div><div><span>Outputs</span>${outputPorts}${outlets}</div></div></article>`;
}

function edgeList(state) {
  const controls = controlEdges(state);
  const data = dataEdges(state);
  const edge = (kind, item, index) => `<div class="workflow-wire-row"><button type="button" class="workflow-wire workflow-wire--${kind}${isSelected(state, kind, index) ? ' workflow-wire--selected' : ''}" data-workflow-select-edge="${kind}:${index}" ${actionDisabled(state)}><span class="workflow-wire-line"></span><strong>${text(kind === 'control' ? controlLabel(item) : dataLabel(state, item))}</strong><small>${kind === 'control' ? `${text(item.fromNode)} → ${text(item.toNode)}` : `${text(item.fromNode)}.${text(item.fromPort)} → ${text(item.toNode)}.${text(item.toPort)}`}</small></button>${iconButton('more', `Actions for ${kind} connection ${index + 1}`, `data-workflow-menu="${kind}:${index}" ${actionDisabled(state)}`)}</div>`;
  return `<section class="workflow-wires" aria-label="Graph connections"><div class="workflow-wire-group workflow-wire-group--control"><h3>Control paths</h3>${controls.length ? controls.map((item, index) => edge('control', item, index)).join('') : '<p>No control connections yet.</p>'}</div><div class="workflow-wire-group workflow-wire-group--data"><h3>Data mappings</h3>${data.length ? data.map((item, index) => edge('data', item, index)).join('') : '<p>No typed data mappings yet.</p>'}</div></section>`;
}

function wireDiagram(state) {
  const path = (kind, edge, index) => `<g class="workflow-svg-wire-group"><path class="workflow-svg-wire workflow-svg-wire--${kind}${isSelected(state, kind, index) ? ' workflow-svg-wire--selected' : ''}" data-workflow-svg-wire="${kind}:${index}" marker-end="url(#workflow-arrow-${kind})"/><text class="workflow-svg-label workflow-svg-label--${kind}" data-workflow-svg-label="${kind}:${index}">${text(kind === 'control' ? controlLabel(edge) : dataLabel(state, edge))}</text></g>`;
  return `<svg class="workflow-wire-diagram" aria-hidden="true" preserveAspectRatio="none"><defs><marker id="workflow-arrow-control" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto"><path d="M 0 0 L 8 4 L 0 8 z"/></marker><marker id="workflow-arrow-data" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto"><path d="M 0 0 L 8 4 L 0 8 z"/></marker></defs>${controlEdges(state).map((edge, index) => path('control', edge, index)).join('')}${dataEdges(state).map((edge, index) => path('data', edge, index)).join('')}<circle class="workflow-design-marker" data-workflow-design-marker r="5" visibility="hidden"/></svg>`;
}

function graphBounds(state) {
  const positions = nodeList(state).map((node, index) => positionFor(state, index));
  const xs = positions.map(position => Number(position.x ?? 0));
  const ys = positions.map(position => Number(position.y ?? 0));
  const padding = 60;
  const minX = Math.min(0, ...xs) - padding;
  const minY = Math.min(0, ...ys) - padding;
  const maxX = Math.max(420, ...xs.map(value => value + 280)) + padding;
  const maxY = Math.max(280, ...ys.map(value => value + 190)) + padding;
  return { offsetX: -minX, offsetY: -minY, width: maxX - minX, height: maxY - minY };
}

function sceneNode(state, node, index, connect) {
  return `<article class="workflow-scene-node${isSelected(state, 'node', index) ? ' workflow-scene-node--selected' : ''}" data-workflow-node="${index}" data-workflow-scene-id="${text(node.id)}"><button type="button" class="workflow-node-select" data-workflow-select-node="${index}" aria-pressed="${isSelected(state, 'node', index)}" ${actionDisabled(state)}>${icon(nodeIcon(node), { size: 16 })}<strong>${text(nodeTitle(node, index))}</strong><small>${text(nodeKind(node))}${connect?.fromIndex === index ? ' · source' : ''}</small></button></article>`;
}

function nodeConnections(state, node, index) {
  if (!node) return '';
  const ports = (direction, values) => asArray(values).map(port => `<button type="button" class="workflow-port" data-workflow-data-${direction}="${index}:${text(portName(port))}" ${structureDisabled(state)}>${direction === 'source' ? 'Out' : 'In'} · ${text(portName(port))}<small>${text(portType(port))}</small></button>`).join('');
  const outgoing = controlOutlets(node).map(outlet => `<button type="button" class="workflow-control-port" data-workflow-control-source="${index}:${outlet}" ${structureDisabled(state)}>Control · ${text(outlet)}</button>`).join('');
  const incident = [...controlEdges(state).map((edge, i) => ({ edge, kind: 'control', i })), ...dataEdges(state).map((edge, i) => ({ edge, kind: 'data', i }))].filter(item => item.edge.fromNode === node.id || item.edge.toNode === node.id);
  return `<details class="workflow-detail-section workflow-detail-section--primary" data-workflow-details="ports" open>${disclosure('Ports and connections')}<p>${text(nodeTypeDetails[nodeKind(node)])}. Select an output, then its destination. These edit a definition only.</p><div class="workflow-inspector-ports">${ports('target', node.inputs)}${ports('source', node.outputs)}${outgoing || '<small>No control outlet</small>'}</div><div class="workflow-incident">${incident.map(({ edge, kind, i }) => `<button type="button" data-workflow-select-edge="${kind}:${i}" ${actionDisabled(state)}>${text(kind)} · ${text(edge.fromNode)} → ${text(edge.toNode)}</button>`).join('') || '<p>No connections yet.</p>'}</div></details>`;
}

function liveDesign(state, live) {
  if (!live) return '';
  const source = live.source === 'synthetic-editor' ? 'Synthetic editor' : live.source === 'local-workspace-definition' ? 'Local workspace definition' : 'No local graph';
  return `<details class="workflow-live" data-workflow-details="live">${disclosure(`<strong>Local edit history</strong><span>${text(source)} · ${nodeList(state).length} nodes · ${controlEdges(state).length + dataEdges(state).length} connections</span>`)}<p class="workflow-live-boundary">Runtime feed: disconnected. These are actual local UI changes, not agent activity, approvals, execution or audit receipts.</p><ol aria-label="Local graph change history">${live.events.slice(-10).reverse().map(event => `<li><span>${event.observedAt === null ? 'Time unknown' : text(new Date(event.observedAt).toISOString().slice(11, 19))} · #${event.sequence}</span><strong>${text(event.kind.replaceAll('-', ' '))}</strong><small>${text(event.subjectIds.join(' → '))}${event.fields.length ? ` · ${text(event.fields.join(', '))}` : ''}</small></li>`).join('')}</ol><p>${live.omitted ? `${live.omitted} older events omitted from this bounded session journal. ` : ''}Showing up to ten newest changes; stored in memory only.</p></details>`;
}

function canvas(state, ui = {}) {
  const nodes = nodeList(state);
  const connect = ui.connect ?? null;
  const viewport = state.viewport ?? { x: 0, y: 0, zoom: 1 };
  const bounds = graphBounds(state);
  const projection = projectWorkflowScene(state);
  const depth = ui.sceneMode === 'depth' && projection.available && !ui.sceneUnavailable;
  const sceneReason = ui.sceneUnavailable || (projection.available ? '' : projection.reason);
  const focusDisabled = disabled(state) || !depth || !sceneSelectionValid(state) ? 'disabled' : '';
  const depthControl = `<button type="button" class="analysis-button analysis-button--primary" data-workflow-mode="flat" aria-pressed="${!depth}">2D editor</button>`;
  const sceneActions = ui.sceneMode === 'depth'
    ? `${depthControl}<button type="button" data-workflow-scene-overview>Overview</button><button type="button" data-workflow-scene-focus ${focusDisabled}>Focus selection</button>`
    : '';
  const modeBar = `<div class="workflow-view-bar" role="group" aria-label="Graph presentation">${sceneActions}<button type="button" data-workflow-preview ${!selection(state) || !['control', 'data'].includes(selection(state).kind) || ui.sceneMode === 'depth' && ui.sceneUnavailable ? 'disabled' : ''}>${ui.motionReduced ? 'Highlight connection' : 'Preview connection'}</button>${ui.sceneMode === 'depth' && ui.sceneUnavailable && !ui.sceneNarrow ? '<button type="button" data-workflow-scene-retry>Retry 3D</button>' : ''}<span>${text((ui.sceneMode === 'depth' ? ui.sceneNotice : '') || ui.previewNotice || 'Design only · no execution')}</span></div>`;
  const depthStage = `<div class="workflow-scene" data-workflow-scene data-workflow-canvas tabindex="0" role="group" aria-label="3D workflow graph"><canvas data-workflow-scene-canvas aria-hidden="true"></canvas><div class="workflow-scene-labels">${nodes.map((node,index) => sceneNode(state,node,index,connect)).join('')}</div><p class="workflow-scene-caption">Fixed orientation. Select a module or connection, then use Focus selection to inspect its section.</p></div>`;
  const sceneNotice = ui.sceneMode === 'depth' && sceneReason ? `<p class="workflow-scene-notice" role="status">${text(sceneReason)} 2D and Outline remain usable.</p>` : depth && projection.overlap ? '<p class="workflow-scene-notice">Some saved positions overlap. Use Outline or move nodes in 2D; every node remains in the inspector.</p>' : '';
  const connectionStatus = connect ? `<p class="workflow-connect-status" role="status">${connect.kind === 'data' ? `Data source ${text(connect.fromPort)} selected. Choose a compatible input port.` : `Control outlet ${text(connect.outlet)} selected. Choose the next node.`}<button type="button" class="analysis-button analysis-button--small" data-workflow-connect-cancel>Cancel connection</button></p>` : '';
  const localLabel = playground(state) ? 'Synthetic graph' : 'Local graph';
  const emptyCopy = playground(state) ? 'Start a synthetic in-memory definition. Refreshing or leaving discards it; it is not saved or inspected.' : 'Add one of the six node types. Incomplete graphs can still be inspected and saved as drafts.';
  const fitDisabled = actionDisabled(state) || !nodes.length ? 'disabled' : '';
  const fitNotice = ui.fitNotice ? `<p class="workflow-fit-notice" role="status">${text(ui.fitNotice)}</p>` : '';
  const flatViewControls = `${iconButton('panleft', 'Pan left', `data-workflow-pan="left" ${actionDisabled(state)}`)}${iconButton('panright', 'Pan right', `data-workflow-pan="right" ${actionDisabled(state)}`)}${iconButton('zoomout', 'Zoom out', `data-workflow-zoom="-0.1" ${actionDisabled(state)}`)}<span>${Math.round(Number(viewport.zoom ?? 1) * 100)}%</span>${iconButton('zoomin', 'Zoom in', `data-workflow-zoom="0.1" ${actionDisabled(state)}`)}${iconButton('reset', 'Reset canvas view', `data-workflow-viewport-reset ${actionDisabled(state)}`)}`;
  const sceneControl = `<span class="workflow-scene-choice"><button type="button" data-workflow-mode="depth" aria-pressed="${depth}" ${!projection.available || ui.sceneUnavailable ? 'disabled' : ''}>Open 3D editor</button>${sceneReason ? `<small>${text(sceneReason)}</small>` : ''}${ui.sceneMode === 'flat' && ui.sceneUnavailable && !ui.sceneNarrow ? '<button type="button" data-workflow-scene-retry>Retry 3D</button>' : ''}</span>`;
  const viewControls = `${flatViewControls}${sceneControl}`;
  const flatStage = `<div class="workflow-canvas${nodes.length ? '' : ' workflow-canvas--empty'}" data-workflow-canvas tabindex="0">${nodes.length ? `<div class="workflow-graph-stage" data-workflow-graph-stage style="width:${bounds.width}px;height:${bounds.height}px;--graph-offset-x:${bounds.offsetX}px;--graph-offset-y:${bounds.offsetY}px;--viewport-x:${Number(viewport.x ?? 0)}px;--viewport-y:${Number(viewport.y ?? 0)}px;--viewport-zoom:${Number(viewport.zoom ?? 1)}">${wireDiagram(state)}${nodes.map((node, index) => nodeCard(state, node, index, connect)).join('')}</div>` : `<div class="workflow-canvas-empty"><h3>Start a ${playground(state) ? 'synthetic canvas' : 'local definition'}</h3><p>${emptyCopy}</p></div>`}</div>`;
  const connections = nodes.length ? `<details class="workflow-scene-connections" data-workflow-details="connections">${disclosure(`Connections · ${controlEdges(state).length} control · ${dataEdges(state).length} data`)}${edgeList(state)}</details>` : '';
  return `<section class="workflow-canvas-panel${depth ? ' workflow-canvas-panel--depth' : ''}" aria-label="Workflow canvas"><header><div><p class="analysis-kicker">CANVAS</p><h2>${localLabel}</h2></div><div class="workflow-canvas-actions">${modeBar}${depth ? '' : `<details class="workflow-view-menu" data-workflow-details="view">${disclosure('View controls')}<div aria-label="2D canvas controls">${viewControls}</div></details><button type="button" class="analysis-button analysis-button--small" data-workflow-fit ${fitDisabled}>Fit graph</button>`}${iconButton('more', 'Canvas actions', `data-workflow-menu="canvas" ${actionDisabled(state)}`)}</div></header><p class="workflow-canvas-legend">Control: solid · Data: dashed</p>${sceneNotice}${fitNotice}${connectionStatus}${depth ? depthStage : flatStage}<details class="workflow-graph-guide">${disclosure('Graph guide')}<p>Arrows show direction. In the flat editor, drag module headers to move nodes and use labelled ports to connect. Outline provides keyboard forms. 3D uses a fixed orientation; Overview frames the graph and Focus selection frames only the selected section.</p></details>${liveDesign(state, ui.live)}${connections}</section>`;
}

function outline(state) {
  const nodes = nodeList(state); const controls = controlEdges(state); const data = dataEdges(state);
  const options = nodes.map((node, index) => `<option value="${index}">${text(nodeTitle(node, index))}</option>`).join('');
  const orderedNodes = nodes.map((node, index) => `<li class="${isSelected(state, 'node', index) ? 'workflow-outline--selected' : ''}"><button type="button" data-workflow-select-node="${index}" ${actionDisabled(state)}><strong>${index + 1}. ${text(nodeTitle(node, index))}</strong><span>${text(nodeKind(node))}</span></button><div class="workflow-outline-actions"><button type="button" data-workflow-reorder="${index}:-1" ${structureDisabled(state) || index === 0 ? 'disabled' : ''}>Move earlier</button><button type="button" data-workflow-reorder="${index}:1" ${structureDisabled(state) || index === nodes.length - 1 ? 'disabled' : ''}>Move later</button><button type="button" data-workflow-menu="node:${index}" ${actionDisabled(state)}>Actions</button></div></li>`).join('') || '<li>No nodes in the current local definition.</li>';
  const controlsList = controls.map((edge, index) => `<div><button type="button" data-workflow-select-edge="control:${index}" ${actionDisabled(state)}>Control · ${text(controlLabel(edge))}</button><button type="button" data-workflow-remove-edge="control:${index}" ${structureDisabled(state)}>Remove</button></div>`).join('');
  const dataList = data.map((edge, index) => `<div><button type="button" data-workflow-select-edge="data:${index}" ${actionDisabled(state)}>Data · ${text(dataLabel(state, edge))}</button><button type="button" data-workflow-remove-edge="data:${index}" ${structureDisabled(state)}>Remove</button></div>`).join('');
  return `<section class="workflow-outline" aria-label="Workflow outline"><header><p class="analysis-kicker">OUTLINE</p><h2>Complete editable graph</h2><p>Use this representation when the canvas is narrow. Connections remain explicit and labelled.</p></header><ol>${orderedNodes}</ol><section class="workflow-connection-forms"><h3>Connect labelled ports</h3><p>Choose <strong>next</strong> for input, capability, check, or checkpoint nodes. Choose <strong>true</strong> or <strong>false</strong> for branch nodes. Output nodes have no control outlet; refused combinations remain unchanged.</p><form data-workflow-control-form><fieldset ${structureDisabled(state)}><label>From node <select name="from">${options}</select></label><label>Outlet <select name="outlet"><option>next</option><option>true</option><option>false</option></select></label><label>To node <select name="to">${options}</select></label><button type="submit" class="analysis-button analysis-button--small">Connect control</button></fieldset></form><form data-workflow-data-form><fieldset ${structureDisabled(state)}><label>From node <select name="from">${options}</select></label><label>Output <input name="fromPort" placeholder="Output port name" required></label><label>To node <select name="to">${options}</select></label><label>Input <input name="toPort" placeholder="Input port name" required></label><button type="submit" class="analysis-button analysis-button--small">Connect data</button></fieldset></form></section><section class="workflow-edge-outline"><h3>Connections</h3>${controlsList}${dataList}</section></section>`;
}

function definitionView(state) {
  const candidate = state.candidate?.revision?.definition ?? state.candidate?.definition;
  return `<section class="workflow-definition" aria-label="Workflow definition"><p class="analysis-kicker">DEFINITION</p><h2>Authored local JSON</h2><p>This text is the current authored definition. ${candidate ? 'The expandable record is native canonical candidate data.' : 'No native candidate is retained.'}</p><pre data-workflow-scroll="definition">${json(state.definition)}</pre>${candidate ? `<details>${disclosure('Native canonical candidate definition')}<pre>${json(candidate)}</pre></details>` : ''}</section>`;
}

function diffView(state) {
  const comparison = state.comparison;
  return `<section class="workflow-diff" aria-label="Workflow revision comparison"><p class="analysis-kicker">DIFF</p><h2>Read-only revision comparison</h2>${comparison ? `<p>${text(comparison.summary ?? 'Selected exact saved revisions are shown below.')}</p><dl><div><dt>Left revision</dt><dd class="analysis-mono">${text(short(comparison.left?.revision?.id ?? comparison.leftRevisionId))}</dd></div><div><dt>Right revision</dt><dd class="analysis-mono">${text(short(comparison.right?.revision?.id ?? comparison.rightRevisionId))}</dd></div></dl><pre>${json(comparison.changes ?? comparison)}</pre>` : '<p>Select exact saved revisions in the inspector to compare them. Diff is read-only and does not establish semantic equivalence or authorization.</p>'}</section>`;
}

function nodeFieldMarkup(state, node, patch) {
  const fieldDisabled = disabled(state) || !state.session;
  const operation = node?.operation ?? {};
  const kind = patch.operation?.kind ?? operation.kind ?? nodeKind(node);
  const prompt = patch.operation?.prompt ?? patch.operation?.check?.needle ?? patch.prompt ?? operation.prompt ?? operation.check?.needle ?? '';
  const checkKind = patch.operation?.check?.kind ?? operation.check?.kind ?? 'contains_text';
  const ports = (name, source) => {
    const value = Array.isArray(patch[name]) ? patch[name] : source;
    return text(asArray(value).map(port => `${portName(port)}:${portType(port) || 'text'}`).join(', '));
  };
  return `<label>Operation<select data-workflow-field="operation.kind" ${fieldDisabled ? 'disabled' : ''}>${nodeKinds.map(candidate => `<option value="${candidate}" ${kind === candidate ? 'selected' : ''}>${candidate}</option>`).join('')}</select></label>${kind === 'checkpoint' ? `<label>Checkpoint prompt<textarea data-workflow-field="prompt" ${fieldDisabled ? 'disabled' : ''}>${text(prompt)}</textarea></label>` : ''}${kind === 'check' ? `<label>Check subtype<select data-workflow-field="check.kind" ${fieldDisabled ? 'disabled' : ''}><option value="contains_text" ${checkKind === 'contains_text' ? 'selected' : ''}>contains text</option><option value="non_empty" ${checkKind === 'non_empty' ? 'selected' : ''}>non-empty</option></select></label><label>Check text<textarea data-workflow-field="prompt" ${fieldDisabled ? 'disabled' : ''}>${text(prompt)}</textarea></label>` : ''}<label>Input ports <small>name:type, comma separated</small><textarea data-workflow-field="inputs" ${fieldDisabled ? 'disabled' : ''}>${ports('inputs', node?.inputs)}</textarea></label><label>Output ports <small>name:type, comma separated</small><textarea data-workflow-field="outputs" ${fieldDisabled ? 'disabled' : ''}>${ports('outputs', node?.outputs)}</textarea></label>`;
}

function edgeDetails(state) {
  const selected = selection(state);
  if (!selected || !['control', 'data'].includes(selected.kind)) return '';
  const edge = (selected.kind === 'control' ? controlEdges(state) : dataEdges(state))[selected.index];
  if (!edge) return '';
  const from = nodeList(state).find(node => node.id === edge.fromNode), to = nodeList(state).find(node => node.id === edge.toNode);
  return `<details class="workflow-detail-section workflow-connection-detail workflow-detail-section--primary" data-workflow-details="edge" open>${disclosure('Connection details')}<p>${text(selected.kind === 'control' ? 'Control path: declares which node follows this outlet.' : 'Data mapping: connects two exact typed ports. It does not execute a tool.')}</p><dl><dt>From</dt><dd>${text(from?.title ?? edge.fromNode)} · ${text(edge.fromNode)}</dd><dt>To</dt><dd>${text(to?.title ?? edge.toNode)} · ${text(edge.toNode)}</dd><dt>${selected.kind === 'control' ? 'Outlet' : 'Ports'}</dt><dd>${text(selected.kind === 'control' ? edge.outlet : `${edge.fromPort} → ${edge.toPort}`)}</dd></dl><p>Design only. No request, approval, authorization or execution is occurring.</p><button type="button" class="analysis-button analysis-button--small" data-workflow-preview>Preview connection</button></details>`;
}

function inspector(state) {
  if (playground(state)) return playgroundInspector(state);
  const target = fieldTarget(state); const node = target.kind === 'node' ? nodeList(state)[target.index] : null; const patch = fieldPatch(state); const report = candidateReport(state) ?? state.opened?.report;
  const fieldsLocked = disabled(state) || !state.session;
  const capabilities = asArray(state.capabilities); const detail = state.capabilityDetail;
  const candidate = state.candidate;
  const canValidated = Boolean(candidate?.report?.structurallyValid) && Number(candidate?.plan?.unresolvedReferences) === 0;
  const recovery = state.fieldEdit ? '<p class="workflow-field-recovery" role="status">Pending field edits stay local. Apply or Cancel before choosing another module.</p>' : '';
  return `<aside class="workflow-inspector" aria-label="Workflow inspector"><header class="workflow-panel-head"><div><p class="analysis-kicker">${target.kind === 'node' ? `SELECTED MODULE · ${text(nodeKind(node))}` : 'INSPECTOR'}</p><h2>${target.kind === 'node' ? text(nodeTitle(node, target.index)) : 'Workflow details'}</h2></div><button type="button" class="analysis-button analysis-button--small" data-workflow-menu="${target.kind}:${target.index ?? ''}" ${actionDisabled(state)}>Actions</button></header>${edgeDetails(state)}<section class="workflow-fields"><p>${state.session ? 'Edit this local module. Pending fields stay local until Apply.' : 'Saved revisions are read-only. Open the current head and start an append draft to edit fields.'}</p><label>Title<input data-workflow-field="title" value="${text(patch.title ?? (node?.title ?? state.definition?.title ?? ''))}" ${fieldsLocked ? 'disabled' : ''}></label>${target.kind === 'node' ? `<details class="workflow-detail-section" data-workflow-details="configuration" open>${disclosure('Module configuration')} ${nodeFieldMarkup(state, node, patch)}</details>` : ''}${recovery}<div class="workflow-field-actions"><button type="button" class="analysis-button analysis-button--small" data-workflow-apply ${fieldsLocked ? 'disabled' : ''}>Apply</button><button type="button" class="analysis-button analysis-button--small" data-workflow-cancel ${fieldsLocked ? 'disabled' : ''}>Cancel</button></div></section>${target.kind === 'node' ? nodeConnections(state, node, target.index) : ''}${target.kind === 'node' && nodeKind(node) === 'capability' ? `<section class="workflow-capability-picker"><h3>Exact capability pin</h3><p>Choose a saved local capability, then choose one of its exact recorded revisions. IDs are not typed here.</p><select data-workflow-capability ${fieldsLocked ? 'disabled' : ''}><option value="">Choose capability</option>${capabilities.map(item => `<option value="${text(item.id)}" ${node?.operation?.capabilityId === item.id ? 'selected' : ''}>${text(item.title ?? item.label ?? short(item.id))}</option>`).join('')}</select>${detail ? `<div class="workflow-capability-history"><strong>${text(detail.title ?? 'Selected capability')}</strong>${asArray(detail.history).map(revision => `<button type="button" data-workflow-pin="${target.index}|${text(detail.id)}|${text(revision.id)}" ${fieldsLocked ? 'disabled' : ''}>${text(revision.title ?? short(revision.id))}<small>${text(short(revision.id))}</small></button>`).join('')}</div>` : '<p class="workflow-empty">Choose a capability to load its actual saved revision history.</p>'}</section>` : ''}<section class="workflow-native-record"><h3>Native report</h3>${report ? `<p>${report.structurallyValid ? 'Structural validity reported by the native inspector.' : 'Draft remains inspectable; native structural report is not valid yet.'}</p><ul>${asArray(report.diagnostics).map(item => `<li>${text(item.code ?? 'structural issue')} · node ${text(item.nodeIndex ?? '—')}</li>`).join('') || '<li>No retained diagnostics.</li>'}</ul>` : '<p>Inspect the authored draft to receive native structural details.</p>'}</section><section class="workflow-save"><h3>Candidate and local save</h3>${candidate ? `<p>${candidateIntent(state) === 'validated' ? 'Validated candidate is retained.' : 'Draft candidate is retained.'} Acknowledgment applies only to this exact candidate.</p><label><input type="checkbox" data-workflow-ack ${state.acknowledged ? 'checked' : ''} ${disabled(state) ? 'disabled' : ''}> I inspected this exact local candidate. This does not approve execution.</label><button type="button" class="analysis-button analysis-button--primary" data-workflow-commit ${!state.acknowledged || actionDisabled(state) ? 'disabled' : ''}>Save ${candidateIntent(state) === 'validated' ? 'validated revision' : 'draft'}</button>${canValidated && candidateIntent(state) === 'draft' ? `<button type="button" class="analysis-button" data-workflow-inspect="validated" ${actionDisabled(state)}>Inspect validated revision</button>` : ''}` : `<div class="workflow-inspect-actions"><button type="button" class="analysis-button" data-workflow-inspect="draft" ${actionDisabled(state) || !state.session ? 'disabled' : ''}>Inspect draft</button><button type="button" class="analysis-button" data-workflow-inspect="validated" disabled>Inspect validated revision</button></div><p>Validated inspection requires a fresh native draft report with resolved exact references.</p>`}${state.receipt ? `<details>${disclosure('Saved local receipt')}<pre>${json(state.receipt)}</pre></details>` : ''}</section></aside>`;
}

function playgroundInspector(state) {
  const target = fieldTarget(state); const node = target.kind === 'node' ? nodeList(state)[target.index] : null; const patch = fieldPatch(state); const fieldsLocked = !state.session;
  const recovery = state.fieldEdit ? '<p class="workflow-field-recovery" role="status">Pending field edits stay in browser memory. Apply or Cancel before choosing another module.</p>' : '';
  return `<aside class="workflow-inspector" aria-label="Synthetic workflow inspector"><header class="workflow-panel-head"><div><p class="analysis-kicker">${target.kind === 'node' ? `SELECTED MODULE · ${text(nodeKind(node))}` : 'SYNTHETIC'}</p><h2>${target.kind === 'node' ? text(nodeTitle(node, target.index)) : 'Canvas details'}</h2></div><button type="button" class="analysis-button analysis-button--small" data-workflow-menu="${target.kind}:${target.index ?? ''}" ${actionDisabled(state)}>Actions</button></header>${edgeDetails(state)}<section class="workflow-fields"><p>Edits stay in browser memory. There is no capability library, native record, inspection, candidate, acknowledgment, or save action.</p><label>Title<input data-workflow-field="title" value="${text(patch.title ?? (node?.title ?? state.definition?.title ?? ''))}" ${fieldsLocked ? 'disabled' : ''}></label>${target.kind === 'node' ? `<details class="workflow-detail-section" data-workflow-details="configuration" open>${disclosure('Module configuration')}${nodeFieldMarkup(state, node, patch)}</details>` : ''}${recovery}<div class="workflow-field-actions"><button type="button" class="analysis-button analysis-button--small" data-workflow-apply ${fieldsLocked ? 'disabled' : ''}>Apply</button><button type="button" class="analysis-button analysis-button--small" data-workflow-cancel ${fieldsLocked ? 'disabled' : ''}>Cancel</button></div></section>${target.kind === 'node' ? nodeConnections(state, node, target.index) : ''}<section class="workflow-native-record"><h3>Ephemeral status</h3><p>Synthetic, in memory, not saved, and not inspected. No native or provider action can run here.</p></section></aside>`;
}

function historyPanel(state) {
  const opened = state.opened;
  if (!opened?.head) return '';
  const current = opened.revision?.id;
  const revisions = asArray(opened.history);
  return `<section class="workflow-history"><h3>Saved revision history</h3><p>Historical records are read-only. Append starts only from the selected current head.</p><label>Compare left<select data-workflow-compare-left>${revisions.map(item => `<option value="${text(item.id)}">${text(short(item.id))}</option>`).join('')}</select></label><label>Compare right<select data-workflow-compare-right>${revisions.map(item => `<option value="${text(item.id)}" ${item.id === current ? 'selected' : ''}>${text(short(item.id))}</option>`).join('')}</select></label><button type="button" class="analysis-button analysis-button--small" data-workflow-compare ${actionDisabled(state) || revisions.length < 2 ? 'disabled' : ''}>Compare exact revisions</button><div class="workflow-history-list">${revisions.map(item => `<button type="button" data-workflow-open-revision="${text(item.id)}" ${actionDisabled(state)}>${text(short(item.id))}<small>${text(item.intent)}${item.id === current ? ' · selected' : ' · historical'}</small></button>`).join('')}</div><button type="button" class="analysis-button" data-workflow-open-current ${actionDisabled(state)}>Open current head</button><button type="button" class="analysis-button" data-workflow-append ${actionDisabled(state) || current !== opened.head.latestRevisionId ? 'disabled' : ''}>Append from current head</button></section>`;
}

function menu(state, menu) {
  if (!menu) return '';
  const [kind, rawIndex] = String(menu.target ?? '').split(':'); const index = Number(rawIndex);
  const hasNode = kind === 'node' && Number.isSafeInteger(index);
  const hasEdge = kind === 'control' || kind === 'data';
  return `<div class="workflow-context" data-workflow-context role="menu" aria-label="Workflow actions"><button type="button" role="menuitem" data-workflow-menu-action="select:${text(menu.target)}">Select</button>${hasNode ? `<button type="button" role="menuitem" data-workflow-menu-action="edit:${index}" ${structureDisabled(state)}>Edit fields</button><button type="button" role="menuitem" data-workflow-menu-action="remove-node:${index}" ${structureDisabled(state)}>Remove node</button>` : ''}${hasEdge ? `<button type="button" role="menuitem" data-workflow-menu-action="remove-edge:${kind}:${index}" ${structureDisabled(state)}>Remove connection</button>` : ''}<button type="button" role="menuitem" data-workflow-menu-action="close">Close menu</button></div>`;
}

export function renderWorkflowView(state = {}, ui = {}) {
  const availableTabs = visibleTabs(state);
  const activeTab = availableTabs.includes(state.tab) ? state.tab : 'canvas';
  const unavailable = !state.bridgeAvailable;
  const panel = activeTab === 'canvas' ? canvas(state, ui) : activeTab === 'outline' ? outline(state) : activeTab === 'definition' ? definitionView(state) : diffView(state);
  const synthetic = playground(state);
  const status = state.fieldEdit ? 'Pending field edits. Apply or Cancel before choosing another module.' : synthetic ? text(state.message ?? 'Synthetic canvas is in memory only.') : unavailable ? 'Native workflow authoring is unavailable in this preview.' : text(state.message ?? 'Open a saved workflow or begin a new local draft.');
  const unavailableLink = unavailable && !synthetic ? '<a class="analysis-button" href="workflow-playground.html">Open synthetic playground</a>' : '';
  const actions = synthetic
    ? `<button id="workflow-new" type="button" class="analysis-button analysis-button--primary" data-workflow-new ${state.session ? 'disabled' : ''}>Start synthetic canvas</button><button type="button" class="analysis-button" data-workflow-discard ${!state.session ? 'disabled' : ''}>Discard canvas</button>`
    : `<button id="workflow-new" type="button" class="analysis-button analysis-button--primary" data-workflow-new ${actionDisabled(state)}>New workflow</button><button id="workflow-refresh" type="button" class="analysis-button" data-workflow-refresh ${actionDisabled(state)}>Refresh library</button>${unavailableLink}`;
  const layout = ui.workspaceLayout ?? { desktop: false, left: 240, right: 320, leftCollapsed: false, rightCollapsed: false };
  const dock = (side, label, body) => `<div class="workspace-dock workspace-dock--${side}${layout[`${side}Collapsed`] ? ' workspace-dock--collapsed' : ''}" data-workspace-dock="${side}"><button type="button" class="workspace-expand" data-workspace-expand="${side}" ${layout[`${side}Collapsed`] ? '' : 'hidden'}>Expand ${label}</button><button type="button" class="workspace-collapse" data-workspace-collapse="${side}" ${layout.desktop && !layout[`${side}Collapsed`] ? '' : 'hidden'}>Collapse ${label}</button><button type="button" class="workspace-resize" data-workspace-resize="${side}" role="separator" tabindex="0" aria-label="Resize ${label}" aria-orientation="vertical" aria-valuemin="${side === 'left' ? 208 : 280}" aria-valuemax="${layout[`${side}Max`] ?? (side === 'left' ? 360 : 440)}" aria-valuenow="${layout[side]}" aria-valuetext="${layout[side]} pixels" ${layout.desktop && !layout[`${side}Collapsed`] ? '' : 'hidden'}></button>${body.replace('<aside ', `<aside ${layout[`${side}Collapsed`] ? 'hidden ' : ''}`)}</div>`;
  const left = dock('left', 'library', library(state, ui));
  const right = dock('right', 'inspector', inspector(state).replace('</aside>', `${synthetic ? '' : historyPanel(state)}</aside>`));
  return `<section class="workflow-page${synthetic ? ' workflow-page--playground' : ''}" aria-labelledby="workflow-title" aria-busy="${Boolean(state.pending)}"><header class="composition-hero workflow-hero"><p class="analysis-kicker">${icon('workflow', { size: 16 })} ${synthetic ? 'SYNTHETIC WORKFLOW PLAYGROUND' : 'LOCAL WORKFLOW AUTHORING'}</p><h1 id="workflow-title" tabindex="-1">Workflows <em>${synthetic ? 'in memory.' : 'with evidence.'}</em></h1><p>${synthetic ? 'Build an in-memory graph. It is synthetic, not saved, not inspected, and has no native or provider action.' : 'Build reviewable local definitions. This editor does not run, deploy, approve, connect a provider, or activate an engine.'}</p><div class="analysis-actions">${actions}</div><p class="workflow-status${state.status === 'unavailable' && !synthetic ? ' workflow-status--error' : ''}" role="status" aria-live="polite">${status}</p></header>${workflowSummary(state)}<nav class="workflow-tabs" role="tablist" aria-label="Workflow editor views">${availableTabs.map(tab => `<button type="button" id="workflow-tab-${tab}" role="tab" aria-controls="workflow-panel-${tab}" aria-selected="${activeTab === tab}" tabindex="${activeTab === tab ? '0' : '-1'}" data-workflow-tab="${tab}">${tab[0].toUpperCase() + tab.slice(1)}</button>`).join('')}<span class="workflow-tabs-actions"><button type="button" class="analysis-button analysis-button--small" data-workflow-undo ${actionDisabled(state) || !state.canUndo ? 'disabled' : ''}>Undo</button><button type="button" class="analysis-button analysis-button--small" data-workflow-redo ${actionDisabled(state) || !state.canRedo ? 'disabled' : ''}>Redo</button><button type="button" class="analysis-button analysis-button--small" data-workspace-reset>Reset workspace</button></span></nav><div class="workflow-workbench" data-workspace-workbench data-workspace-desktop="${layout.desktop}" style="--workspace-palette-width:${layout.left}px;--workspace-inspector-width:${layout.right}px;--workspace-palette-track:${layout.leftCollapsed ? 44 : layout.left}px;--workspace-inspector-track:${layout.rightCollapsed ? 44 : layout.right}px">${left}<section class="workflow-main" id="workflow-panel-${activeTab}" role="tabpanel" aria-labelledby="workflow-tab-${activeTab}">${panel}</section>${right}</div>${menu(state, ui.menu)}</section>`;
}

export function bindWorkflowView(root, controller, { initialMode = 'flat', initialLibraryCollapsed = false, sceneFactory = createWorkflowScene } = {}) {
  const retained = bindingState.get(controller) ?? { menu: null, focus: null, scroll: new Map(), connect: null };
  let { menu, focus, scroll, connect } = retained;
  let libraryFilter = '';
  let connectOrigin = null;
  let fitNotice = '';
  let initialLayoutPending = initialLibraryCollapsed;
  let disposed = false;
  let skipFocusRestore = false;
  let sceneMode = initialMode === 'depth' ? 'depth' : 'flat';
  let sceneUnavailable = '', previewNotice = '';
  let graphScene = null, sceneCanvas = null, sceneFingerprint = null, sceneTopologyFingerprint = null, scenePoseFingerprint = null;
  let sceneFrameState = null, sceneFocusSelection = null, sceneTopologyNotice = '', sceneFrame = null, animation = null, previousFrame = null;
  const detailsOpen = new Map();
  const liveFeed = createWorkflowLiveFeed();
  const viewWindow = root.ownerDocument.defaultView;
  const motionMedia = viewWindow?.matchMedia?.('(prefers-reduced-motion: reduce)');
  const flatMedia = viewWindow?.matchMedia?.('(max-width: 1000px), (forced-colors: active)');
  const sceneClosed = () => Boolean(flatMedia?.matches);
  const cancelPreview = () => { animation = null; previousFrame = null; if (sceneFrame !== null) viewWindow?.cancelAnimationFrame?.(sceneFrame); sceneFrame = null; if (!disposed) root.querySelector('[data-workflow-design-marker]')?.setAttribute('visibility', 'hidden'); };
  const selectedSceneEdge = state => state.selection && ['control', 'data'].includes(state.selection.kind) ? `${state.selection.kind}:${state.selection.index}` : null;
  const drawScene = () => {
    if (disposed) return;
    const state = controller.getState();
    if (sceneMode === 'depth' && !sceneClosed() && !sceneUnavailable && state.tab === 'canvas') graphScene?.draw({ camera: defaultCamera, frame: sceneFrameState, theme: root.ownerDocument.documentElement?.dataset.theme ?? 'dark', activeNodeId: state.selection?.kind === 'node' ? state.definition?.nodes?.[state.selection.index]?.id : null, selectedEdgeId: selectedSceneEdge(state), motion: animation ? { edgeId: animation.edgeId } : null, progress: animation ? animation.elapsed / 800 : 0 });
    const marker = root.querySelector('[data-workflow-design-marker]');
    if (marker) {
      const wire = animation ? Array.from(root.querySelectorAll('[data-workflow-svg-wire]')).find(el => el.dataset.workflowSvgWire === animation.edgeId) : null;
      let point = null;
      const path = wire?.getAttribute?.('d');
      if (typeof path === 'string' && path.trim() && typeof wire?.getTotalLength === 'function' && typeof wire?.getPointAtLength === 'function') {
        try {
          const length = wire.getTotalLength();
          if (Number.isFinite(length) && length >= 0) {
            const candidate = wire.getPointAtLength(length * animation.elapsed / 800);
            if ([candidate?.x, candidate?.y].every(Number.isFinite)) point = candidate;
          }
        } catch (error) {
          if (error?.name !== 'InvalidStateError') throw error;
        }
      }
      marker.setAttribute('visibility', point ? 'visible' : 'hidden');
      if (point) { marker.setAttribute('cx', String(point.x)); marker.setAttribute('cy', String(point.y)); }
    }
  };
  const scheduleScene = () => {
    if (disposed || sceneFrame !== null || root.ownerDocument.hidden || !animation || !viewWindow?.requestAnimationFrame) return;
    sceneFrame = viewWindow.requestAnimationFrame(timestamp => { sceneFrame = null; if (disposed) return; const delta = previousFrame === null ? 0 : Math.max(0, Math.min(250, timestamp - previousFrame)); previousFrame = timestamp; animation.elapsed = Math.min(800, animation.elapsed + delta); if (animation.elapsed >= 800) { animation = null; previousFrame = null; } drawScene(); scheduleScene(); });
  };
  const focusSceneSelection = () => {
    if (disposed) return;
    if (sceneMode !== 'depth' || sceneClosed() || sceneUnavailable) return;
    const state = controller.getState();
    const next = !disabled(state) && sceneSelectionValid(state) ? frameForSelection(projectWorkflowScene(state), state.selection) : null;
    if (!next) return;
    cancelPreview();
    sceneFrameState = next;
    sceneFocusSelection = { ...state.selection };
    previewNotice = '';
    sceneTopologyNotice = 'Focused selected section.';
    render();
  };
  const overviewScene = () => {
    if (disposed) return;
    cancelPreview();
    sceneFrameState = null;
    sceneFocusSelection = null;
    previewNotice = '';
    sceneTopologyNotice = 'Overview frames the admitted graph.';
    render();
  };
  const fieldSelector = '[data-workflow-field]';
  const targetKey = state => {
    const target = fieldTarget(state);
    return target.kind === 'node' ? `node:${target.index}` : 'workflow';
  };
  const saveDomState = () => {
    const active = root.ownerDocument?.activeElement;
    if (active?.matches?.('[data-workflow-library-filter]')) focus = { kind: 'library-filter', start: active.selectionStart, end: active.selectionEnd };
    else if (active?.matches?.(fieldSelector)) focus = { key: active.dataset.workflowField, target: targetKey(controller.getState()), start: active.selectionStart, end: active.selectionEnd };
    else {
      focus = null;
      for (const attribute of ['data-workflow-mode', 'data-workflow-scene-overview', 'data-workflow-scene-focus', 'data-workflow-select-node', 'data-workflow-select-edge', 'data-workflow-menu', 'data-workflow-data-source', 'data-workflow-data-target', 'data-workflow-control-source', 'data-workflow-preview', 'data-workflow-scene-retry', 'data-workflow-apply', 'data-workflow-cancel', 'data-workflow-undo', 'data-workflow-redo']) {
        if (active?.hasAttribute?.(attribute)) { focus = { kind: 'action', attribute, value: active.getAttribute(attribute) }; break; }
      }
    }
    root.querySelectorAll('[data-workflow-scroll]').forEach(element => scroll.set(element.dataset.workflowScroll, { top: element.scrollTop, left: element.scrollLeft }));
  };
  const restoreDomState = () => {
    root.querySelectorAll('[data-workflow-scroll]').forEach(element => { const saved = scroll.get(element.dataset.workflowScroll); if (saved) { element.scrollTop = saved.top; element.scrollLeft = saved.left; } });
    if (focus?.kind === 'library-filter') { const element = root.querySelector('[data-workflow-library-filter]'); element?.focus({ preventScroll: true }); if (element && Number.isInteger(focus.start) && Number.isInteger(focus.end) && 'setSelectionRange' in element) element.setSelectionRange(focus.start, focus.end); }
    else if (focus?.kind === 'action') { const element = Array.from(root.querySelectorAll(`[${focus.attribute}]`)).find(item => item.getAttribute(focus.attribute) === focus.value); if (element && !element.disabled) element.focus({ preventScroll: true }); else root.querySelector('[data-workflow-mode]')?.focus({ preventScroll: true }); }
    else if (focus?.target === targetKey(controller.getState())) { const element = root.querySelector(`[data-workflow-field="${CSS.escape(focus.key)}"]`); if (workspace.getState().rightCollapsed) root.querySelector('[data-workspace-expand="right"]')?.focus({ preventScroll: true }); else { element?.focus({ preventScroll: true }); if (element && Number.isInteger(focus.start) && Number.isInteger(focus.end) && 'setSelectionRange' in element) element.setSelectionRange(focus.start, focus.end); } }
    else focus = null;
  };
  const layoutWires = () => {
    const state = controller.getState();
    const stage = root.querySelector('[data-workflow-graph-stage]');
    const svg = stage?.querySelector('.workflow-wire-diagram');
    if (!stage || !svg) return;
    const zoom = Number(state.viewport?.zoom ?? 1);
    if (!Number.isFinite(zoom) || zoom < .25 || zoom > 4) return;
    const stageRect = stage.getBoundingClientRect();
    const point = (element, side) => {
      const rect = element?.getBoundingClientRect();
      if (!rect) return null;
      return {
        x: (rect[side] - stageRect.left) / zoom,
        y: (rect.top + rect.height / 2 - stageRect.top) / zoom,
      };
    };
    const path = (kind, index, from, to) => {
      if (!from || !to) return;
      const element = svg.querySelector(`[data-workflow-svg-wire="${kind}:${index}"]`);
      if (!element) return;
      const middle = from.x + (to.x - from.x) / 2;
      element.setAttribute('d', `M ${from.x} ${from.y} C ${middle} ${from.y}, ${middle} ${to.y}, ${to.x} ${to.y}`);
      const label = svg.querySelector(`[data-workflow-svg-label="${kind}:${index}"]`);
      label?.setAttribute('x', String(middle));
      label?.setAttribute('y', String((from.y + to.y) / 2 - 5));
    };
    const byId = new Map(nodeList(state).map((node, index) => [node.id, index]));
    controlEdges(state).forEach((edge, index) => {
      const fromIndex = byId.get(edge.fromNode); const toIndex = byId.get(edge.toNode);
      path('control', index,
        point(root.querySelector(`[data-workflow-control-source="${fromIndex}:${CSS.escape(edge.outlet)}"]`), 'right'),
        point(root.querySelector(`[data-workflow-node="${toIndex}"] .workflow-node-select`), 'left'));
    });
    dataEdges(state).forEach((edge, index) => {
      const fromIndex = byId.get(edge.fromNode); const toIndex = byId.get(edge.toNode);
      path('data', index,
        point(root.querySelector(`[data-workflow-data-source="${fromIndex}:${CSS.escape(edge.fromPort)}"]`), 'right'),
        point(root.querySelector(`[data-workflow-data-target="${toIndex}:${CSS.escape(edge.toPort)}"]`), 'left'));
    });
    const width = stage.offsetWidth;
    const height = stage.offsetHeight;
    svg.setAttribute('width', String(width));
    svg.setAttribute('height', String(height));
    svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  };
  const drag = createWorkflowDrag({
    getState: () => controller.getState(),
    requestFrame: callback => root.ownerDocument.defaultView?.requestAnimationFrame(timestamp => { if (!disposed) callback(timestamp); }) ?? setTimeout(() => { if (!disposed) callback(); }, 16),
    cancelFrame: frame => {
      if (typeof frame === 'number') root.ownerDocument.defaultView?.cancelAnimationFrame(frame);
      else clearTimeout(frame);
    },
    redraw: () => { if (!disposed) layoutWires(); },
    commit: (index, x, y) => disposed ? false : controller.moveNode(index, x, y),
  });
  const workspace = createWorkspaceLayout({
    requestFrame: callback => root.ownerDocument.defaultView?.requestAnimationFrame(timestamp => { if (!disposed) callback(timestamp); }) ?? setTimeout(() => { if (!disposed) callback(); }, 16),
    cancelFrame: frame => root.ownerDocument.defaultView?.cancelAnimationFrame?.(frame) ?? clearTimeout(frame),
    onChange: () => { if (!disposed) { applyWorkspaceLayout(); layoutWires(); drawScene(); } },
  });
  const workspaceBounds = () => ({ viewportWidth: root.ownerDocument.defaultView?.innerWidth ?? 0, workbenchWidth: root.querySelector('[data-workspace-workbench]')?.clientWidth ?? 0 });
  const applyWorkspaceLayout = () => {
    const bench = root.querySelector('[data-workspace-workbench]'); const layout = workspace.getState();
    if (!bench?.style) return;
    bench.dataset.workspaceDesktop = String(layout.desktop);
    bench.style.setProperty('--workspace-palette-width', `${layout.left}px`); bench.style.setProperty('--workspace-inspector-width', `${layout.right}px`);
    bench.style.setProperty('--workspace-palette-track', `${layout.leftCollapsed ? 44 : layout.left}px`); bench.style.setProperty('--workspace-inspector-track', `${layout.rightCollapsed ? 44 : layout.right}px`);
    root.querySelectorAll('[data-workspace-resize]').forEach(node => { const side = node.dataset.workspaceResize; node.setAttribute('aria-valuenow', String(layout[side])); node.setAttribute('aria-valuemax', String(layout[`${side}Max`])); node.setAttribute('aria-valuetext', `${layout[side]} pixels`); });
  };
  const syncWorkspaceBounds = () => { if (disposed) return; const before = workspace.getState(); workspace.setBounds(workspaceBounds()); if (initialLayoutPending && workspace.getState().desktop) { initialLayoutPending = false; workspace.toggle('left'); render(); } else if (before.desktop !== workspace.getState().desktop) render(); };
  const render = () => {
    if (disposed) return;
    workspace.cancel();
    if (drag.isActive()) drag.cancel();
    if (skipFocusRestore) { focus = null; skipFocusRestore = false; }
    else saveDomState();
    const state = controller.getState(), projected = projectWorkflowScene(state);
    if (sceneFrameState !== null && sceneTopologyFingerprint !== projected.topologyFingerprint) {
      sceneFrameState = null;
      sceneFocusSelection = null;
      sceneTopologyNotice = 'Graph changed. Overview restored.';
    } else if (sceneFrameState !== null && scenePoseFingerprint !== projected.fingerprint) {
      sceneFrameState = frameForSelection(projected, sceneFocusSelection);
      if (!sceneFrameState) { sceneFocusSelection = null; sceneTopologyNotice = 'Section unavailable. Overview restored.'; }
    }
    sceneTopologyFingerprint = projected.topologyFingerprint;
    scenePoseFingerprint = projected.fingerprint;
    const live = liveFeed.observe(state);
    if (animation && (animation.fingerprint !== projected.fingerprint || animation.edgeId !== selectedSceneEdge(state) || state.tab !== 'canvas')) cancelPreview();
    root.querySelectorAll('[data-workflow-details]').forEach(element => detailsOpen.set(element.dataset.workflowDetails, element.open));
    if (sceneCanvas) sceneCanvas.remove?.();
    root.innerHTML = renderWorkflowView(state, { menu, connect, libraryFilter, workspaceLayout: workspace.getState(), fitNotice, live, sceneMode, sceneNarrow: sceneClosed(), motionReduced: Boolean(motionMedia?.matches), previewNotice, sceneNotice: sceneTopologyNotice, sceneUnavailable: sceneUnavailable || (sceneClosed() ? '3D unavailable in narrow or forced-color view.' : '') });
    root.querySelectorAll('[data-workflow-details]').forEach(element => { if (detailsOpen.has(element.dataset.workflowDetails)) element.open = detailsOpen.get(element.dataset.workflowDetails); });
    const nextCanvas = root.querySelector('[data-workflow-scene-canvas]');
    if (nextCanvas && projected.available) {
      if (sceneCanvas) nextCanvas.replaceWith(sceneCanvas); else sceneCanvas = nextCanvas;
      if (!graphScene) graphScene = sceneFactory(sceneCanvas, projected, { onUnavailable: reason => { if (disposed) return; sceneUnavailable = reason; cancelPreview(); viewWindow?.queueMicrotask?.(() => { if (!disposed) render(); }); }, onProject: points => { if (disposed) return; for (const point of points) { const node = Array.from(root.querySelectorAll('[data-workflow-scene-id]')).find(el => el.dataset.workflowSceneId === point.id); if (node) { node.style.left = `${point.x}%`; node.style.top = `${point.y}%`; node.style.visibility = point.visible ? 'visible' : 'hidden'; } } } });
      else if (sceneFingerprint !== projected.fingerprint) graphScene.setTopology(projected);
      sceneFingerprint = projected.fingerprint;
      drawScene();
    }
    graphScene?.invalidateProjection?.();
    restoreDomState();
    layoutWires();
    drawScene();
    syncWorkspaceBounds();
    if (menu) root.querySelector('[data-workflow-context] button')?.focus({ preventScroll: true });
  };
  const invoke = action => disposed ? undefined : Promise.resolve(action()).finally(() => { if (!disposed) render(); });
  const updateFields = () => {
    const state = controller.getState(); const target = fieldTarget(state); const patch = {};
    const values = Object.fromEntries([...root.querySelectorAll(fieldSelector)].map(element => [element.dataset.workflowField, element.value]));
    if ('title' in values) patch.title = values.title;
    if (target.kind === 'node') {
      const node = nodeList(state)[target.index];
      const kind = values['operation.kind'] ?? nodeKind(node);
      const operation = { kind };
      if (kind === 'capability') {
        operation.capabilityId = node?.operation?.capabilityId ?? '';
        operation.revisionId = node?.operation?.revisionId ?? '';
      }
      if (kind === 'checkpoint') operation.prompt = values.prompt ?? node?.operation?.prompt ?? '';
      if (kind === 'check') {
        const checkKind = values['check.kind'] ?? node?.operation?.check?.kind ?? 'contains_text';
        operation.check = checkKind === 'contains_text'
          ? { kind: checkKind, needle: values.prompt ?? node?.operation?.check?.needle ?? '' }
          : { kind: 'non_empty' };
      }
      patch.operation = operation;
      for (const [key, source] of [['inputs', node?.inputs], ['outputs', node?.outputs]]) {
        if (!(key in values)) continue;
        patch[key] = values[key].split(',').map(part => {
          const [name, dataType] = part.split(':').map(value => value.trim());
          return name ? { name, dataType: dataType || 'text' } : null;
        }).filter(Boolean);
      }
    }
    return controller.editFields(target, patch);
  };
  let menuReturnTarget = null;
  const closeMenu = ({ restoreFocus = true } = {}) => {
    const invoker = menuReturnTarget;
    menu = null;
    menuReturnTarget = null;
    if (!restoreFocus) skipFocusRestore = true;
    render();
    if (restoreFocus) root.querySelector(`[data-workflow-menu="${CSS.escape(invoker ?? '')}"]`)?.focus({ preventScroll: true });
  };
  const dismissMenu = () => {
    if (!menu) return;
    menu = null;
    menuReturnTarget = null;
    root.querySelector('[data-workflow-context]')?.remove();
  };
  const cancelConnect = () => {
    const source = connectOrigin;
    connect = null;
    connectOrigin = null;
    render();
    if (source?.kind === 'data') root.querySelector(`[data-workflow-data-source="${source.index}:${CSS.escape(source.port)}"]`)?.focus({ preventScroll: true });
    if (source?.kind === 'control') root.querySelector(`[data-workflow-control-source="${source.index}:${CSS.escape(source.port)}"]`)?.focus({ preventScroll: true });
  };
  const beginConnect = (next, origin) => {
    connect = next;
    connectOrigin = origin;
    render();
    root.querySelector('[data-workflow-connect-cancel]')?.focus({ preventScroll: true });
  };
  const selectNode = index => { if (workspace.getState().rightCollapsed) workspace.toggle('right'); return invoke(() => controller.select({ kind: 'node', index })); };
  const selectEdge = (kind, index) => invoke(() => controller.select({ kind, index }));
  const focusFlatSelection = () => {
    const current = selection(controller.getState());
    const selector = current?.kind === 'node'
      ? `[data-workflow-select-node="${current.index}"]`
      : current && ['control', 'data'].includes(current.kind)
        ? `[data-workflow-select-edge="${current.kind}:${current.index}"]`
        : '[data-workflow-mode="depth"]';
    const element = root.querySelector(selector);
    const disclosure = element?.closest?.('details');
    if (disclosure && !disclosure.open) {
      disclosure.open = true;
      if (disclosure.dataset.workflowDetails) detailsOpen.set(disclosure.dataset.workflowDetails, true);
    }
    element?.focus({ preventScroll: true });
  };
  const confirmDiscard = action => {
    const current = controller.getState();
    if (!current.dirty && !current.fieldEdit) return invoke(action);
    if (!root.ownerDocument.defaultView?.confirm('Keep editing or discard local workflow changes? Choose OK to discard this matching local draft.')) return;
    return Promise.resolve(controller.clear(true)).then(result => !disposed && result ? action() : false).finally(() => { if (!disposed) render(); });
  };
  const click = event => {
    if (disposed) return;
    const target = event.target.closest('button,[data-workflow-node]');
    if (!target || target.disabled) return;
    if (target.dataset.workflowMode) { cancelPreview(); sceneMode = target.dataset.workflowMode === 'depth' ? 'depth' : 'flat'; render(); if (sceneMode === 'flat') focusFlatSelection(); else root.querySelector('[data-workflow-mode="flat"]')?.focus({ preventScroll: true }); return; }
    if (target.dataset.workflowSceneRetry !== undefined) { cancelPreview(); graphScene?.dispose(); graphScene = null; sceneCanvas = null; sceneFingerprint = null; sceneUnavailable = ''; sceneMode = 'depth'; return render(); }
    if (target.dataset.workflowSceneOverview !== undefined) return overviewScene();
    if (target.dataset.workflowSceneFocus !== undefined) return focusSceneSelection();
    if (target.dataset.workflowPreview !== undefined) { cancelPreview(); const state = controller.getState(), projected = projectWorkflowScene(state), edgeId = selectedSceneEdge(state); if (!edgeId || root.ownerDocument.hidden || sceneMode === 'depth' && sceneUnavailable) return; sceneTopologyNotice = ''; if (motionMedia?.matches) { previewNotice = 'Reduced motion: selected connection highlighted. Design only · no execution'; render(); return; } previewNotice = 'One connection · one directional preview · no execution'; animation = { edgeId, elapsed: 0, fingerprint: projected.fingerprint }; render(); scheduleScene(); return; }
    if (target.dataset.workflowTab) { cancelPreview(); return invoke(() => controller.setTab(target.dataset.workflowTab)); }
    if (target.dataset.workflowNew !== undefined) return confirmDiscard(() => controller.beginNew());
    if (target.dataset.workflowDiscard !== undefined) return invoke(() => controller.clear(true));
    if (target.dataset.workflowRefresh !== undefined) return invoke(() => controller.load());
    if (target.dataset.workflowOpen) return confirmDiscard(() => controller.open(target.dataset.workflowOpen));
    if (target.dataset.workflowOpenCurrent !== undefined) return confirmDiscard(() => controller.open(controller.getState().opened?.head?.id));
    if (target.dataset.workflowOpenRevision) return confirmDiscard(() => controller.open(controller.getState().opened?.head?.id, target.dataset.workflowOpenRevision));
    if (target.dataset.workflowAppend !== undefined) return confirmDiscard(() => controller.beginAppend());
    if (target.dataset.workflowCompare !== undefined) {
      const left = root.querySelector('[data-workflow-compare-left]')?.value;
      const right = root.querySelector('[data-workflow-compare-right]')?.value;
      return invoke(() => controller.compare(left, right));
    }
    if (target.dataset.workflowZoom) {
      const viewport = controller.getState().viewport ?? { x: 0, y: 0, zoom: 1 };
      return invoke(() => controller.setViewport({ zoom: Math.min(4, Math.max(.25, Number(viewport.zoom ?? 1) + Number(target.dataset.workflowZoom))) }));
    }
    if (target.dataset.workflowPan) {
      const viewport = controller.getState().viewport ?? { x: 0, y: 0 };
      const delta = target.dataset.workflowPan === 'left' ? -40 : 40;
      return invoke(() => controller.setViewport({ x: Number(viewport.x ?? 0) + delta }));
    }
    if (target.dataset.workflowViewportReset !== undefined) return invoke(() => controller.setViewport({ x: 0, y: 0, zoom: 1 }));
    if (target.dataset.workflowFit !== undefined) {
      const canvas = root.querySelector('[data-workflow-canvas]');
      const computed = canvas ? root.ownerDocument.defaultView?.getComputedStyle?.(canvas) : null;
      const padding = name => Number.parseFloat(computed?.[name] ?? '0') || 0;
      const fitted = fitWorkflowGraph({ bounds: graphBounds(controller.getState()), canvas: { width: Number(canvas?.clientWidth) - padding('paddingLeft') - padding('paddingRight'), height: Number(canvas?.clientHeight) - padding('paddingTop') - padding('paddingBottom') } });
      if (!fitted) { fitNotice = 'Fit graph is unavailable until the canvas has usable geometry.'; return render(); }
      fitNotice = fitted.minimumOverflow ? 'Graph exceeds minimum Fit zoom. Pan, zoom, or use Outline; canvas scrolling remains available.' : '';
      return invoke(() => controller.setViewport({ x: fitted.x, y: fitted.y, zoom: fitted.zoom }));
    }
    if (target.dataset.workspaceReset !== undefined) { workspace.reset(); render(); root.querySelector('[data-workspace-reset]')?.focus({ preventScroll: true }); return; }
    if (target.dataset.workspaceCollapse) { workspace.toggle(target.dataset.workspaceCollapse); render(); return root.querySelector(`[data-workspace-expand="${target.dataset.workspaceCollapse}"]`)?.focus({ preventScroll: true }); }
    if (target.dataset.workspaceExpand) { workspace.toggle(target.dataset.workspaceExpand); render(); return root.querySelector(`[data-workspace-collapse="${target.dataset.workspaceExpand}"]`)?.focus({ preventScroll: true }); }
    if (target.dataset.workflowAdd) {
      const count = nodeList(controller.getState()).length;
      return invoke(() => controller.addNode(target.dataset.workflowAdd, { x: 40 + (count % 3) * 245, y: 40 + Math.floor(count / 3) * 190 }));
    }
    if (target.dataset.workflowControlSource) {
      const [fromIndex, outlet] = target.dataset.workflowControlSource.split(':');
      return beginConnect({ kind: 'control', fromIndex: Number(fromIndex), outlet }, { kind: 'control', index: Number(fromIndex), port: outlet });
    }
    if (target.dataset.workflowDataSource) {
      const [fromIndex, fromPort] = target.dataset.workflowDataSource.split(':');
      return beginConnect({ kind: 'data', fromIndex: Number(fromIndex), fromPort }, { kind: 'data', index: Number(fromIndex), port: fromPort });
    }
    if (target.dataset.workflowConnectCancel !== undefined) return cancelConnect();
    if (target.dataset.workflowDataTarget && connect?.kind === 'data') {
      const [toIndex, toPort] = target.dataset.workflowDataTarget.split(':');
      const next = connect; connect = null; connectOrigin = null;
      return invoke(() => controller.connectData(next.fromIndex, next.fromPort, Number(toIndex), toPort));
    }
    // Pointer capture can retarget a stationary header click to its node container.
    const nodeTarget = target.dataset.workflowSelectNode ?? target.dataset.workflowNode;
    if (nodeTarget !== undefined && connect?.kind === 'control') {
      const next = connect; connect = null; connectOrigin = null;
      return invoke(() => controller.connectControl(next.fromIndex, next.outlet, Number(nodeTarget)));
    }
    if (nodeTarget !== undefined) { if (controller.getState().fieldEdit) return; return selectNode(Number(nodeTarget)); }
    if (target.dataset.workflowSelectEdge) { if (controller.getState().fieldEdit) return; const [kind, index] = target.dataset.workflowSelectEdge.split(':'); return selectEdge(kind, Number(index)); }
    if (target.dataset.workflowReorder) { const [index, delta] = target.dataset.workflowReorder.split(':').map(Number); return invoke(() => controller.reorderNode(index, delta)); }
    if (target.dataset.workflowUndo !== undefined) return invoke(() => controller.undo());
    if (target.dataset.workflowRedo !== undefined) return invoke(() => controller.redo());
    if (target.dataset.workflowApply !== undefined) return invoke(() => { const result = updateFields(); return Promise.resolve(result).then(value => value && controller.applyFields()); });
    if (target.dataset.workflowCancel !== undefined) return invoke(() => controller.cancelFields());
    if (target.dataset.workflowCapability !== undefined) return;
    if (target.dataset.workflowPin) { const [index, capabilityId, revisionId] = target.dataset.workflowPin.split('|'); return invoke(() => controller.pinCapability(Number(index), capabilityId, revisionId)); }
    if (target.dataset.workflowInspect) return invoke(() => controller.inspect(target.dataset.workflowInspect));
    if (target.dataset.workflowCommit !== undefined) return invoke(() => controller.commit());
    if (target.dataset.workflowRemoveEdge) { const [kind, index] = target.dataset.workflowRemoveEdge.split(':'); if (!root.ownerDocument.defaultView?.confirm('Remove this local connection?')) return; return invoke(() => controller.removeEdge(kind, Number(index))); }
    if (target.dataset.workflowMenu) { menuReturnTarget = target.dataset.workflowMenu; menu = { target: target.dataset.workflowMenu }; return render(); }
    if (target.dataset.workflowMenuAction) {
      const [action, kind, rawIndex] = target.dataset.workflowMenuAction.split(':');
      if (action === 'close') return closeMenu();
      if (action === 'select') { menu = null; return kind === 'canvas' ? invoke(() => controller.select(null)) : kind === 'node' ? selectNode(Number(rawIndex)) : selectEdge(kind, Number(rawIndex)); }
      if (action === 'edit') { menu = null; return selectNode(Number(kind)); }
      if (action === 'remove-node') { if (!root.ownerDocument.defaultView?.confirm('Remove this node and its incident local connections?')) return; menu = null; return invoke(() => controller.removeNode(Number(kind), true)); }
      if (action === 'remove-edge') { if (!root.ownerDocument.defaultView?.confirm('Remove this local connection?')) return; menu = null; return invoke(() => controller.removeEdge(kind, Number(rawIndex))); }
    }
  };
  const change = event => {
    if (disposed) return;
    const target = event.target;
    if (target.matches('[data-workflow-capability]')) return invoke(() => controller.chooseCapability(target.value));
    if (target.matches('[data-workflow-ack]')) return invoke(() => controller.acknowledge(target.checked));
  };
  const input = event => {
    if (disposed) return;
    if (event.target.matches('[data-workflow-library-filter]')) { libraryFilter = event.target.value; return render(); }
    if (!event.target.matches(fieldSelector)) return;
    updateFields();
  };
  const submit = event => {
    if (disposed) return;
    const form = event.target; if (!form.matches('[data-workflow-control-form],[data-workflow-data-form]')) return;
    event.preventDefault(); const fields = new FormData(form);
    if (form.matches('[data-workflow-control-form]')) return invoke(() => controller.connectControl(Number(fields.get('from')), String(fields.get('outlet')), Number(fields.get('to'))));
    return invoke(() => controller.connectData(Number(fields.get('from')), String(fields.get('fromPort')), Number(fields.get('to')), String(fields.get('toPort'))));
  };
  const pointerdown = event => {
    if (disposed) return;
    const resize = event.target.closest('[data-workspace-resize]');
    if (resize && workspace.start(resize.dataset.workspaceResize, event, resize)) { event.preventDefault(); return; }
    const handle = event.target.closest('[data-workflow-drag-handle]');
    const node = handle?.closest('[data-workflow-node]');
    if (!node || event.target.closest('[data-workflow-menu],[data-workflow-data-source],[data-workflow-data-target],[data-workflow-control-source],input,textarea,select')) return;
    drag.start(event, node);
  };
  const pointermove = event => { if (!disposed && !workspace.move(event)) drag.move(event); };
  const pointerup = event => { if (!disposed && !workspace.finish(event)) drag.finish(event); };
  const pointercancel = event => { if (!disposed && !workspace.cancel(event.pointerId)) drag.cancel(event.pointerId); };
  const lostpointercapture = pointercancel;
  const dragstart = event => {
    if (disposed) return;
    if (!controller.getState().session) return;
    const kind = event.target.closest('[data-workflow-add-kind]')?.dataset.workflowAddKind;
    if (kind) return event.dataTransfer?.setData('text/x-rangoon-workflow-node', kind);
    const dataSource = event.target.closest('[data-workflow-data-source]')?.dataset.workflowDataSource;
    if (dataSource) return event.dataTransfer?.setData('text/x-rangoon-workflow-connection', `data:${dataSource}`);
    const controlSource = event.target.closest('[data-workflow-control-source]')?.dataset.workflowControlSource;
    if (controlSource) event.dataTransfer?.setData('text/x-rangoon-workflow-connection', `control:${controlSource}`);
  };
  const drop = event => {
    if (disposed) return;
    const transfer = event.dataTransfer;
    const connection = transfer?.getData('text/x-rangoon-workflow-connection');
    const kind = transfer?.getData('text/x-rangoon-workflow-node');
    if (!controller.getState().session) { if (connection || kind) event.preventDefault(); return; }
    const dataTarget = event.target.closest('[data-workflow-data-target]')?.dataset.workflowDataTarget;
    const nodeTarget = event.target.closest('[data-workflow-node]')?.dataset.workflowNode;
    if (connection?.startsWith('data:') && dataTarget) {
      event.preventDefault(); const [fromIndex, fromPort] = connection.slice(5).split(':'); const [toIndex, toPort] = dataTarget.split(':');
      return invoke(() => controller.connectData(Number(fromIndex), fromPort, Number(toIndex), toPort));
    }
    if (connection?.startsWith('control:') && nodeTarget !== undefined) {
      event.preventDefault(); const [fromIndex, outlet] = connection.slice(8).split(':');
      return invoke(() => controller.connectControl(Number(fromIndex), outlet, Number(nodeTarget)));
    }
    if (kind && event.target.closest('[data-workflow-scene]')) { event.preventDefault(); const count = nodeList(controller.getState()).length; return invoke(() => controller.addNode(kind, { x: 40 + (count % 3) * 245, y: 40 + Math.floor(count / 3) * 190 })); }
    if (!kind || !event.target.closest('[data-workflow-canvas]')) return;
    event.preventDefault(); const canvas = event.target.closest('[data-workflow-canvas]'); const rect = canvas.getBoundingClientRect(); const stage = canvas.querySelector('[data-workflow-graph-stage]'); const viewport = controller.getState().viewport ?? { x: 0, y: 0, zoom: 1 }; const zoom = Number(viewport.zoom ?? 1); const offset = name => Number((stage?.style.getPropertyValue(name) ?? '0').replace('px', '')); const offsetX = offset('--graph-offset-x'); const offsetY = offset('--graph-offset-y'); invoke(() => controller.addNode(kind, { x: Math.round((event.clientX - rect.left + canvas.scrollLeft - Number(viewport.x ?? 0)) / zoom - offsetX), y: Math.round((event.clientY - rect.top + canvas.scrollTop - Number(viewport.y ?? 0)) / zoom - offsetY) }));
  };
  const keydown = event => {
    if (disposed) return;
    const active = root.ownerDocument.activeElement;
    if (event.key === 'Escape' && animation) { event.preventDefault(); event.stopPropagation?.(); cancelPreview(); drawScene(); return; }
    if (active?.matches?.('[data-workspace-resize]') && workspace.key(active.dataset.workspaceResize, event)) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') { event.preventDefault(); return invoke(() => event.shiftKey ? controller.redo() : controller.undo()); }
    if (event.key === 'Escape' && active?.matches?.('[data-workflow-library-filter]')) { event.preventDefault(); event.stopPropagation(); if (libraryFilter) { libraryFilter = ''; return render(); } return; }
    if (active?.getAttribute?.('role') === 'tab' && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const availableTabs = visibleTabs(controller.getState());
      const current = availableTabs.indexOf(active.dataset.workflowTab);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? availableTabs.length - 1 : (current + (event.key === 'ArrowRight' ? 1 : -1) + availableTabs.length) % availableTabs.length;
      controller.setTab(availableTabs[next]);
      root.ownerDocument.defaultView?.requestAnimationFrame(() => { if (!disposed) root.querySelector(`[data-workflow-tab="${availableTabs[next]}"]`)?.focus({ preventScroll: true }); });
      return;
    }
    if (event.key === 'Escape' && workspace.cancel()) { event.preventDefault(); event.stopPropagation?.(); return; }
    if (event.key === 'Escape' && drag.isActive()) { event.preventDefault(); return drag.cancel(); }
    if (event.key === 'Escape' && connect) { event.preventDefault(); return cancelConnect(); }
    if (event.key === 'Escape' && menu) { event.preventDefault(); return closeMenu(); }
    if (menu && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const items = [...root.querySelectorAll('[data-workflow-context] [role=menuitem]:not([disabled])')];
      const index = Math.max(0, items.indexOf(active));
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
      return items[next]?.focus();
    }
    if ((event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) && active?.closest?.('[data-workflow-node],[data-workflow-select-edge]')) { event.preventDefault(); const node = active.closest('[data-workflow-node]'); menuReturnTarget = node ? `node:${node.dataset.workflowNode}` : active.dataset.workflowSelectEdge; menu = { target: menuReturnTarget }; return render(); }
    if (controller.getState().session && active?.closest?.('[data-workflow-node]') && ['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) { event.preventDefault(); const nodes = nodeList(controller.getState()); const index = Number(active.closest('[data-workflow-node]').dataset.workflowNode); const delta = event.key === 'ArrowUp' || event.key === 'Home' ? -1 : 1; return invoke(() => controller.reorderNode(index, event.key === 'Home' ? -index : event.key === 'End' ? nodes.length - index - 1 : delta)); }
  };
  const contextmenu = event => { if (disposed || event.target.closest('input,textarea')) return; const target = event.target.closest('[data-workflow-node],[data-workflow-select-edge],[data-workflow-canvas]'); if (!target) return; event.preventDefault(); menuReturnTarget = target.dataset.workflowNode !== undefined ? `node:${target.dataset.workflowNode}` : target.dataset.workflowSelectEdge ?? 'canvas'; menu = { target: menuReturnTarget }; render(); };
  const outside = event => { if (!disposed && menu && !event.target.closest('[data-workflow-context],[data-workflow-menu]')) dismissMenu(); };
  const menuFocusout = event => {
    if (disposed) return;
    if (!menu || !event.target.closest?.('[data-workflow-context]')) return;
    const next = event.relatedTarget;
    if (next?.closest?.('[data-workflow-context]')) return;
    dismissMenu();
  };
  const documentKeydown = event => {
    if (disposed) return;
    if (event.key === 'Escape' && workspace.cancel()) { event.preventDefault(); return; }
    if (event.key === 'Escape' && drag.isActive()) { event.preventDefault(); drag.cancel(); return; }
    if (event.key === 'Escape' && connect) { event.preventDefault(); cancelConnect(); }
  };
  const dragover = event => { if (!disposed && event.target.closest('[data-workflow-canvas],[data-workflow-data-target],[data-workflow-node]')) event.preventDefault(); };
  const workspaceResizeObserver = root.ownerDocument.defaultView?.ResizeObserver ? new root.ownerDocument.defaultView.ResizeObserver(() => { if (!disposed) syncWorkspaceBounds(); }) : null;
  const workspaceWindowResize = () => { if (!disposed) { syncWorkspaceBounds(); drawScene(); } };
  const scenePreference = () => { if (!disposed) { cancelPreview(); render(); } };
  const sceneVisibility = () => { if (!disposed && root.ownerDocument.hidden) cancelPreview(); };
  motionMedia?.addEventListener?.('change', scenePreference); flatMedia?.addEventListener?.('change', scenePreference); root.ownerDocument.addEventListener('visibilitychange', sceneVisibility);
  root.addEventListener('click', click); root.addEventListener('change', change); root.addEventListener('input', input); root.addEventListener('submit', submit); root.addEventListener('pointerdown', pointerdown); root.addEventListener('pointermove', pointermove); root.addEventListener('pointerup', pointerup); root.addEventListener('pointercancel', pointercancel); root.addEventListener('lostpointercapture', lostpointercapture); root.addEventListener('dragstart', dragstart); root.addEventListener('dragover', dragover); root.addEventListener('drop', drop); root.addEventListener('keydown', keydown); root.addEventListener('focusout', menuFocusout); root.addEventListener('contextmenu', contextmenu); root.ownerDocument.addEventListener('pointerdown', outside); root.ownerDocument.addEventListener('keydown', documentKeydown);
  const unsubscribe = typeof controller.subscribe === 'function' ? controller.subscribe(render) : null;
  bindingState.set(controller, retained);
  render();
  workspaceResizeObserver?.observe(root);
  root.ownerDocument.defaultView?.addEventListener?.('resize', workspaceWindowResize);
  const dispose = ({ purge = false } = {}) => {
    if (disposed) return;
    disposed = true; liveFeed.dispose(); cancelPreview(); graphScene?.dispose(); graphScene = null; sceneCanvas = null; sceneFingerprint = null; sceneTopologyFingerprint = null; scenePoseFingerprint = null; sceneFrameState = null; sceneFocusSelection = null; detailsOpen.clear(); scroll.clear(); focus = null; menu = null; connect = null; connectOrigin = null; fitNotice = ''; previewNotice = ''; sceneUnavailable = ''; sceneTopologyNotice = '';
    libraryFilter = ''; menuReturnTarget = null;
    if (purge) bindingState.delete(controller);
    motionMedia?.removeEventListener?.('change', scenePreference); flatMedia?.removeEventListener?.('change', scenePreference); root.ownerDocument.removeEventListener('visibilitychange', sceneVisibility); workspace.dispose(); workspaceResizeObserver?.disconnect(); root.ownerDocument.defaultView?.removeEventListener?.('resize', workspaceWindowResize); drag.dispose(); unsubscribe?.(); root.removeEventListener('click', click); root.removeEventListener('change', change); root.removeEventListener('input', input); root.removeEventListener('submit', submit); root.removeEventListener('pointerdown', pointerdown); root.removeEventListener('pointermove', pointermove); root.removeEventListener('pointerup', pointerup); root.removeEventListener('pointercancel', pointercancel); root.removeEventListener('lostpointercapture', lostpointercapture); root.removeEventListener('dragstart', dragstart); root.removeEventListener('dragover', dragover); root.removeEventListener('drop', drop); root.removeEventListener('keydown', keydown); root.removeEventListener('focusout', menuFocusout); root.removeEventListener('contextmenu', contextmenu); root.ownerDocument.removeEventListener('pointerdown', outside); root.ownerDocument.removeEventListener('keydown', documentKeydown);
  };
  return { render, dispose, purgeForWorkspaceLock: () => dispose({ purge: true }) };
}
