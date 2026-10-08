import {
  WORKFLOW_DEFINITION_SCHEMA, cloneWorkflow, validateWorkflowDefinition,
  validateWorkflowDetail, validateWorkflowLayout, validateWorkflowReceipt, validateWorkflowSaveReady,
  validateWorkflowSummary,
} from './workflow-contract.mjs';
import { validateCapabilityDetail, validateCapabilitySummary } from './skills-model.mjs';

const encode = value => new TextEncoder().encode(JSON.stringify(value));
const clone = cloneWorkflow;
const emptyDefinition = () => ({ schemaVersion: WORKFLOW_DEFINITION_SCHEMA, title: 'Untitled workflow', nodes: [], controlEdges: [], dataEdges: [] });
const emptyLayout = () => ({ positions: [] });
const failure = result => ({ code: typeof result?.error?.code === 'string' ? result.error.code : 'workflow_unavailable', message: typeof result?.error?.message === 'string' ? result.error.message : 'The local workflow action did not return a confirmed result.' });
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const whole = value => Number.isSafeInteger(value) && value >= 0;
const position = (layout, index) => layout.positions.find(item => item.nodeIndex === index) ?? { nodeIndex: index, x: 0, y: 0 };
const caps = new Set(['input', 'capability', 'check', 'branch', 'checkpoint', 'output']);
const exactKeys = (value, keys) => object(value)
  && Object.keys(value).length === keys.length
  && keys.every(key => Object.prototype.hasOwnProperty.call(value, key));

function template(kind, index) {
  const id = `node_${index + 1}`;
  const title = kind[0].toUpperCase() + kind.slice(1);
  if (kind === 'input') return { id, title, operation: { kind }, inputs: [], outputs: [{ name: 'text', dataType: 'text' }] };
  if (kind === 'capability') return { id, title, operation: { kind, capabilityId: '', revisionId: '' }, inputs: [{ name: 'text', dataType: 'text' }], outputs: [{ name: 'text', dataType: 'text' }] };
  if (kind === 'check') return { id, title, operation: { kind, check: { kind: 'non_empty' } }, inputs: [{ name: 'value', dataType: 'text' }], outputs: [{ name: 'passed', dataType: 'boolean' }] };
  if (kind === 'branch') return { id, title, operation: { kind }, inputs: [{ name: 'condition', dataType: 'boolean' }], outputs: [] };
  if (kind === 'checkpoint') return { id, title, operation: { kind, prompt: 'Review this checkpoint.' }, inputs: [], outputs: [] };
  return { id, title, operation: { kind: 'output' }, inputs: [{ name: 'text', dataType: 'text' }], outputs: [] };
}
function nextSequence(nodes) {
  return nodes.reduce((highest, node) => {
    const match = /^node_(\d+)$/.exec(node.id);
    return match ? Math.max(highest, Number(match[1])) : highest;
  }, 0);
}
function uniqueTitle(nodes, base) {
  const titles = new Set(nodes.map(node => node.title));
  if (!titles.has(base)) return base;
  let number = 2;
  while (titles.has(`${base} ${number}`)) number += 1;
  return `${base} ${number}`;
}
function remapLayout(layout, edit) {
  const positions = layout.positions.map(item => ({ ...item }));
  if (edit.kind === 'insert') positions.forEach(item => { if (item.nodeIndex >= edit.index) item.nodeIndex += 1; });
  if (edit.kind === 'remove') {
    for (let at = positions.length - 1; at >= 0; at -= 1) {
      if (positions[at].nodeIndex === edit.index) positions.splice(at, 1);
      else if (positions[at].nodeIndex > edit.index) positions[at].nodeIndex -= 1;
    }
  }
  if (edit.kind === 'move') {
    positions.forEach(item => {
      if (item.nodeIndex === edit.from) item.nodeIndex = edit.to;
      else if (edit.from < edit.to && item.nodeIndex > edit.from && item.nodeIndex <= edit.to) item.nodeIndex -= 1;
      else if (edit.to < edit.from && item.nodeIndex >= edit.to && item.nodeIndex < edit.from) item.nodeIndex += 1;
    });
  }
  return { positions: positions.sort((a, b) => a.nodeIndex - b.nodeIndex) };
}

export const EMPTY_WORKFLOW_STATE = Object.freeze({
  bridgeAvailable: false, status: 'unavailable', pending: null, message: 'Open Rangoon desktop to author local workflows.',
  workflows: [], capabilities: [], capabilityDetail: null, opened: null, comparison: null, session: null,
  definition: null, layout: null, dirty: false, candidate: null, acknowledged: false, receipt: null,
  tab: 'canvas', selection: null, viewport: { x: 0, y: 0, zoom: 1 }, fieldEdit: null, canUndo: false, canRedo: false,
});

export function createWorkflowController({ bridge, playground = false, onChange = () => {}, onSaved = () => {} } = {}) {
  const invoke = playground === true ? null : typeof bridge === 'function' ? bridge : typeof bridge?.invoke === 'function' ? bridge.invoke.bind(bridge) : null;
  let state = { ...EMPTY_WORKFLOW_STATE, ...(playground === true ? { playground: true } : {}), bridgeAvailable: Boolean(invoke), status: invoke ? 'idle' : 'unavailable', message: invoke ? 'Refresh the local workflow library to begin.' : EMPTY_WORKFLOW_STATE.message };
  let undo = []; let redo = []; let editGeneration = 0; let requestGeneration = 0; let localNodeSequence = 0;
  let activeRequest = null;
  let playgroundGeneration = 0;
  let invalidatedRequest = null;
  const emit = () => onChange(clone(state));
  const set = changes => { state = { ...state, ...changes }; emit(); };
  const refuse = message => {
    set({ message });
    return false;
  };
  const call = async (command, args) => { try { return await invoke(command, args); } catch { return { outcome: 'failed' }; } };
  const locked = () => (!invoke && playground !== true) || Boolean(state.pending);
  const obsolete = request => {
    if (request !== requestGeneration) return true;
    if (invalidatedRequest !== request) return false;
    invalidatedRequest = null;
    activeRequest = null;
    set({ pending: null, message: 'A local workspace change made this response stale. Keep your draft and inspect again.' });
    return true;
  };
  const editing = () => Boolean(state.session && state.definition && state.layout && !state.pending && !state.fieldEdit);
  const snapshot = () => ({ definition: clone(state.definition), layout: clone(state.layout), dirty: state.dirty, selection: clone(state.selection) });
  const restore = value => { state = { ...state, definition: clone(value.definition), layout: clone(value.layout), dirty: value.dirty, selection: clone(value.selection), candidate: null, acknowledged: false, receipt: null }; editGeneration += 1; emit(); };
  const useMutation = action => {
    if (!editing()) return false;
    const before = snapshot();
    if (!action()) return false;
    undo.push(before); if (undo.length > 50) undo.shift(); redo = []; editGeneration += 1;
    set({ dirty: true, candidate: null, acknowledged: false, receipt: null, message: playground === true ? 'Synthetic edits stay in memory. Nothing is saved or inspected.' : 'Local workflow edits need a fresh inspection.' });
    return true;
  };
  const fieldTarget = target => target?.kind === 'workflow' || target?.kind === 'node' && whole(target.index) && target.index < (state.definition?.nodes.length ?? 0);
  const closeField = apply => {
    if (!state.fieldEdit) return true;
    if (!apply) { set({ fieldEdit: null }); return true; }
    const { target, patch } = state.fieldEdit;
    const nextDefinition = clone(state.definition);
    if (target.kind === 'workflow') Object.assign(nextDefinition, patch);
    else Object.assign(nextDefinition.nodes[target.index], patch);
    if (!validateWorkflowDefinition(nextDefinition)) return false;
    state = { ...state, fieldEdit: null };
    return useMutation(() => {
      state.definition = nextDefinition;
      return true;
    });
  };
  const detailFor = result => exactKeys(result, ['outcome', 'workflow']) && result.outcome === 'opened' ? validateWorkflowDetail(result.workflow) : null;
  const list = async () => {
    if (playground === true) return false;
    if (locked() || state.fieldEdit) return false;
    const request = ++requestGeneration; activeRequest = request; set({ pending: 'load', message: 'Refreshing local workflow library.' });
    const [workflows, capabilities] = await Promise.all([call('list_workflows'), call('list_capabilities')]);
    if (obsolete(request)) return false;
    const nextWorkflows = exactKeys(workflows, ['outcome', 'workflows']) && workflows.outcome === 'listed' && Array.isArray(workflows.workflows) && workflows.workflows.length <= 128 && new Set(workflows.workflows.map(item => item?.id)).size === workflows.workflows.length && workflows.workflows.every(validateWorkflowSummary) ? workflows.workflows : null;
    const nextCapabilities = exactKeys(capabilities, ['outcome', 'capabilities']) && capabilities.outcome === 'listed' && Array.isArray(capabilities.capabilities) && capabilities.capabilities.length <= 128 && new Set(capabilities.capabilities.map(item => item?.id)).size === capabilities.capabilities.length && capabilities.capabilities.every(validateCapabilitySummary) ? capabilities.capabilities : null;
    if (!nextWorkflows || !nextCapabilities) { set({ pending: null, status: state.workflows.length || state.capabilities.length ? 'ready' : 'failed', message: failure(!nextWorkflows ? workflows : capabilities).message }); return false; }
    set({ pending: null, status: 'ready', workflows: clone(nextWorkflows), capabilities: clone(nextCapabilities), message: 'Local workflow library refreshed.' }); return true;
  };
  const controller = {
    getState: () => clone({ ...state, canUndo: !locked() && undo.length > 0, canRedo: !locked() && redo.length > 0 }),
    load: list,
    async open(workflowId, revisionId = null) {
      if (playground === true) return false;
      if (locked() || state.session || state.dirty || state.fieldEdit || !/^workflow:[0-9a-f]{64}$/.test(workflowId) || revisionId !== null && !/^workflow-revision:[0-9a-f]{64}$/.test(revisionId)) return false;
      const request = ++requestGeneration; activeRequest = request; set({ pending: 'open', message: 'Opening saved workflow.' });
      const result = await call('open_workflow', encode({ schemaVersion: 'rangoon.workflow-open.v1', workflowId, selection: revisionId ? { kind: 'historical', revisionId } : { kind: 'head' } }));
      if (obsolete(request)) return false;
      const detail = detailFor(result);
      if (!detail || detail.revision.workflowId !== workflowId || revisionId === null && detail.revision.id !== detail.head.latestRevisionId || revisionId !== null && detail.revision.id !== revisionId) { set({ pending: null, message: failure(result).message }); return false; }
      undo = []; redo = []; set({ pending: null, status: 'ready', opened: clone(detail), definition: clone(detail.revision.definition), layout: clone(detail.revision.layout), session: null, dirty: false, candidate: null, acknowledged: false, receipt: null, fieldEdit: null, selection: null, message: 'Saved workflow open read-only. Start an explicit append draft to edit.' }); return true;
    },
    async beginNew() {
      if (playground === true) {
        if (state.dirty || state.fieldEdit || state.session) return false;
        undo = []; redo = []; localNodeSequence = 0;
        set({ status: 'playground', session: { kind: 'playground', generation: ++playgroundGeneration }, definition: emptyDefinition(), layout: emptyLayout(), selection: null, message: 'Synthetic canvas ready. No native or provider action.' });
        return true;
      }
      if (locked() || state.session || state.dirty || state.fieldEdit) return false;
      const request = ++requestGeneration; activeRequest = request; set({ pending: 'begin', message: 'Starting a new local workflow draft.' });
      const result = await call('begin_workflow_draft', encode({ schemaVersion: 'rangoon.workflow-draft.v1', target: { kind: 'new' } }));
      if (obsolete(request)) return false;
      if (!(exactKeys(result, ['outcome', 'schemaVersion', 'draftId', 'workflowId', 'parentRevisionId', 'workflow']) && result.outcome === 'draft_ready' && result.schemaVersion === 'rangoon.workflow-draft-session.v1' && /^workflow-draft:[0-9a-f]{64}$/.test(result.draftId) && /^workflow:[0-9a-f]{64}$/.test(result.workflowId) && result.parentRevisionId === null && result.workflow === null)) { set({ pending: null, message: failure(result).message }); return false; }
      undo = []; redo = []; localNodeSequence = 0; set({ pending: null, status: 'draft', opened: null, session: { draftId: result.draftId, workflowId: result.workflowId, parentRevisionId: null }, definition: emptyDefinition(), layout: emptyLayout(), dirty: false, candidate: null, acknowledged: false, receipt: null, fieldEdit: null, selection: null, message: 'New local draft ready.' }); return true;
    },
    async beginAppend() {
      if (playground === true) return false;
      const head = state.opened?.head;
      if (locked() || state.session || state.dirty || state.fieldEdit || !head || state.opened.revision.id !== head.latestRevisionId) return false;
      const request = ++requestGeneration; activeRequest = request; set({ pending: 'append', message: 'Starting an append draft from the current head.' });
      const result = await call('begin_workflow_draft', encode({ schemaVersion: 'rangoon.workflow-draft.v1', target: { kind: 'append', workflowId: head.id, expectedHeadId: head.latestRevisionId } }));
      if (obsolete(request)) return false;
      const detail = result?.workflow && validateWorkflowDetail(result.workflow);
      if (!(exactKeys(result, ['outcome', 'schemaVersion', 'draftId', 'workflowId', 'parentRevisionId', 'workflow']) && result.outcome === 'draft_ready' && result.schemaVersion === 'rangoon.workflow-draft-session.v1' && /^workflow-draft:[0-9a-f]{64}$/.test(result.draftId) && result.workflowId === head.id && result.parentRevisionId === head.latestRevisionId && detail && detail.head.latestRevisionId === head.latestRevisionId)) { set({ pending: null, message: failure(result).message }); return false; }
      if (detail.revision.id !== detail.head.latestRevisionId || detail.revision.id !== head.latestRevisionId) {
        set({ pending: null, message: 'The append response did not contain the selected current head.' });
        return false;
      }
      undo = [];
      redo = [];
      localNodeSequence = nextSequence(detail.revision.definition.nodes);
      set({ pending: null, status: 'draft', opened: clone(detail), session: { draftId: result.draftId, workflowId: result.workflowId, parentRevisionId: result.parentRevisionId }, definition: clone(detail.revision.definition), layout: clone(detail.revision.layout), dirty: false, candidate: null, acknowledged: false, receipt: null, fieldEdit: null, message: 'Append draft ready at the selected current head.' }); return true;
    },
    async clear(confirmed) {
      if (playground === true) {
        if (!confirmed) return false;
        undo = []; redo = [];
        set({ session: null, definition: null, layout: null, selection: null, dirty: false, candidate: null, acknowledged: false, receipt: null, fieldEdit: null, message: 'Synthetic canvas discarded. No persisted record changed.' });
        return true;
      }
      if (!confirmed || locked() || state.fieldEdit || !state.session) return false;
      const draftId = state.session.draftId; const request = ++requestGeneration; activeRequest = request; set({ pending: 'clear', message: 'Clearing the local draft session.' });
      const result = await call('clear_workflow_draft', encode({ schemaVersion: 'rangoon.workflow-clear.v1', draftId }));
      if (obsolete(request)) return false;
      if (!exactKeys(result, ['outcome']) || result.outcome !== 'cleared') { set({ pending: null, message: failure(result).message }); return false; }
      undo = []; redo = []; set({ pending: null, status: state.opened ? 'ready' : 'idle', session: null, definition: state.opened ? clone(state.opened.revision.definition) : null, layout: state.opened ? clone(state.opened.revision.layout) : null, dirty: false, candidate: null, acknowledged: false, receipt: null, fieldEdit: null, message: 'Local draft cleared.' }); return true;
    },
    async compare(leftRevisionId, rightRevisionId) {
      if (playground === true) return false;
      if (locked() || state.fieldEdit || !state.opened || !/^workflow-revision:[0-9a-f]{64}$/.test(leftRevisionId) || !/^workflow-revision:[0-9a-f]{64}$/.test(rightRevisionId)) return false;
      const workflowId = state.opened.head.id; const request = ++requestGeneration; activeRequest = request; set({ pending: 'compare', message: 'Comparing saved workflow revisions.' });
      const [leftResult, rightResult] = await Promise.all([leftRevisionId === state.opened.revision.id ? { outcome: 'opened', workflow: state.opened } : call('open_workflow', encode({ schemaVersion: 'rangoon.workflow-open.v1', workflowId, selection: { kind: 'historical', revisionId: leftRevisionId } })), rightRevisionId === state.opened.revision.id ? { outcome: 'opened', workflow: state.opened } : call('open_workflow', encode({ schemaVersion: 'rangoon.workflow-open.v1', workflowId, selection: { kind: 'historical', revisionId: rightRevisionId } }))]);
      if (obsolete(request)) return false;
      const left = detailFor(leftResult); const right = detailFor(rightResult);
      if (!left || !right || left.revision.workflowId !== workflowId || right.revision.workflowId !== workflowId || left.revision.id !== leftRevisionId || right.revision.id !== rightRevisionId) { set({ pending: null, message: 'The exact saved revisions could not be compared.' }); return false; }
      const summary = { definitionChanged: !same(left.revision.definition, right.revision.definition), layoutChanged: !same(left.revision.layout, right.revision.layout), nodeDelta: right.revision.definition.nodes.length - left.revision.definition.nodes.length, controlEdgeDelta: right.revision.definition.controlEdges.length - left.revision.definition.controlEdges.length, dataEdgeDelta: right.revision.definition.dataEdges.length - left.revision.definition.dataEdges.length };
      set({ pending: null, comparison: { left: clone(left), right: clone(right), summary }, message: 'Saved revisions compared read-only.' }); return true;
    },
    select(selection) { if (state.fieldEdit || selection !== null && !(selection && ['node', 'control', 'data'].includes(selection.kind) && whole(selection.index))) return false; set({ selection: clone(selection) }); return true; },
    setTab(tab) { if (playground === true && tab === 'diff' || !['canvas', 'outline', 'definition', 'diff'].includes(tab)) return false; set({ tab }); return true; },
    setViewport(patch) { if (!object(patch) || !Object.keys(patch).every(key => ['x', 'y', 'zoom'].includes(key)) || !Object.values(patch).every(Number.isFinite)) return false; const viewport = { ...state.viewport, ...patch }; if (viewport.zoom < .25 || viewport.zoom > 4) return false; set({ viewport }); return true; },
    editFields(target, patch) {
      const sameTarget = state.fieldEdit
        && state.fieldEdit.target.kind === target?.kind
        && state.fieldEdit.target.index === target?.index;
      if ((!editing() && !sameTarget) || !fieldTarget(target) || !object(patch) || Object.keys(patch).length === 0 || state.fieldEdit && !sameTarget) return false;
      set({ fieldEdit: { target: clone(target), patch: { ...(sameTarget ? state.fieldEdit.patch : {}), ...clone(patch) } } });
      return true;
    },
    applyFields() { return closeField(true); }, cancelFields() { return closeField(false); },
    addNode(kind, rawPosition = { x: 0, y: 0 }) { if (!caps.has(kind) || !rawPosition || !Number.isSafeInteger(rawPosition.x) || !Number.isSafeInteger(rawPosition.y) || Math.abs(rawPosition.x) > 100000 || Math.abs(rawPosition.y) > 100000 || state.definition?.nodes.length >= 128) return false; return useMutation(() => { const index = state.definition.nodes.length; const node = template(kind, localNodeSequence++); while (state.definition.nodes.some(item => item.id === node.id)) node.id = `node_${++localNodeSequence}`; node.title = uniqueTitle(state.definition.nodes, node.title); state.definition.nodes.push(node); state.layout = remapLayout(state.layout, { kind: 'insert', index }); state.layout.positions.push({ nodeIndex: index, x: rawPosition.x, y: rawPosition.y }); state.selection = { kind: 'node', index }; return true; }); },
    removeNode(index, confirmed) { if (!confirmed || !whole(index) || index >= (state.definition?.nodes.length ?? 0)) return false; return useMutation(() => { const id = state.definition.nodes[index].id; state.definition.nodes.splice(index, 1); state.definition.controlEdges = state.definition.controlEdges.filter(edge => edge.fromNode !== id && edge.toNode !== id); state.definition.dataEdges = state.definition.dataEdges.filter(edge => edge.fromNode !== id && edge.toNode !== id); state.layout = remapLayout(state.layout, { kind: 'remove', index }); state.selection = null; return true; }); },
    reorderNode(index, delta) { const target = index + delta; if (!whole(index) || !Number.isSafeInteger(delta) || !whole(target) || target >= (state.definition?.nodes.length ?? 0)) return false; return useMutation(() => { const [node] = state.definition.nodes.splice(index, 1); state.definition.nodes.splice(target, 0, node); state.layout = remapLayout(state.layout, { kind: 'move', from: index, to: target }); state.selection = { kind: 'node', index: target }; return true; }); },
    moveNode(index, x, y) { if (!whole(index) || index >= (state.definition?.nodes.length ?? 0) || !Number.isSafeInteger(x) || !Number.isSafeInteger(y) || Math.abs(x) > 100000 || Math.abs(y) > 100000) return false; const old = position(state.layout, index); if (old.x === x && old.y === y) return false; return useMutation(() => { const found = state.layout.positions.find(item => item.nodeIndex === index); if (found) { found.x = x; found.y = y; } else state.layout.positions.push({ nodeIndex: index, x, y }); return true; }); },
    connectControl(fromIndex, outlet, toIndex) {
      if (!editing()) return false;
      if (!whole(fromIndex) || !whole(toIndex) || !state.definition.nodes[fromIndex] || !state.definition.nodes[toIndex]) return refuse('Choose existing nodes for the control connection.');
      if (fromIndex === toIndex) return refuse('A control edge cannot connect a node to itself.');
      const from = state.definition.nodes[fromIndex];
      if (!['next', 'true', 'false'].includes(outlet) || from.operation.kind === 'branch' && !['true', 'false'].includes(outlet) || from.operation.kind !== 'branch' && outlet !== 'next') return refuse('That control outlet is not available on this node.');
      if (from.operation.kind === 'output') return refuse('An Output node cannot create a control connection.');
      if (state.definition.controlEdges.some(edge => edge.fromNode === from.id && edge.outlet === outlet)) return refuse('That control outlet already has a connection.');
      return useMutation(() => {
        state.definition.controlEdges.push({ fromNode: from.id, outlet, toNode: state.definition.nodes[toIndex].id });
        return true;
      });
    },
    connectData(fromIndex, fromPort, toIndex, toPort) {
      if (!editing()) return false;
      if (!whole(fromIndex) || !whole(toIndex) || !state.definition.nodes[fromIndex] || !state.definition.nodes[toIndex]) return refuse('Choose existing nodes for the data mapping.');
      if (fromIndex === toIndex) return refuse('A data mapping cannot connect a node to itself.');
      if (typeof fromPort !== 'string' || typeof toPort !== 'string') return refuse('Choose a source output and destination input.');
      const from = state.definition.nodes[fromIndex];
      const to = state.definition.nodes[toIndex];
      const source = from.outputs.find(port => port.name === fromPort);
      const target = to.inputs.find(port => port.name === toPort);
      if (!source) return refuse('That source output is not available on this node.');
      if (!target) return refuse('That destination input is not available on this node.');
      if (source.dataType !== target.dataType) return refuse('Data mappings require matching port types.');
      if (state.definition.dataEdges.some(edge => edge.toNode === to.id && edge.toPort === toPort)) return refuse('That destination input already has a mapping.');
      return useMutation(() => {
        state.definition.dataEdges.push({ fromNode: from.id, fromPort, toNode: to.id, toPort });
        return true;
      });
    },
    removeEdge(kind, index) { if (!['control', 'data'].includes(kind) || !whole(index)) return false; return useMutation(() => { const list = kind === 'control' ? state.definition.controlEdges : state.definition.dataEdges; if (index >= list.length) return false; list.splice(index, 1); state.selection = null; return true; }); },
    async chooseCapability(capabilityId) {
      if (playground === true) return false;
      if (locked() || state.fieldEdit || !/^capability:[0-9a-f]{64}$/.test(capabilityId)) return false;
      const request = ++requestGeneration;
      activeRequest = request;
      set({ pending: 'capability', message: 'Opening exact local capability.' });
      const result = await call('open_capability', { capabilityId, revisionId: null });
      if (obsolete(request)) return false;
      const detail = exactKeys(result, ['outcome', 'capability', 'alreadyApplied']) && result.outcome === 'opened' ? validateCapabilityDetail(result.capability) : null;
      if (!detail || typeof result.alreadyApplied !== 'boolean' || detail.id !== capabilityId || detail.revision.id !== detail.latestRevisionId) {
        set({ pending: null, message: failure(result).message });
        return false;
      }
      set({ pending: null, capabilityDetail: clone(detail), message: 'Exact local capability selected.' });
      return true;
    },
    pinCapability(nodeIndex, capabilityId, revisionId) { if (playground === true) return false; if (!whole(nodeIndex) || !/^capability:[0-9a-f]{64}$/.test(capabilityId) || !/^revision:[0-9a-f]{64}$/.test(revisionId)) return false; return useMutation(() => { const node = state.definition.nodes[nodeIndex]; if (!node || node.operation.kind !== 'capability' || state.capabilityDetail?.id !== capabilityId || !state.capabilityDetail.history.some(item => item.id === revisionId)) return false; node.operation = { kind: 'capability', capabilityId, revisionId }; return true; }); },
    undo() { if (locked() || !undo.length || state.fieldEdit) return false; redo.push(snapshot()); restore(undo.pop()); return true; }, redo() { if (locked() || !redo.length || state.fieldEdit) return false; undo.push(snapshot()); restore(redo.pop()); return true; },
    async inspect(intent) {
      if (playground === true) return false;
      const draftCandidate = state.candidate;
      const freshDraft = draftCandidate
        && draftCandidate.editGeneration === editGeneration
        && draftCandidate.revision.intent === 'draft'
        && draftCandidate.report.structurallyValid
        && draftCandidate.plan.unresolvedReferences === 0;
      if (!editing() || !['draft', 'validated'].includes(intent) || intent === 'validated' && !freshDraft) return false;
      const request = ++requestGeneration;
      activeRequest = request;
      const generation = editGeneration;
      const authoredDefinition = clone(state.definition);
      const authoredLayout = clone(state.layout);
      set({ pending: 'inspect', message: 'Inspecting the exact local workflow draft.' });
      const result = await call('inspect_workflow_save', encode({ schemaVersion: 'rangoon.workflow-inspect.v1', draftId: state.session.draftId, intent, definition: authoredDefinition, layout: authoredLayout }));
      if (obsolete(request)) return false;
      const candidate = validateWorkflowSaveReady(result, { ...state.session, definition: authoredDefinition, layout: authoredLayout, intent });
      if (!candidate || generation !== editGeneration || !same(state.definition, authoredDefinition) || !same(state.layout, authoredLayout)) { set({ pending: null, candidate: null, acknowledged: false, message: failure(result).message }); return false; }
      set({ pending: null, status: candidate.revision.intent === 'validated' ? 'validated' : 'draft', candidate: { ...clone(candidate), editGeneration: generation }, acknowledged: false, receipt: null, message: 'Native inspection ready for explicit acknowledgment.' }); return true;
    },
    acknowledge(value) { if (!state.candidate || state.candidate.editGeneration !== editGeneration || typeof value !== 'boolean') return false; set({ acknowledged: value }); return true; },
    async commit() { if (playground === true) return false; const candidate = state.candidate; if (locked() || state.fieldEdit || !candidate || !state.acknowledged || candidate.editGeneration !== editGeneration || !state.session) return false; const request = ++requestGeneration; activeRequest = request; set({ pending: 'commit', candidate: null, acknowledged: false, message: 'Saving the exact inspected local revision.' }); const result = await call('commit_workflow_save', encode({ schemaVersion: 'rangoon.workflow-commit.v1', previewId: candidate.previewId, expectedStateId: candidate.plan.expectedStateId, acknowledged: true })); if (obsolete(request)) return false; const receipt = exactKeys(result, ['outcome', 'receipt']) && result.outcome === 'saved' ? validateWorkflowReceipt(result.receipt, candidate) : null; if (!receipt) { set({ pending: null, status: 'draft', message: failure(result).message }); return false; }
      const saved = clone(receipt);
      set({ pending: null, status: 'saved', opened: clone(saved.workflow), session: null, receipt: saved, candidate: null, acknowledged: false, dirty: false, message: saved.alreadySaved ? 'Exact revision was already current.' : 'Local workflow revision saved.' });
      try {
        if (await onSaved(clone(saved)) === false) throw new Error('refresh_failed');
      } catch {
        set({ message: 'Local workflow revision saved. Library refresh did not finish.' });
      }
      return saved;
    },
    invalidate(message = 'A shared workspace change requires a fresh workflow inspection.') {
      if (playground === true) return false;
      editGeneration += 1;
      if (activeRequest !== null && state.pending) invalidatedRequest = activeRequest;
      const staleBegin = state.pending === 'begin' || state.pending === 'append';
      set({ candidate: null, acknowledged: false, capabilityDetail: null, ...(staleBegin ? { session: null } : {}), message: staleBegin ? 'A draft session may have changed while the workspace changed. Keep local edits, then explicitly start a new or append draft.' : message });
      return true;
    },
  };
  return controller;
}

function object(value) { return Boolean(value) && typeof value === 'object' && !Array.isArray(value); }
