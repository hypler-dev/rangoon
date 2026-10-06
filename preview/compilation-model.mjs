import { validateCapabilityDetail, validateCapabilitySummary } from './skills-model.mjs';

const encoder = new TextEncoder();
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const id = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const clone = value => value === null || value === undefined ? value : structuredClone(value);
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const hasLoneSurrogate = value => typeof value === 'string' && /(?:[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?:^|[^\uD800-\uDBFF])[\uDC00-\uDFFF])/.test(value);
const wellFormed = value => typeof value === 'string' && (typeof value.isWellFormed === 'function' ? value.isWellFormed() : !hasLoneSurrogate(value));
const rustTrim = value => value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
const failure = result => ({
  code: typeof result?.error?.code === 'string' ? result.error.code : null,
  message: typeof result?.error?.message === 'string' ? result.error.message : 'The local compilation inspection did not return a confirmed result.',
});
const failed = () => ({ outcome: 'failed', error: { message: 'The local compilation inspection did not return a confirmed result.' } });

export const COMPILATION_PROFILES = Object.freeze({
  agents_md_v1: Object.freeze({
    id: 'agents_md_v1', version: 1, artifactPath: 'AGENTS.md',
    documentationUrl: 'https://learn.chatgpt.com/docs/agent-configuration/agents-md',
    documentationRetrievedOn: '2026-10-06', runtimeQualification: 'untested', targetBudget: 'unknown', semanticEquivalence: 'unverified',
    digest: 'e6485c07fa7134403582747e4930c6330b9db94aee094195c26621791475b8e2',
  }),
  claude_md_v1: Object.freeze({
    id: 'claude_md_v1', version: 1, artifactPath: 'CLAUDE.md',
    documentationUrl: 'https://code.claude.com/docs/en/memory',
    documentationRetrievedOn: '2026-10-06', runtimeQualification: 'untested', targetBudget: 'unknown', semanticEquivalence: 'unverified',
    digest: 'f9f8cc107e092ad8c62f4cbee0bdd1ea041002674d44a152bf600a7c9b5753a3',
  }),
});

export const EMPTY_COMPILATION_STATE = Object.freeze({
  bridgeAvailable: false, listStatus: 'unavailable', capabilities: [], selected: null, profile: null,
  pending: null, needsRefresh: false, report: null, reportStale: false, error: null,
  bundleExport: { status: 'idle', receipt: null, error: null },
  externalBundle: { status: 'idle', report: null, previous: null, error: null },
  message: 'Open Rangoon desktop to inspect local compilation.', tab: 'text',
});

function selectedRevision(detail) {
  const revision = detail.revision;
  return { capabilityId: detail.id, revisionId: revision.id, parentRevisionId: revision.parentRevisionId, title: revision.title, sha256: revision.sha256, provenance: revision.provenance };
}

function diagnosticsFor(profile, content) {
  const result = [
    { code: 'instruction_semantics_unverified', severity: 'warning', detail: null },
    { code: 'target_environment_unqualified', severity: 'warning', detail: null },
  ];
  if (profile === 'claude_md_v1' && content.includes('@')) result.push({ code: 'possible_include_syntax', severity: 'error', detail: null });
  if (profile === 'claude_md_v1' && content.includes('<!--')) result.push({ code: 'possible_comment_elision', severity: 'error', detail: null });
  return result;
}

function validProfile(value, profile) {
  const expected = COMPILATION_PROFILES[profile];
  return Boolean(expected) && only(value, ['id', 'version', 'artifactPath', 'documentationUrl', 'documentationRetrievedOn', 'runtimeQualification', 'targetBudget', 'semanticEquivalence'])
    && same(value, { id: expected.id, version: expected.version, artifactPath: expected.artifactPath, documentationUrl: expected.documentationUrl, documentationRetrievedOn: expected.documentationRetrievedOn, runtimeQualification: expected.runtimeQualification, targetBudget: expected.targetBudget, semanticEquivalence: expected.semanticEquivalence });
}

function validCandidate(candidate, manifestText, report) {
  if (!only(candidate, ['manifest', 'manifestSha256', 'candidateId']) || !hex(candidate.manifestSha256) || !id(candidate.candidateId, 'candidate:') || typeof manifestText !== 'string') return false;
  const manifestBytes = textBytes(manifestText);
  if (!manifestBytes || hasLoneSurrogate(manifestText) || manifestBytes.length < 1 || manifestBytes.length > MAX_MANIFEST_BYTES) return false;
  let parsed;
  try { parsed = JSON.parse(manifestText); } catch { return false; }
  if (!same(parsed, candidate.manifest) || manifestText !== JSON.stringify(candidate.manifest)) return false;
  const expected = {
    schemaVersion: 'rangoon.instruction-candidate.v1', compilationId: report.compilationId, profileDigest: report.profileDigest,
    selectedRevision: report.selectedRevision,
    artifact: { path: report.artifact.path, sha256: report.artifact.sha256, byteLength: report.artifact.byteLength },
    reviewObservation: report.reviewObservation, diagnostics: report.diagnostics, authority: 'none',
  };
  return only(candidate.manifest, ['schemaVersion', 'compilationId', 'profileDigest', 'selectedRevision', 'artifact', 'reviewObservation', 'diagnostics', 'authority'])
    && Array.isArray(candidate.manifest.diagnostics) && candidate.manifest.diagnostics.length === 2 && same(candidate.manifest, expected);
}

async function sha256Hex(text) {
  try {
    const digest = await globalThis.crypto?.subtle?.digest('SHA-256', encoder.encode(text));
    return digest ? Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('') : null;
  } catch { return null; }
}

function u64be(value) {
  if (!Number.isSafeInteger(value) || value < 0) return null;
  const bytes = new Uint8Array(8); let remainder = BigInt(value);
  for (let index = 7; index >= 0; index -= 1) { bytes[index] = Number(remainder & 0xffn); remainder >>= 8n; }
  return bytes;
}

function framedBytes(domain, parts) {
  const domainBytes = encoder.encode(domain);
  const lengths = parts.map(part => u64be(part.length));
  if (lengths.some(length => length === null)) return null;
  const total = domainBytes.length + parts.reduce((size, part) => size + 8 + part.length, 0);
  const bytes = new Uint8Array(total); let offset = 0;
  bytes.set(domainBytes, offset); offset += domainBytes.length;
  parts.forEach((part, index) => { bytes.set(lengths[index], offset); offset += 8; bytes.set(part, offset); offset += part.length; });
  return bytes;
}

async function framedIdentity(prefix, domain, parts) {
  const bytes = framedBytes(domain, parts);
  if (!bytes) return null;
  try {
    const digest = await globalThis.crypto?.subtle?.digest('SHA-256', bytes);
    return digest ? prefix + Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('') : null;
  } catch { return null; }
}

async function compilationIdentity(report) {
  const envelope = {
    schemaVersion: 'rangoon.compilation-identity.v1', profileDigest: report.profileDigest,
    selectedRevision: report.selectedRevision,
    artifact: { path: report.artifact.path, sha256: report.artifact.sha256, byteLength: report.artifact.byteLength },
    diagnostics: report.diagnostics, authority: 'none',
  };
  return framedIdentity('compilation:', 'rangoon.compilation.v1\0', [encoder.encode(JSON.stringify(envelope))]);
}

async function candidateIdentity(compilationId, manifestSha256) {
  return framedIdentity('candidate:', 'rangoon.instruction-candidate.v1\0', [encoder.encode(compilationId), encoder.encode(manifestSha256)]);
}

const BUNDLE_MAGIC = encoder.encode('RANGOON-INSTRUCTIONS-V1\n');
const MAX_BUNDLE_BYTES = 295016;
const MAX_MANIFEST_BYTES = 32768;
const MAX_ARTIFACT_BYTES = 262144;
const FILE_ERROR_COPY = Object.freeze({
  instruction_bundle_request_invalid: 'Instruction bundle request was rejected.',
  instruction_bundle_unavailable: 'Native instruction bundle action is unavailable.',
  workspace_busy: 'Another local operation is still running.',
  instruction_candidate_changed: 'Saved review evidence changed. Refresh and compile again.',
  instruction_bundle_name: 'Choose a new valid instruction bundle name.',
  instruction_bundle_read_failed: 'Selected instruction bundle could not be read.',
  instruction_bundle_exists: 'Selected instruction bundle name already exists.',
  instruction_bundle_write_failed: 'Bundle write was not confirmed. A file may remain; choose a new name before retrying.',
  bundle_too_large: 'Instruction bundle exceeds supported size limits.',
  bundle_invalid: 'Instruction bundle failed consistency validation.',
  bundle_not_candidate: 'Compilation is not an exportable candidate.',
  bundle_serialization_failed: 'Instruction bundle could not be prepared.',
  workspace_unavailable: 'Local workspace is unavailable.',
  workspace_schema: 'Local workspace schema is unavailable.',
  workspace_corrupt: 'Local workspace data could not be verified.',
  capability_invalid: 'Saved capability could not be verified.',
  compilation_invalid: 'Local compilation could not be verified.',
});
const FILE_ERROR_CODES = new Set(Object.keys(FILE_ERROR_COPY));
const fileFailure = result => ({
  code: typeof result?.error?.code === 'string' && FILE_ERROR_CODES.has(result.error.code) ? result.error.code : null,
  message: FILE_ERROR_COPY[result?.error?.code] ?? 'Instruction bundle action did not return a confirmed result.',
});
const knownNoOutputFailure = result => only(result, ['outcome', 'error', 'outputState']) && result.outcome === 'failed'
  && result.outputState === 'not_created' && only(result.error, ['code', 'message'])
  && typeof result.error.code === 'string' && FILE_ERROR_CODES.has(result.error.code) && typeof result.error.message === 'string';
const textBytes = value => typeof value === 'string' ? encoder.encode(value) : null;
const validText = value => wellFormed(value) && rustTrim(value).length > 0 && textBytes(value).length <= MAX_ARTIFACT_BYTES && !value.includes('\0');
const validTitle = value => wellFormed(value) && textBytes(value).length > 0 && textBytes(value).length <= 160 && rustTrim(value) === value && !/[\p{Cc}\u2028\u2029]/u.test(value);
const validTimestamp = value => Number.isSafeInteger(value) && value >= 0 && value <= 8640000000000000;
function validReview(value) {
  return only(value, ['reviewer', 'reviewedAtMs']) && value.reviewer === 'local_operator' && validTimestamp(value.reviewedAtMs);
}
function validProvenance(value) {
  if (!object(value) || typeof value.kind !== 'string') return false;
  if (value.kind === 'ordinary') return only(value, ['kind']);
  return value.kind === 'composition' && only(value, ['kind', 'applicationId', 'compositionId', 'outputIndex'])
    && id(value.applicationId, 'composition-application:') && id(value.compositionId, 'composition:')
    && Number.isSafeInteger(value.outputIndex) && value.outputIndex >= 0 && value.outputIndex <= 15;
}
function validSelected(value) {
  return only(value, ['capabilityId', 'revisionId', 'parentRevisionId', 'title', 'sha256', 'provenance'])
    && id(value.capabilityId, 'capability:') && id(value.revisionId, 'revision:')
    && (value.parentRevisionId === null || id(value.parentRevisionId, 'revision:'))
    && validTitle(value.title) && hex(value.sha256) && validProvenance(value.provenance);
}
function validDiagnostic(value) {
  return only(value, ['code', 'severity', 'detail']) && typeof value.code === 'string'
    && ['instruction_semantics_unverified', 'target_environment_unqualified', 'unsupported_requirement', 'possible_include_syntax', 'possible_comment_elision'].includes(value.code)
    && ['warning', 'error'].includes(value.severity) && value.detail === null;
}
function concat(parts) {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const output = new Uint8Array(length); let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}
async function revisionIdentity(selected, content) {
  return framedIdentity('revision:', 'rangoon.revision.v0\0', [
    encoder.encode(selected.capabilityId), encoder.encode(selected.parentRevisionId ?? ''), encoder.encode(selected.title), encoder.encode(content),
  ]);
}
async function bundleIdentity(bytes) {
  const length = u64be(bytes.length);
  if (!length) return null;
  try {
    const digest = await globalThis.crypto?.subtle?.digest('SHA-256', concat([encoder.encode('rangoon.instruction-bundle.v1\0'), length, bytes]));
    return digest ? `instruction-bundle:${Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')}` : null;
  } catch { return null; }
}
async function validBundleReport(report, manifestText) {
  if (!only(report, ['schemaVersion', 'compilationId', 'profile', 'profileDigest', 'selectedRevision', 'artifact', 'diagnostics', 'reviewObservation', 'readiness', 'candidate', 'authority'])
    || report.schemaVersion !== 'rangoon.compilation.v1' || report.readiness !== 'candidate' || report.authority !== 'none'
    || !id(report.compilationId, 'compilation:') || !validSelected(report.selectedRevision) || !validReview(report.reviewObservation)
    || !only(report.artifact, ['path', 'content', 'sha256', 'byteLength']) || !validText(report.artifact.content) || !hex(report.artifact.sha256)
    || !Number.isSafeInteger(report.artifact.byteLength) || report.artifact.byteLength !== textBytes(report.artifact.content).length
    || !Array.isArray(report.diagnostics) || report.diagnostics.length !== 2 || !report.diagnostics.every(validDiagnostic)) return null;
  const profile = report.profile?.id;
  const descriptor = COMPILATION_PROFILES[profile];
  if (!descriptor || !validProfile(report.profile, profile) || report.profileDigest !== descriptor.digest || report.artifact.path !== descriptor.artifactPath
    || report.artifact.sha256 !== report.selectedRevision.sha256 || report.diagnostics.some(item => item.severity === 'error')
    || !same(report.diagnostics, diagnosticsFor(profile, report.artifact.content))) return null;
  if (await sha256Hex(report.artifact.content) !== report.artifact.sha256) return null;
  if (report.selectedRevision.provenance.kind === 'ordinary' && await revisionIdentity(report.selectedRevision, report.artifact.content) !== report.selectedRevision.revisionId) return null;
  if (!validCandidate(report.candidate, manifestText, report)) return null;
  const manifestDigest = await sha256Hex(manifestText);
  if (manifestDigest !== report.candidate.manifestSha256) return null;
  const compilationId = await compilationIdentity(report);
  if (compilationId !== report.compilationId || await candidateIdentity(compilationId, manifestDigest) !== report.candidate.candidateId) return null;
  return clone(report);
}
async function validateExternal(result) {
  if (!only(result, ['outcome', 'schemaVersion', 'inspection', 'candidateManifestJson']) || result.outcome !== 'inspected'
    || result.schemaVersion !== 'rangoon.instruction-bundle-file.v1' || typeof result.candidateManifestJson !== 'string' || !object(result.inspection)) return null;
  const inspection = result.inspection;
  if (!only(inspection, ['schemaVersion', 'bundleId', 'sha256', 'byteLength', 'compilation', 'verification', 'authority'])
    || inspection.schemaVersion !== 'rangoon.instruction-bundle-inspection.v1' || !id(inspection.bundleId, 'instruction-bundle:') || !hex(inspection.sha256)
    || !Number.isSafeInteger(inspection.byteLength) || inspection.byteLength < 1 || inspection.byteLength > MAX_BUNDLE_BYTES
    || inspection.verification !== 'internal_consistency_only' || inspection.authority !== 'none') return null;
  const report = await validBundleReport(inspection.compilation, result.candidateManifestJson);
  if (!report) return null;
  const manifest = textBytes(result.candidateManifestJson); const artifact = textBytes(report.artifact.content);
  if (!manifest || !artifact || manifest.length < 1 || manifest.length > MAX_MANIFEST_BYTES || artifact.length < 1 || artifact.length > MAX_ARTIFACT_BYTES) return null;
  const prefix = concat([BUNDLE_MAGIC, u64be(manifest.length), u64be(artifact.length), manifest, artifact]);
  let checksumBytes;
  try { checksumBytes = await globalThis.crypto?.subtle?.digest('SHA-256', prefix); } catch { return null; }
  if (!checksumBytes) return null;
  const trailer = encoder.encode(Array.from(new Uint8Array(checksumBytes), byte => byte.toString(16).padStart(2, '0')).join(''));
  const bundle = concat([prefix, trailer]);
  const sha256 = await sha256HexBytes(bundle);
  const bundleId = await bundleIdentity(bundle);
  if (!sha256 || !bundleId || inspection.byteLength !== bundle.length || inspection.sha256 !== sha256 || inspection.bundleId !== bundleId) return null;
  return { inspection: { ...clone(inspection), compilation: report }, candidateManifestJson: result.candidateManifestJson };
}
async function sha256HexBytes(bytes) {
  try {
    const digest = await globalThis.crypto?.subtle?.digest('SHA-256', bytes);
    return digest ? Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('') : null;
  } catch { return null; }
}
async function validateExportReceipt(result, report) {
  if (!only(result, ['outcome', 'schemaVersion', 'bundleId', 'candidateId', 'sha256', 'byteLength', 'authority'])
    || result.outcome !== 'exported' || result.schemaVersion !== 'rangoon.instruction-bundle-export.v1'
    || result.authority !== 'none' || !id(result.bundleId, 'instruction-bundle:') || !id(result.candidateId, 'candidate:')
    || !hex(result.sha256) || !Number.isSafeInteger(result.byteLength)) return null;
  const manifestText = JSON.stringify(report.candidate.manifest);
  const external = await validateExternal({ outcome: 'inspected', schemaVersion: 'rangoon.instruction-bundle-file.v1', inspection: {
    schemaVersion: 'rangoon.instruction-bundle-inspection.v1', bundleId: result.bundleId, sha256: result.sha256, byteLength: result.byteLength,
    compilation: report, verification: 'internal_consistency_only', authority: 'none',
  }, candidateManifestJson: manifestText });
  return external && result.candidateId === report.candidate.candidateId ? clone(result) : null;
}

async function validateResponse(result, detail, profile) {
  if (!only(result, ['outcome', 'schemaVersion', 'observedCurrentHead', 'compilation', 'candidateManifestJson'])
    || result.outcome !== 'compiled' || result.schemaVersion !== 'rangoon.compilation-inspection.v1' || !id(result.observedCurrentHead, 'revision:') || !object(result.compilation)) return null;
  const report = result.compilation;
  const expectedSelection = selectedRevision(detail);
  const descriptor = COMPILATION_PROFILES[profile];
  if (!only(report, ['schemaVersion', 'compilationId', 'profile', 'profileDigest', 'selectedRevision', 'artifact', 'diagnostics', 'reviewObservation', 'readiness', 'candidate', 'authority'])
    || report.schemaVersion !== 'rangoon.compilation.v1' || !id(report.compilationId, 'compilation:') || !validProfile(report.profile, profile)
    || report.profileDigest !== descriptor.digest || !same(report.selectedRevision, expectedSelection) || report.authority !== 'none'
    || !only(report.artifact, ['path', 'content', 'sha256', 'byteLength']) || report.artifact.path !== descriptor.artifactPath
    || report.artifact.content !== detail.revision.content || report.artifact.sha256 !== detail.revision.sha256
    || !Number.isSafeInteger(report.artifact.byteLength) || report.artifact.byteLength !== encoder.encode(detail.revision.content).length
    || !Array.isArray(report.diagnostics) || !same(report.diagnostics, diagnosticsFor(profile, detail.revision.content))
    || !same(report.reviewObservation, detail.revision.review)) return null;
  const hasError = report.diagnostics.some(item => item.severity === 'error');
  const readiness = hasError ? 'blocked' : detail.revision.review ? 'candidate' : 'review_required';
  if (report.readiness !== readiness) return null;
  const compilationId = await compilationIdentity(report);
  if (compilationId !== report.compilationId) return null;
  if (readiness !== 'candidate') return report.candidate === null && result.candidateManifestJson === null ? { ...result, compilation: clone(report) } : null;
  if (!validCandidate(report.candidate, result.candidateManifestJson, report)) return null;
  const digest = await sha256Hex(result.candidateManifestJson);
  if (digest !== report.candidate.manifestSha256) return null;
  const candidateId = await candidateIdentity(report.compilationId, digest);
  return candidateId === report.candidate.candidateId ? { ...result, compilation: clone(report) } : null;
}

export function createCompilationController({ invoke, onChange = () => {} } = {}) {
  const bridgeAvailable = typeof invoke === 'function';
  let state = { ...EMPTY_COMPILATION_STATE, bridgeAvailable, listStatus: bridgeAvailable ? 'idle' : 'unavailable', message: bridgeAvailable ? 'Refresh local skills, then choose a revision and profile.' : EMPTY_COMPILATION_STATE.message };
  let epoch = 0;
  let fileEpoch = 0;
  const publish = () => onChange(clone(state));
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const clearReport = () => ({ report: null, reportStale: false, bundleExport: { status: 'idle', receipt: null, error: null } });
  const call = async (command, args) => { try { return await invoke(command, args); } catch { return failed(); } };
  const clearSelection = message => set({ selected: null, ...clearReport(), needsRefresh: false, error: null, message });

  async function openSelection(capabilityId, revisionId, request) {
    const result = await call('open_capability', { capabilityId, revisionId });
    if (request !== epoch) return null;
    const detail = result?.outcome === 'opened' ? validateCapabilityDetail(result.capability) : null;
    if (!detail || detail.id !== capabilityId || detail.revision.id !== (revisionId ?? detail.latestRevisionId)) return { error: failure(result) };
    return { detail };
  }

  const controller = {
    getState: () => clone(state),
    setTab(tab) {
      if (!['text', 'diagnostics', 'evidence', 'external'].includes(tab)) return false;
      set({ tab }); return true;
    },
    selectProfile(profile) {
      if (state.pending || (profile !== null && !Object.hasOwn(COMPILATION_PROFILES, profile)) || profile === state.profile) return false;
      set({ profile, ...clearReport(), error: null, message: profile ? 'Compilation profile selected. Choose Compile to inspect exact local output.' : 'Choose a closed compilation profile.' });
      return true;
    },
    async open(capabilityId, revisionId = null) {
      if (!bridgeAvailable || state.pending || state.needsRefresh || !id(capabilityId, 'capability:') || (revisionId !== null && !id(revisionId, 'revision:'))) return false;
      const request = ++epoch;
      set({ pending: 'open', error: null, message: 'Opening saved skill revision…' });
      const opened = await openSelection(capabilityId, revisionId, request);
      if (!opened) return false;
      if (opened.error) { set({ pending: null, error: opened.error, message: opened.error.message }); return false; }
      set({ selected: opened.detail, pending: null, ...clearReport(), error: null, message: opened.detail.revision.id === opened.detail.latestRevisionId ? 'Saved skill current head selected.' : 'Saved historical skill revision selected.' });
      return true;
    },
    async refresh() {
      if (!bridgeAvailable || state.pending) return false;
      const request = ++epoch;
      const prior = state.selected ? { id: state.selected.id, revisionId: state.selected.revision.id } : null;
      set({ listStatus: 'loading', pending: 'refresh', error: null, message: 'Refreshing saved local skills…' });
      const listed = await call('list_capabilities');
      if (request !== epoch) return false;
      const capabilities = listed?.outcome === 'listed' && Array.isArray(listed.capabilities) && listed.capabilities.length <= 128
        && new Set(listed.capabilities.map(item => item?.id)).size === listed.capabilities.length && listed.capabilities.every(validateCapabilitySummary) ? listed.capabilities : null;
      if (!capabilities) { const issue = failure(listed); set({ listStatus: 'failed', pending: null, error: issue, message: issue.message }); return false; }
      if (!prior) { set({ listStatus: 'ready', capabilities: clone(capabilities), pending: null, needsRefresh: false, error: null, message: capabilities.length ? 'Saved local skills refreshed. Choose one to inspect.' : 'No saved local skills are available yet.' }); return true; }
      if (!capabilities.some(item => item.id === prior.id)) { set({ listStatus: 'ready', capabilities: clone(capabilities), pending: null }); clearSelection('Selected saved skill was removed.'); return true; }
      const opened = await openSelection(prior.id, prior.revisionId, request);
      if (!opened) return false;
      if (opened.error) {
        if (opened.error.code === 'capability_not_found') { set({ listStatus: 'ready', capabilities: clone(capabilities), pending: null }); clearSelection('Selected saved skill revision was removed.'); return true; }
        set({ listStatus: 'failed', pending: null, error: opened.error, message: opened.error.message }); return false;
      }
      set({ listStatus: 'ready', capabilities: clone(capabilities), selected: opened.detail, pending: null, needsRefresh: false, reportStale: Boolean(state.report), bundleExport: { status: 'idle', receipt: null, error: null }, error: null, message: state.report ? 'Saved skill selection refreshed. Existing report is stale; Compile again for a new observation.' : 'Saved skill selection refreshed.' });
      return true;
    },
    async compile() {
      if (!bridgeAvailable || state.pending || state.needsRefresh || !state.selected || !state.profile) return false;
      const detail = clone(state.selected); const profile = state.profile; const request = ++epoch;
      const payload = { schemaVersion: 'rangoon.compile-request.v1', capabilityId: detail.id, revisionId: detail.revision.id, profile };
      set({ pending: 'generate', error: null, message: 'Compiling exact saved local revision…' });
      const result = await call('compile_capability', encoder.encode(JSON.stringify(payload)));
      if (request !== epoch) return false;
      const inspected = await validateResponse(result, detail, profile);
      if (request !== epoch) return false;
      if (!inspected) {
        const issue = failure(result);
        const retained = Boolean(state.report);
        const retainedLabel = state.reportStale ? 'Retained prior report is stale.' : 'Existing report still matches current selection.';
        set({ pending: null, error: issue, message: retained ? `Compilation did not finish. ${retainedLabel} ${issue.message}` : issue.message });
        return false;
      }
      set({ pending: null, report: inspected, reportStale: false, tab: 'text', error: null, message: inspected.compilation.readiness === 'candidate' ? 'Local static candidate inspected. Runtime remains untested.' : inspected.compilation.readiness === 'blocked' ? 'Compilation is blocked by static compatibility diagnostics.' : 'Compilation needs local content review before a candidate exists.' });
      return true;
    },
    async exportBundle() {
      if (!bridgeAvailable || state.pending || state.needsRefresh || state.reportStale || !state.selected || !state.profile
        || state.report?.compilation?.readiness !== 'candidate' || !state.report?.compilation?.candidate) return false;
      const report = clone(state.report.compilation);
      const selection = clone(state.selected); const profile = state.profile; const request = ++fileEpoch;
      const payload = { schemaVersion: 'rangoon.instruction-bundle-export-request.v1', capabilityId: selection.id, revisionId: selection.revision.id, profile, expectedCandidateId: report.candidate.candidateId };
      set({ pending: 'export', bundleExport: { status: 'pending', receipt: null, error: null }, error: null, message: 'Exporting exact reviewed candidate…' });
      let result;
      try { result = await invoke('export_instruction_bundle', encoder.encode(JSON.stringify(payload))); } catch { result = null; }
      if (request !== fileEpoch) return false;
      let receipt = null;
      try { receipt = await validateExportReceipt(result, report); } catch { receipt = null; }
      if (request !== fileEpoch) return false;
      if (receipt) {
        set({ pending: null, bundleExport: { status: 'exported', receipt, error: null }, error: null, message: 'Instruction bundle write and file sync completed. Future disk integrity is unconfirmed.' });
        return true;
      }
      if (result?.outcome === 'cancelled' && only(result, ['outcome'])) {
        set({ pending: null, bundleExport: { status: 'cancelled', receipt: null, error: null }, error: null, message: 'Instruction bundle export cancelled.' });
        return false;
      }
      const issue = fileFailure(result);
      const uncertain = !knownNoOutputFailure(result);
      const changed = issue.code === 'instruction_candidate_changed';
      set({ pending: null, needsRefresh: changed ? true : state.needsRefresh, reportStale: changed ? Boolean(state.report) : state.reportStale,
        bundleExport: { status: uncertain ? 'uncertain' : 'failed', receipt: null, error: issue }, error: issue,
        message: uncertain ? 'Instruction bundle output is uncertain. A file may remain; choose a new name before retrying.' : issue.message });
      return false;
    },
    async inspectBundle() {
      if (!bridgeAvailable || state.pending) return false;
      const request = ++fileEpoch;
      set({ pending: 'inspect-bundle', externalBundle: { status: 'pending', report: state.externalBundle.report, previous: state.externalBundle.report ?? state.externalBundle.previous, error: null }, error: null, message: 'Inspecting selected external instruction bundle…' });
      let result;
      try { result = await invoke('inspect_instruction_bundle', encoder.encode(JSON.stringify({ schemaVersion: 'rangoon.instruction-bundle-inspect-request.v1' }))); } catch { result = null; }
      if (request !== fileEpoch) return false;
      let external = null;
      try { external = await validateExternal(result); } catch { external = null; }
      if (request !== fileEpoch) return false;
      if (external) {
        set({ pending: null, tab: 'external', externalBundle: { status: 'inspected', report: external, previous: null, error: null }, error: null, message: 'External instruction bundle passed internal-consistency checks. Authority remains none.' });
        return true;
      }
      const prior = state.externalBundle.report ?? state.externalBundle.previous;
      if (result?.outcome === 'cancelled' && only(result, ['outcome'])) {
        set({ pending: null, externalBundle: { status: 'cancelled', report: null, previous: prior, error: null }, error: null, message: 'External instruction bundle inspection cancelled.' });
        return false;
      }
      const issue = fileFailure(result);
      set({ pending: null, externalBundle: { status: 'failed', report: null, previous: prior, error: issue }, error: issue, message: issue.message });
      return false;
    },
    invalidate(message = 'Saved workspace changed. Refresh before compiling again.') {
      const filePending = state.pending === 'export' || state.pending === 'inspect-bundle';
      if (!filePending) ++epoch;
      const changes = { pending: filePending ? state.pending : null, needsRefresh: true, reportStale: Boolean(state.report), error: null, message };
      set(changes); return true;
    },
  };
  return controller;
}
