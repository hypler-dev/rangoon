const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, fields) => object(value) && Object.keys(value).length === fields.length && fields.every(field => Object.prototype.hasOwnProperty.call(value, field));
const whole = value => Number.isSafeInteger(value) && value >= 0;
const bounded = (value, limit) => typeof value === 'string' && new TextEncoder().encode(value).length <= limit;
const id = (value, prefix) => typeof value === 'string' && new RegExp(`^${prefix}[0-9a-f]{64}$`).test(value);
const label = value => bounded(value, 160) && value.length > 0 && value.trim() === value && !/[\p{Cc}\u2028\u2029]/u.test(value);
// Definition decoding intentionally admits semantic defects. Native inspection reports
// those defects and is the only authority for structural eligibility.
const authoredText = value => typeof value === 'string';
const identifier = value => typeof value === 'string' && /^[a-z][a-z0-9_-]{0,47}$/.test(value);
const operationKinds = new Set(['input', 'capability', 'check', 'branch', 'checkpoint', 'output']);
const dataTypes = new Set(['text', 'json', 'boolean']);
const outletKinds = new Set(['next', 'true', 'false']);
const referenceStatuses = new Set(['invalid_reference', 'missing_capability', 'missing_revision', 'resolved']);
const diagnosticCodes = new Set(['invalid_title', 'invalid_node_count', 'invalid_node_id', 'duplicate_node_id', 'invalid_input_port_name', 'invalid_output_port_name', 'duplicate_input_port', 'duplicate_output_port', 'invalid_operation_ports', 'invalid_reference', 'invalid_check_needle', 'invalid_checkpoint_prompt', 'entry_count', 'output_missing', 'missing_control_endpoint', 'duplicate_control_edge', 'self_control_edge', 'invalid_control_outlet', 'control_cardinality', 'entry_incoming', 'cycle', 'unreachable_node', 'no_output_path', 'missing_data_endpoint', 'missing_output_port', 'missing_input_port', 'duplicate_data_edge', 'self_data_edge', 'type_mismatch', 'input_cardinality', 'data_not_dominating']);

export const WORKFLOW_DEFINITION_SCHEMA = 'rangoon.workflow-definition.v1';
export const WORKFLOW_REVISION_SCHEMA = 'rangoon.workflow-revision.v1';
export const WORKFLOW_DETAIL_SCHEMA = 'rangoon.workflow-detail.v1';
export const WORKFLOW_REPORT_SCHEMA = 'rangoon.workflow-validation.v1';
export const WORKFLOW_SAVE_SCHEMA = 'rangoon.workflow-save-session.v1';
export const MAX_WORKFLOW_NODES = 128;
export const MAX_WORKFLOW_POSITIONS = 128;

export const cloneWorkflow = value => value === null ? null : structuredClone(value);

function validPort(value) {
  return only(value, ['name', 'dataType']) && authoredText(value.name) && dataTypes.has(value.dataType);
}
function validOperation(value) {
  if (!object(value) || !operationKinds.has(value.kind)) return false;
  if (value.kind === 'capability') return only(value, ['kind', 'capabilityId', 'revisionId']) && authoredText(value.capabilityId) && authoredText(value.revisionId);
  if (value.kind === 'check') return only(value, ['kind', 'check']) && validCheck(value.check);
  if (value.kind === 'checkpoint') return only(value, ['kind', 'prompt']) && authoredText(value.prompt);
  return only(value, ['kind']);
}
function validCheck(value) {
  return (only(value, ['kind']) && value.kind === 'non_empty')
    || (only(value, ['kind', 'needle']) && value.kind === 'contains_text' && authoredText(value.needle));
}
function validNode(value) {
  return only(value, ['id', 'title', 'operation', 'inputs', 'outputs'])
    && authoredText(value.id) && authoredText(value.title) && validOperation(value.operation)
    && Array.isArray(value.inputs) && Array.isArray(value.outputs)
    && value.inputs.length <= 8 && value.outputs.length <= 8
    && value.inputs.every(validPort) && value.outputs.every(validPort);
}
function validControl(value) {
  return only(value, ['fromNode', 'outlet', 'toNode']) && authoredText(value.fromNode) && authoredText(value.toNode) && outletKinds.has(value.outlet);
}
function validData(value) {
  return only(value, ['fromNode', 'fromPort', 'toNode', 'toPort']) && [value.fromNode, value.fromPort, value.toNode, value.toPort].every(authoredText);
}
export function validateWorkflowDefinition(value) {
  if (!only(value, ['schemaVersion', 'title', 'nodes', 'controlEdges', 'dataEdges']) || value.schemaVersion !== WORKFLOW_DEFINITION_SCHEMA || !authoredText(value.title)
    || !Array.isArray(value.nodes) || !Array.isArray(value.controlEdges) || !Array.isArray(value.dataEdges)
    || value.nodes.length > MAX_WORKFLOW_NODES || value.controlEdges.length > 256 || value.dataEdges.length > 1024
    || !value.nodes.every(validNode) || !value.controlEdges.every(validControl) || !value.dataEdges.every(validData)) return null;
  try {
    return new TextEncoder().encode(JSON.stringify(value)).length <= 128 * 1024 ? value : null;
  } catch {
    return null;
  }
}
export function validateWorkflowLayout(value, nodes = null) {
  if (!only(value, ['positions']) || !Array.isArray(value.positions) || value.positions.length > MAX_WORKFLOW_POSITIONS) return null;
  const seen = new Set();
  if (!value.positions.every(position => only(position, ['nodeIndex', 'x', 'y']) && whole(position.nodeIndex) && Number.isSafeInteger(position.x) && Number.isSafeInteger(position.y) && Math.abs(position.x) <= 100000 && Math.abs(position.y) <= 100000 && !seen.has(position.nodeIndex) && (seen.add(position.nodeIndex), true) && (!nodes || position.nodeIndex < nodes.length))) return null;
  return value;
}
function validRevision(value) {
  return only(value, ['schemaVersion', 'id', 'workflowId', 'parentRevisionId', 'intent', 'definition', 'layout'])
    && value.schemaVersion === WORKFLOW_REVISION_SCHEMA && id(value.id, 'workflow-revision:') && id(value.workflowId, 'workflow:')
    && (value.parentRevisionId === null || id(value.parentRevisionId, 'workflow-revision:')) && ['draft', 'validated'].includes(value.intent)
    && Boolean(validateWorkflowDefinition(value.definition)) && Boolean(validateWorkflowLayout(value.layout, value.definition.nodes));
}
function validSummary(value) {
  return only(value, ['id', 'latestRevisionId', 'label', 'labelAdjusted', 'intent', 'structurallyValid', 'revisionCount', 'savedAtMs', 'unresolvedReferences'])
    && id(value.id, 'workflow:') && id(value.latestRevisionId, 'workflow-revision:') && label(value.label) && typeof value.labelAdjusted === 'boolean'
    && ['draft', 'validated'].includes(value.intent) && typeof value.structurallyValid === 'boolean' && whole(value.revisionCount) && value.revisionCount >= 1 && value.revisionCount <= 32
    && whole(value.savedAtMs) && whole(value.unresolvedReferences) && value.unresolvedReferences <= 128
    && (value.intent !== 'validated' || (value.structurallyValid && value.unresolvedReferences === 0));
}
function validHistory(value) {
  return only(value, ['id', 'parentRevisionId', 'intent', 'structurallyValid', 'savedAtMs']) && id(value.id, 'workflow-revision:')
    && (value.parentRevisionId === null || id(value.parentRevisionId, 'workflow-revision:')) && ['draft', 'validated'].includes(value.intent)
    && typeof value.structurallyValid === 'boolean' && whole(value.savedAtMs);
}
function validReference(value) {
  return only(value, ['nodeIndex', 'capabilityId', 'revisionId', 'status'])
    && whole(value.nodeIndex)
    && authoredText(value.capabilityId)
    && authoredText(value.revisionId)
    && referenceStatuses.has(value.status);
}
export function validateWorkflowDetail(value) {
  if (!only(value, ['schemaVersion', 'head', 'revision', 'savedAtMs', 'history', 'references']) || value.schemaVersion !== WORKFLOW_DETAIL_SCHEMA
    || !validSummary(value.head) || !validRevision(value.revision) || !whole(value.savedAtMs) || !Array.isArray(value.history) || !Array.isArray(value.references)
    || value.history.length < 1 || value.history.length > 32 || value.references.length > 128 || !value.history.every(validHistory) || !value.references.every(validReference)) return null;
  const historyIds = new Set(value.history.map(item => item.id));
  const selected = value.history.find(item => item.id === value.revision.id);
  const current = value.history.at(-1);
  if (historyIds.size !== value.history.length
    || !selected
    || !current
    || value.head.id !== value.revision.workflowId
    || value.head.revisionCount !== value.history.length
    || current.id !== value.head.latestRevisionId
    || value.history[0].parentRevisionId !== null
    || value.history.slice(1).some((item, index) => item.parentRevisionId !== value.history[index].id)
    || value.history.some(item => item.intent === 'validated' && !item.structurallyValid)
    || selected.parentRevisionId !== value.revision.parentRevisionId
    || selected.intent !== value.revision.intent
    || selected.savedAtMs !== value.savedAtMs) return null;
  if (value.revision.id === value.head.latestRevisionId
    && (value.head.intent !== selected.intent
      || value.head.structurallyValid !== selected.structurallyValid
      || value.head.savedAtMs !== selected.savedAtMs
      || value.head.unresolvedReferences !== value.references.filter(reference => reference.status !== 'resolved').length)) return null;
  const expectedReferences = value.revision.definition.nodes.map((node, nodeIndex) => node.operation.kind === 'capability' ? { nodeIndex, capabilityId: node.operation.capabilityId, revisionId: node.operation.revisionId } : null).filter(Boolean);
  if (expectedReferences.length !== value.references.length || !value.references.every((reference, index) => reference.nodeIndex === expectedReferences[index].nodeIndex && reference.capabilityId === expectedReferences[index].capabilityId && reference.revisionId === expectedReferences[index].revisionId)) return null;
  return value;
}
function validDiagnostic(value, definition) {
  if (!only(value, ['code', 'nodeIndex', 'controlEdgeIndex', 'dataEdgeIndex', 'portIndex']) || !diagnosticCodes.has(value.code)) return false;
  if (value.nodeIndex !== null && (!whole(value.nodeIndex) || value.nodeIndex >= definition.nodes.length)) return false;
  if (value.controlEdgeIndex !== null && (!whole(value.controlEdgeIndex) || value.controlEdgeIndex >= definition.controlEdges.length)) return false;
  if (value.dataEdgeIndex !== null && (!whole(value.dataEdgeIndex) || value.dataEdgeIndex >= definition.dataEdges.length)) return false;
  if (value.portIndex === null) return true;
  if (!whole(value.portIndex) || value.nodeIndex === null) return false;
  const node = definition.nodes[value.nodeIndex];
  if (['invalid_input_port_name', 'duplicate_input_port', 'input_cardinality'].includes(value.code)) return value.portIndex < node.inputs.length;
  if (['invalid_output_port_name', 'duplicate_output_port'].includes(value.code)) return value.portIndex < node.outputs.length;
  return false;
}
function nativeDependencies(definition) {
  const dependencies = [];
  const seen = new Set();
  for (const node of definition.nodes) {
    if (node.operation.kind !== 'capability') continue;
    const { capabilityId, revisionId } = node.operation;
    if (!id(capabilityId, 'capability:') || !id(revisionId, 'revision:')) continue;
    const key = `${capabilityId}/${revisionId}`;
    if (!seen.has(key)) {
      seen.add(key);
      dependencies.push({ capabilityId, revisionId });
    }
  }
  return dependencies;
}
function validReport(value, definition) {
  const nodeIds = definition.nodes.map(node => node.id);
  const order = new Map(Array.isArray(value?.topologicalOrder) ? value.topologicalOrder.map((nodeId, index) => [nodeId, index]) : []);
  const dependencies = nativeDependencies(definition);
  return only(value, ['schemaVersion', 'structurallyValid', 'definitionId', 'diagnostics', 'diagnosticsTruncated', 'topologicalOrder', 'dependencies', 'referenceStatus', 'executionStatus', 'authority'])
    && value.schemaVersion === WORKFLOW_REPORT_SCHEMA && typeof value.structurallyValid === 'boolean'
    && (value.definitionId === null || id(value.definitionId, 'workflow-definition:')) && Array.isArray(value.diagnostics) && value.diagnostics.length <= 128
    && value.diagnostics.every(item => validDiagnostic(item, definition))
    && typeof value.diagnosticsTruncated === 'boolean' && Array.isArray(value.topologicalOrder) && value.topologicalOrder.length <= 128 && value.topologicalOrder.every(authoredText) && new Set(value.topologicalOrder).size === value.topologicalOrder.length && Array.isArray(value.dependencies)
    && value.dependencies.length <= 128 && value.dependencies.every(item => only(item, ['capabilityId', 'revisionId']) && id(item.capabilityId, 'capability:') && id(item.revisionId, 'revision:'))
    && JSON.stringify(value.dependencies) === JSON.stringify(dependencies)
    && value.referenceStatus === 'unverified' && value.executionStatus === 'unavailable' && value.authority === 'none'
    && (value.structurallyValid
      ? value.definitionId !== null && !value.diagnostics.length && !value.diagnosticsTruncated && value.topologicalOrder.length === nodeIds.length && nodeIds.every(nodeId => order.has(nodeId)) && definition.controlEdges.every(edge => order.get(edge.fromNode) < order.get(edge.toNode))
      : value.definitionId === null && value.topologicalOrder.length === 0 && (value.diagnostics.length > 0 || value.diagnosticsTruncated));
}
function validPlan(value) {
  return only(value, ['schemaVersion', 'workflowId', 'revisionId', 'expectedStateId', 'expectedHeadId', 'alreadySaved', 'unresolvedReferences'])
    && value.schemaVersion === 'rangoon.workflow-save-plan.v1' && id(value.workflowId, 'workflow:') && id(value.revisionId, 'workflow-revision:') && id(value.expectedStateId, 'workspace:')
    && (value.expectedHeadId === null || id(value.expectedHeadId, 'workflow-revision:')) && typeof value.alreadySaved === 'boolean' && whole(value.unresolvedReferences) && value.unresolvedReferences <= 128;
}
export function validateWorkflowSaveReady(value, { draftId, workflowId, parentRevisionId, definition, layout, intent } = {}) {
  if (!only(value, ['outcome', 'schemaVersion', 'draftId', 'previewId', 'revision', 'report', 'plan']) || value.outcome !== 'save_ready' || value.schemaVersion !== WORKFLOW_SAVE_SCHEMA
    || !id(value.draftId, 'workflow-draft:') || !id(value.previewId, 'workflow-preview:') || !validRevision(value.revision) || !validReport(value.report, value.revision.definition) || !validPlan(value.plan)) return null;
  if (draftId && value.draftId !== draftId || workflowId && value.revision.workflowId !== workflowId || parentRevisionId !== undefined && value.revision.parentRevisionId !== parentRevisionId || intent && value.revision.intent !== intent
    || definition && JSON.stringify(value.revision.definition) !== JSON.stringify(definition) || layout && JSON.stringify(value.revision.layout) !== JSON.stringify(layout)
    || value.plan.workflowId !== value.revision.workflowId || value.plan.revisionId !== value.revision.id
    || (value.plan.expectedHeadId !== value.revision.parentRevisionId && !(value.plan.alreadySaved && value.plan.expectedHeadId === value.revision.id))
    || value.plan.unresolvedReferences > value.revision.definition.nodes.filter(node => node.operation.kind === 'capability').length
    || (value.revision.intent === 'validated' && (!value.report.structurallyValid || value.plan.unresolvedReferences !== 0))) return null;
  return value;
}
export function validateWorkflowReceipt(value, candidate) {
  if (!only(value, ['schemaVersion', 'workflow', 'alreadySaved', 'stateId']) || value.schemaVersion !== 'rangoon.workflow-save-receipt.v1' || !validateWorkflowDetail(value.workflow) || typeof value.alreadySaved !== 'boolean' || !id(value.stateId, 'workspace:')) return null;
  const revision = candidate?.revision;
  if (!revision || JSON.stringify(value.workflow.revision) !== JSON.stringify(revision) || value.workflow.head.latestRevisionId !== revision.id || value.workflow.head.id !== revision.workflowId || value.workflow.head.intent !== revision.intent
    || value.workflow.head.structurallyValid !== candidate.report.structurallyValid || value.workflow.head.unresolvedReferences !== candidate.plan.unresolvedReferences) return null;
  return value;
}
export function validateWorkflowSummary(value) { return validSummary(value) ? value : null; }
export { validateCapabilitySummary } from './skills-model.mjs';
