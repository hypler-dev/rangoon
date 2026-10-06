import { validateCapabilityDetail as strictCapabilityDetail, validateCapabilitySummary as strictCapabilitySummary } from './skills-model.mjs';
import { validateWorkspaceData } from './workspace-model.mjs';

const encoder = new TextEncoder();
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, names) => object(value) && Object.keys(value).length === names.size && [...names].every(name => Object.hasOwn(value, name));
const bytes = value => encoder.encode(value).length;
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const id = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const whole = value => Number.isSafeInteger(value) && value >= 0;
const text = value => typeof value === 'string' && !value.includes('\0');
const title = value => text(value) && value.length > 0 && value.trim() === value && bytes(value) <= 160 && !/[\p{Cc}\u2028\u2029]/u.test(value);
const reason = value => text(value) && value.trim().length > 0 && bytes(value.trim()) <= 1024;
const range = value => only(value, new Set(['inputIndex', 'startByte', 'endByte'])) && whole(value.inputIndex) && whole(value.startByte) && whole(value.endByte) && value.endByte > value.startByte;

export const utf8Bytes = bytes;
export const validId = id;
export const validTitle = title;
export const validRange = range;

export function validateInputReference(value) {
  if (!object(value)) return null;
  if (value.kind === 'source' && only(value, new Set(['kind', 'sourceId', 'sha256'])) && id(value.sourceId, 'source:') && hex(value.sha256)) return value;
  if (value.kind === 'revision' && only(value, new Set(['kind', 'capabilityId', 'revisionId', 'sha256'])) && id(value.capabilityId, 'capability:') && id(value.revisionId, 'revision:') && hex(value.sha256)) return value;
  return null;
}

export function validateTarget(value) {
  if (!object(value)) return null;
  if (value.kind === 'new' && only(value, new Set(['kind']))) return value;
  if (value.kind === 'append' && only(value, new Set(['kind', 'capabilityId', 'expectedRevisionId'])) && id(value.capabilityId, 'capability:') && id(value.expectedRevisionId, 'revision:')) return value;
  return null;
}

function piece(value) {
  if (!object(value)) return false;
  if (value.kind === 'copy') return only(value, new Set(['kind', 'range'])) && range(value.range);
  if (value.kind === 'authored') return only(value, new Set(['kind', 'content', 'reason'])) && text(value.content) && bytes(value.content) <= 256 * 1024 && reason(value.reason);
  return value.kind === 'replace' && only(value, new Set(['kind', 'range', 'content', 'reason'])) && range(value.range) && text(value.content) && bytes(value.content) <= 256 * 1024 && reason(value.reason);
}

export function validateDraft(value) {
  if (!only(value, new Set(['schemaVersion', 'operation', 'inputs', 'outputs', 'exclusions', 'duplications', 'conflicts'])) || value.schemaVersion !== 'rangoon.composition-draft.v0' || !['decompose', 'merge', 'split'].includes(value.operation) || !Array.isArray(value.inputs) || !Array.isArray(value.outputs) || !Array.isArray(value.exclusions) || !Array.isArray(value.duplications) || !Array.isArray(value.conflicts)) return null;
  if (!value.inputs.length || value.inputs.length > 16 || !value.outputs.length || value.outputs.length > 16 || value.exclusions.length > 256 || value.duplications.length > 256 || value.conflicts.length > 128 || !value.inputs.every(validateInputReference) || !value.outputs.every(output => only(output, new Set(['title', 'pieces'])) && title(output.title) && Array.isArray(output.pieces) && output.pieces.every(piece))) return null;
  if ((value.operation === 'decompose' && (value.inputs.length !== 1 || value.inputs[0].kind !== 'source')) || (value.operation === 'merge' && (value.inputs.length < 2 || value.inputs.some(input => input.kind !== 'revision') || value.outputs.length !== 1)) || (value.operation === 'split' && (value.inputs.length !== 1 || value.inputs[0].kind !== 'revision' || value.outputs.length < 2))) return null;
  const identities = new Set(value.inputs.map(input => input.kind === 'source' ? `s:${input.sourceId}` : `r:${input.capabilityId}:${input.revisionId}`));
  if (identities.size !== value.inputs.length) return null;
  const annotations = list => list.every(item => only(item, new Set(['range', 'reason'])) && range(item.range) && reason(item.reason));
  if (!annotations(value.exclusions) || !annotations(value.duplications)) return null;
  const conflictIds = new Set();
  if (!value.conflicts.every(conflict => only(conflict, new Set(['id', 'title', 'ranges', 'context', 'resolution'])) && whole(conflict.id) && conflict.id > 0 && !conflictIds.has(conflict.id) && (conflictIds.add(conflict.id), true) && title(conflict.title) && Array.isArray(conflict.ranges) && conflict.ranges.length >= 2 && conflict.ranges.length <= 16 && conflict.ranges.every(range) && reason(conflict.context) && (conflict.resolution === null || reason(conflict.resolution)))) return null;
  return value;
}

export const validateCapabilitySummary = value => strictCapabilitySummary(value) ? value : null;
export const validateCapabilityDetail = value => strictCapabilityDetail(value) ? value : null;
export const validateWorkspace = value => validateWorkspaceData(value);

function exactCore(core, draft, targets, inputs) {
  if (!only(core, new Set(['schemaVersion', 'compositionId', 'saveable', 'outputs', 'mappings', 'coverage', 'diagnostics', 'authority'])) || core.schemaVersion !== 'rangoon.composition-core-preview.v0' || !id(core.compositionId, 'composition:') || typeof core.saveable !== 'boolean' || core.authority !== 'none' || !Array.isArray(core.outputs) || !Array.isArray(core.mappings) || !Array.isArray(core.coverage) || !Array.isArray(core.diagnostics) || core.outputs.length !== draft.outputs.length || core.mappings.length !== draft.outputs.reduce((count, output) => count + output.pieces.length, 0) || core.coverage.length > 4096) return false;
  let mappingIndex = 0;
  for (let outputIndex = 0; outputIndex < draft.outputs.length; outputIndex += 1) {
    const recipe = draft.outputs[outputIndex];
    const output = core.outputs[outputIndex];
    if (!only(output, new Set(['outputIndex', 'title', 'content', 'sha256', 'capabilityId', 'revisionId'])) || output.outputIndex !== outputIndex || output.title !== recipe.title || !text(output.content) || !hex(output.sha256) || !id(output.capabilityId, 'capability:') || !id(output.revisionId, 'revision:')) return false;
    let expected = ''; let outputByte = 0;
    for (let pieceIndex = 0; pieceIndex < recipe.pieces.length; pieceIndex += 1) {
      const piece = recipe.pieces[pieceIndex];
      const mapping = core.mappings[mappingIndex++];
      let materialized = ''; let inputRange = null;
      if (piece.kind === 'copy' || piece.kind === 'replace') {
        inputRange = piece.range;
        const content = inputs?.[piece.range.inputIndex]?.content;
        if (typeof content !== 'string' || !range(piece.range) || !point(content, piece.range.startByte) || !point(content, piece.range.endByte)) return false;
        materialized = piece.kind === 'copy' ? slice(content, piece.range.startByte, piece.range.endByte) : piece.content;
      } else materialized = piece.content;
      if (materialized === null || !only(mapping, new Set(['outputIndex', 'pieceIndex', 'outputStartByte', 'outputEndByte', 'kind', 'inputRange'])) || mapping.outputIndex !== outputIndex || mapping.pieceIndex !== pieceIndex || mapping.outputStartByte !== outputByte || mapping.outputEndByte !== outputByte + bytes(materialized) || mapping.kind !== piece.kind || (inputRange ? JSON.stringify(mapping.inputRange) !== JSON.stringify(inputRange) : mapping.inputRange !== null)) return false;
      expected += materialized; outputByte += bytes(materialized);
    }
    if (output.content !== expected || bytes(output.content) > 256 * 1024) return false;
    const target = targets[outputIndex];
    if (target.kind === 'new' && (!core.outputs[outputIndex] || !id(output.capabilityId, 'capability:'))) return false;
  }
  for (let inputIndex = 0; inputIndex < inputs.length; inputIndex += 1) {
    const content = inputs[inputIndex]?.content;
    if (typeof content !== 'string') return false;
    const segments = core.coverage.filter(segment => segment?.range?.inputIndex === inputIndex).sort((left, right) => left.range.startByte - right.range.startByte);
    const length = bytes(content);
    if ((length === 0 && segments.length) || (length > 0 && (!segments.length || segments[0].range.startByte !== 0 || segments.at(-1).range.endByte !== length || segments.some((segment, index) => index && segments[index - 1].range.endByte !== segment.range.startByte)))) return false;
  }
  if (!core.coverage.every(segment => only(segment, new Set(['range', 'disposition', 'references', 'duplicationAcknowledged'])) && range(segment.range) && segment.range.inputIndex < inputs.length && point(inputs[segment.range.inputIndex].content, segment.range.startByte) && point(inputs[segment.range.inputIndex].content, segment.range.endByte) && ['copied', 'duplicated', 'replaced', 'excluded', 'unassigned', 'mixed'].includes(segment.disposition) && Array.isArray(segment.references) && typeof segment.duplicationAcknowledged === 'boolean' && segment.references.every(reference => only(reference, new Set(['outputIndex', 'pieceIndex', 'kind'])) && whole(reference.outputIndex) && reference.outputIndex < draft.outputs.length && whole(reference.pieceIndex) && reference.pieceIndex < draft.outputs[reference.outputIndex].pieces.length && ['copy', 'authored', 'replace'].includes(reference.kind) && core.mappings.some(mapping => mapping.outputIndex === reference.outputIndex && mapping.pieceIndex === reference.pieceIndex && mapping.kind === reference.kind && mapping.inputRange && mapping.inputRange.inputIndex === segment.range.inputIndex && mapping.inputRange.startByte <= segment.range.startByte && mapping.inputRange.endByte >= segment.range.endByte)))) return false;
  const diagnostic = item => only(item, new Set(['code', 'inputIndex', 'outputIndex', 'range', 'conflictId'])) && ['invalid_output', 'unassigned', 'mixed_disposition', 'unacknowledged_duplicate', 'unresolved_conflict'].includes(item.code) && (item.inputIndex === null || (whole(item.inputIndex) && item.inputIndex < inputs.length)) && (item.outputIndex === null || (whole(item.outputIndex) && item.outputIndex < draft.outputs.length)) && (item.range === null || (range(item.range) && item.range.inputIndex < inputs.length && point(inputs[item.range.inputIndex].content, item.range.startByte) && point(inputs[item.range.inputIndex].content, item.range.endByte))) && (item.conflictId === null || (whole(item.conflictId) && item.conflictId > 0 && draft.conflicts.some(conflict => conflict.id === item.conflictId)));
  return core.saveable === (core.diagnostics.length === 0) && core.diagnostics.every(diagnostic);
}

function point(content, byte) { try { return whole(byte) && byte <= bytes(content) && new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(encoder.encode(content).slice(0, byte)) !== undefined; } catch { return false; } }
function slice(content, startByte, endByte) { try { return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(encoder.encode(content).slice(startByte, endByte)); } catch { return null; } }

export function validatePreviewEnvelope(value, draft, targets, inputs = []) {
  if (!only(value, new Set(['outcome', 'schemaVersion', 'previewId', 'expectedStateId', 'preview'])) || value.outcome !== 'ready' || value.schemaVersion !== 'rangoon.composition-session.v0' || !id(value.previewId, 'preview:') || !id(value.expectedStateId, 'workspace:') || !object(value.preview)) return null;
  const preview = value.preview;
  if (!only(preview, new Set(['schemaVersion', 'core', 'applicationId', 'appliedOutputs', 'saveable', 'authority'])) || preview.schemaVersion !== 'rangoon.composition-application-preview.v0' || !exactCore(preview.core, draft, targets, inputs) || !id(preview.applicationId, 'composition-application:') || preview.authority !== 'none' || typeof preview.saveable !== 'boolean' || (preview.saveable && !preview.core.saveable) || !Array.isArray(preview.appliedOutputs) || preview.appliedOutputs.length !== targets.length) return null;
  if (!preview.appliedOutputs.every((output, index) => only(output, new Set(['outputIndex', 'kind', 'capabilityId', 'parentRevisionId', 'revisionId'])) && output.outputIndex === index && ['new', 'append'].includes(output.kind) && id(output.capabilityId, 'capability:') && id(output.revisionId, 'revision:') && (output.parentRevisionId === null || id(output.parentRevisionId, 'revision:')) && output.kind === targets[index].kind && (output.kind === 'new' ? output.parentRevisionId === null && output.capabilityId === preview.core.outputs[index].capabilityId && output.revisionId === preview.core.outputs[index].revisionId : output.capabilityId === targets[index].capabilityId && output.parentRevisionId === targets[index].expectedRevisionId))) return null;
  return value;
}

export function validateReceipt(value, preview) {
  if (!only(value, new Set(['schemaVersion', 'compositionId', 'applicationId', 'committedStateId', 'capabilities', 'authority'])) || value.schemaVersion !== 'rangoon.composition-receipt.v0' || value.compositionId !== preview.core.compositionId || value.applicationId !== preview.applicationId || !id(value.committedStateId, 'workspace:') || value.authority !== 'none' || !Array.isArray(value.capabilities) || value.capabilities.length !== preview.appliedOutputs.length || !value.capabilities.every(validateCapabilityDetail)) return null;
  return value.capabilities.every((capability, index) => {
    const applied = preview.appliedOutputs[index]; const output = preview.core.outputs[index]; const revision = capability.revision;
    return capability.id === applied.capabilityId && capability.latestRevisionId === applied.revisionId && revision.id === applied.revisionId && revision.parentRevisionId === applied.parentRevisionId && revision.title === output.title && revision.content === output.content && revision.sha256 === output.sha256 && revision.review === null && revision.provenance?.kind === 'composition' && revision.provenance.applicationId === preview.applicationId && revision.provenance.compositionId === preview.core.compositionId && revision.provenance.outputIndex === index;
  }) ? value : null;
}
