import { validateCapabilityDetail, validateCapabilitySummary } from './skills-model.mjs';

const encoder = new TextEncoder();
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const id = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const clone = value => value === null || value === undefined ? value : structuredClone(value);
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
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
  let parsed;
  try { parsed = JSON.parse(manifestText); } catch { return false; }
  if (!same(parsed, candidate.manifest) || manifestText !== JSON.stringify(candidate.manifest)) return false;
  const expected = {
    schemaVersion: 'rangoon.instruction-candidate.v1', compilationId: report.compilationId, profileDigest: report.profileDigest,
    selectedRevision: report.selectedRevision,
    artifact: { path: report.artifact.path, sha256: report.artifact.sha256, byteLength: report.artifact.byteLength },
    reviewObservation: report.reviewObservation, diagnostics: report.diagnostics, authority: 'none',
  };
  return only(candidate.manifest, ['schemaVersion', 'compilationId', 'profileDigest', 'selectedRevision', 'artifact', 'reviewObservation', 'diagnostics', 'authority']) && same(candidate.manifest, expected);
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
  const publish = () => onChange(clone(state));
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const clearReport = () => ({ report: null, reportStale: false });
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
      if (!['text', 'diagnostics', 'evidence'].includes(tab)) return false;
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
      set({ listStatus: 'ready', capabilities: clone(capabilities), selected: opened.detail, pending: null, needsRefresh: false, reportStale: Boolean(state.report), error: null, message: state.report ? 'Saved skill selection refreshed. Existing report is stale; Compile again for a new observation.' : 'Saved skill selection refreshed.' });
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
    invalidate(message = 'Saved workspace changed. Refresh before compiling again.') {
      ++epoch;
      const changes = { pending: null, needsRefresh: true, reportStale: Boolean(state.report), error: null, message };
      set(changes); return true;
    },
  };
  return controller;
}
