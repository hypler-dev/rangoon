import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createCompilationController } from '../preview/compilation-model.mjs';

const fixture = JSON.parse(await readFile(new URL('../fixtures/compilation/instruction-v1.json', import.meta.url), 'utf8'));
const candidateCase = fixture.cases.find(item => item.name === 'agents_exact_bom_crlf_unicode');
const unreviewedCase = fixture.cases.find(item => item.name === 'unreviewed_same_compilation');
const blockedCase = fixture.cases.find(item => item.name === 'blocked_wins_over_missing_review');
const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const nextRevisionId = `revision:${'3'.repeat(64)}`;

function detailFor(vector, { latestRevisionId = vector.revision.id, history = null } = {}) {
  const revision = structuredClone(vector.revision);
  const summary = ({ content, ...value }) => value;
  const records = history ?? [summary(revision)];
  const origin = revision.provenance.kind === 'composition'
    ? { kind: 'composition', compositionId: revision.provenance.compositionId, operation: 'split', outputIndex: revision.provenance.outputIndex, inputs: [{ kind: 'revision', capabilityId: `capability:${'4'.repeat(64)}`, revisionId: `revision:${'5'.repeat(64)}`, sha256: '6'.repeat(64) }] }
    : { kind: 'source', sourceId, fragmentId, sourceName: 'fixture.md', span: { startByte: 0, endByte: new TextEncoder().encode(revision.content).length, startLine: 1, endLine: 1 }, originalText: revision.content };
  return {
    schemaVersion: 'rangoon.capability.v1', id: vector.capabilityId,
    origin,
    latestRevisionId, revision, history: records, authority: 'none',
  };
}
function summaryFor(detail) {
  const origin = detail.origin.kind === 'source' ? { kind: 'source', sourceId, fragmentId } : { kind: 'composition', compositionId: detail.origin.compositionId, operation: detail.origin.operation, outputIndex: detail.origin.outputIndex };
  return { id: detail.id, origin, latestRevisionId: detail.latestRevisionId, title: detail.revision.title, reviewed: Boolean(detail.revision.review), revisionCount: detail.history.length };
}
function inspected(vector, observedCurrentHead = vector.revision.id) {
  const report = structuredClone(vector.expected);
  return { outcome: 'compiled', schemaVersion: 'rangoon.compilation-inspection.v1', observedCurrentHead, compilation: report, candidateManifestJson: report.candidate ? JSON.stringify(report.candidate.manifest) : null };
}
async function digest(text) {
  const bytes = new TextEncoder().encode(text);
  const hash = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
}
function u64be(value) {
  const bytes = new Uint8Array(8); let remainder = BigInt(value);
  for (let index = 7; index >= 0; index -= 1) { bytes[index] = Number(remainder & 0xffn); remainder >>= 8n; }
  return bytes;
}
async function framed(prefix, domain, parts) {
  const encoder = new TextEncoder(); const all = [encoder.encode(domain), ...parts.flatMap(part => [u64be(part.length), part])];
  const bytes = new Uint8Array(all.reduce((total, part) => total + part.length, 0)); let offset = 0;
  for (const part of all) { bytes.set(part, offset); offset += part.length; }
  const hash = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return prefix + Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
}
async function internallyConsistentBogusCompilation(result) {
  const report = result.compilation; const candidate = report.candidate;
  report.compilationId = `compilation:${'f'.repeat(64)}`;
  candidate.manifest.compilationId = report.compilationId;
  result.candidateManifestJson = JSON.stringify(candidate.manifest);
  candidate.manifestSha256 = await digest(result.candidateManifestJson);
  candidate.candidateId = await framed('candidate:', 'rangoon.instruction-candidate.v1\0', [new TextEncoder().encode(report.compilationId), new TextEncoder().encode(candidate.manifestSha256)]);
  return result;
}
function listed(vector, options = {}) { return { outcome: 'listed', capabilities: options.empty ? [] : [summaryFor(detailFor(vector, options))] }; }
function controller(overrides = {}) {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push([command, args]);
    if (command === 'list_capabilities') return listed(candidateCase);
    if (command === 'open_capability') return { outcome: 'opened', capability: detailFor(candidateCase), alreadyApplied: false };
    if (command === 'compile_capability') return inspected(candidateCase);
    return { outcome: 'failed', error: { code: 'unexpected', message: 'Unexpected command.' } };
  };
  return { model: createCompilationController({ invoke: overrides.invoke ?? invoke }), calls };
}

test('browser state stays unavailable and native refresh distinguishes empty and failed libraries', async () => {
  const browser = createCompilationController();
  assert.equal(browser.getState().bridgeAvailable, false);
  assert.equal(browser.getState().profile, null);
  assert.equal(await browser.refresh(), false);
  const empty = controller({ invoke: async command => command === 'list_capabilities' ? listed(candidateCase, { empty: true }) : { outcome: 'failed' } }).model;
  assert.equal(await empty.refresh(), true);
  assert.deepEqual(empty.getState().capabilities, []);
  const broken = controller({ invoke: async () => ({ outcome: 'failed', error: { code: 'workspace_unavailable', message: 'Unavailable' } }) }).model;
  assert.equal(await broken.refresh(), false);
  assert.equal(broken.getState().listStatus, 'failed');
});

test('explicit selection and profile send exact closed raw request and accept fixture candidate', async () => {
  const { model, calls } = controller();
  await model.refresh();
  assert.equal(model.getState().selected, null);
  assert.equal(await model.open(candidateCase.capabilityId), true);
  assert.equal(model.selectProfile('agents_md_v1'), true);
  assert.equal(model.setTab('evidence'), true);
  assert.equal(await model.compile(), true);
  const call = calls.find(([name]) => name === 'compile_capability');
  assert.ok(call[1] instanceof Uint8Array);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(call[1])), { schemaVersion: 'rangoon.compile-request.v1', capabilityId: candidateCase.capabilityId, revisionId: candidateCase.revision.id, profile: 'agents_md_v1' });
  assert.equal(model.getState().report.compilation.readiness, 'candidate');
  assert.equal(model.getState().reportStale, false);
  assert.equal(model.getState().tab, 'text');
});

test('invalid report, noncanonical candidate manifest, and manifest digest mismatch fail closed', async () => {
  let attempt = 0;
  const { model } = controller({ invoke: async (command, args) => {
    if (command === 'list_capabilities') return listed(candidateCase);
    if (command === 'open_capability') return { outcome: 'opened', capability: detailFor(candidateCase), alreadyApplied: false };
    const result = inspected(candidateCase);
    if (attempt++ === 0) result.compilation.authority = 'granted';
    else if (attempt === 2) result.compilation.candidate.manifestSha256 = '0'.repeat(64);
    else result.candidateManifestJson += ' ';
    return result;
  } });
  await model.refresh(); await model.open(candidateCase.capabilityId); model.selectProfile('agents_md_v1');
  assert.equal(await model.compile(), false);
  assert.equal(model.getState().report, null);
  assert.equal(await model.compile(), false);
  assert.equal(model.getState().report, null);
  assert.equal(await model.compile(), false);
  assert.equal(model.getState().report, null);
});

test('rejects internally consistent bogus identities, mismatched selection/profile, and absent Web Crypto', async () => {
  let attempt = 0;
  const { model } = controller({ invoke: async command => {
    if (command === 'list_capabilities') return listed(candidateCase);
    if (command === 'open_capability') return { outcome: 'opened', capability: detailFor(candidateCase), alreadyApplied: false };
    const result = inspected(candidateCase);
    if (attempt++ === 0) return internallyConsistentBogusCompilation(result);
    if (attempt === 2) result.compilation.candidate.candidateId = `candidate:${'f'.repeat(64)}`;
    else if (attempt === 3) result.compilation.selectedRevision.revisionId = `revision:${'f'.repeat(64)}`;
    else result.compilation.profile.id = 'claude_md_v1';
    return result;
  } });
  await model.refresh(); await model.open(candidateCase.capabilityId); model.selectProfile('agents_md_v1');
  for (let index = 0; index < 4; index += 1) assert.equal(await model.compile(), false);

  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  Object.defineProperty(globalThis, 'crypto', { value: undefined, configurable: true });
  try {
    const unavailable = controller().model;
    await unavailable.refresh(); await unavailable.open(candidateCase.capabilityId); unavailable.selectProfile('agents_md_v1');
    assert.equal(await unavailable.compile(), false);
  } finally { Object.defineProperty(globalThis, 'crypto', descriptor); }
});

test('accepts every no-requirements checked-in compiler vector', async () => {
  const applicable = fixture.cases.filter(vector => vector.requirements.length === 0);
  assert.equal(applicable.length, 9);
  for (const vector of applicable) {
    const model = controller({ invoke: async command => command === 'list_capabilities' ? listed(vector) : command === 'open_capability' ? { outcome: 'opened', capability: detailFor(vector), alreadyApplied: false } : inspected(vector) }).model;
    await model.refresh(); await model.open(vector.capabilityId); model.selectProfile(vector.profile);
    assert.equal(await model.compile(), true, vector.name);
  }
});

test('failed same-input compile retains prior report while known mutation invalidates late work', async () => {
  let resolve;
  let count = 0;
  const { model } = controller({ invoke: (command) => {
    if (command === 'list_capabilities') return Promise.resolve(listed(candidateCase));
    if (command === 'open_capability') return Promise.resolve({ outcome: 'opened', capability: detailFor(candidateCase), alreadyApplied: false });
    if (count++ === 0) return Promise.resolve(inspected(candidateCase));
    if (count === 2) return Promise.resolve({ outcome: 'failed', error: { code: 'workspace_busy', message: 'Busy' } });
    return new Promise(done => { resolve = done; });
  } });
  await model.refresh(); await model.open(candidateCase.capabilityId); model.selectProfile('agents_md_v1');
  assert.equal(await model.compile(), true);
  const report = model.getState().report;
  assert.equal(await model.compile(), false);
  assert.deepEqual(model.getState().report, report);
  assert.match(model.getState().message, /Existing report still matches current selection/);
  const pending = model.compile();
  assert.equal(model.setTab('evidence'), true);
  model.invalidate(); resolve(inspected(candidateCase));
  assert.equal(await pending, false);
  assert.deepEqual(model.getState().report, report);
  assert.equal(model.getState().reportStale, true);
  assert.equal(model.getState().needsRefresh, true);
});

test('unreviewed and blocked fixtures preserve readiness; historical refresh preserves exact revision and stales report', async () => {
  const historical = detailFor(candidateCase);
  const successor = { ...historical.history[0], id: nextRevisionId, parentRevisionId: historical.revision.id, createdAtMs: historical.revision.createdAtMs + 1 };
  const old = detailFor(candidateCase, { latestRevisionId: nextRevisionId, history: [historical.history[0], successor] });
  const calls = [];
  let fail = false;
  const model = createCompilationController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'list_capabilities') return { outcome: 'listed', capabilities: [summaryFor(old)] };
    if (command === 'open_capability') return { outcome: 'opened', capability: old, alreadyApplied: false };
    return fail ? { outcome: 'failed', error: { code: 'workspace_busy', message: 'Busy' } } : inspected(candidateCase, nextRevisionId);
  } });
  await model.refresh(); await model.open(candidateCase.capabilityId, candidateCase.revision.id); model.selectProfile('agents_md_v1');
  assert.equal(await model.compile(), true);
  assert.equal(model.getState().report.observedCurrentHead, nextRevisionId);
  assert.equal(await model.refresh(), true);
  assert.equal(model.getState().selected.revision.id, candidateCase.revision.id);
  assert.equal(model.getState().reportStale, true);
  assert.deepEqual(calls.filter(([name]) => name === 'open_capability').at(-1), ['open_capability', { capabilityId: candidateCase.capabilityId, revisionId: candidateCase.revision.id }]);
  fail = true;
  assert.equal(await model.compile(), false);
  assert.match(model.getState().message, /Retained prior report is stale/);
  assert.equal(model.setTab('evidence'), true);
  fail = false;
  assert.equal(await model.compile(), true);
  assert.equal(model.getState().reportStale, false);
  assert.equal(model.getState().tab, 'text');
  for (const vector of [unreviewedCase, blockedCase]) {
    const local = controller({ invoke: async command => command === 'list_capabilities' ? listed(vector) : command === 'open_capability' ? { outcome: 'opened', capability: detailFor(vector), alreadyApplied: false } : inspected(vector) }).model;
    await local.refresh(); await local.open(vector.capabilityId); local.selectProfile(vector.profile);
    assert.equal(await local.compile(), true);
    assert.equal(local.getState().report.compilation.readiness, vector.expected.readiness);
  }
});

test('refresh clears a deleted selection without rerunning compilation', async () => {
  let present = true;
  let compiled = 0;
  const model = createCompilationController({ invoke: async command => {
    if (command === 'list_capabilities') return listed(candidateCase, { empty: !present });
    if (command === 'open_capability') return { outcome: 'opened', capability: detailFor(candidateCase), alreadyApplied: false };
    compiled += 1; return inspected(candidateCase);
  } });
  await model.refresh(); await model.open(candidateCase.capabilityId); model.selectProfile('agents_md_v1'); await model.compile();
  present = false;
  assert.equal(await model.refresh(), true);
  assert.equal(model.getState().selected, null);
  assert.equal(model.getState().report, null);
  assert.equal(compiled, 1);
});
