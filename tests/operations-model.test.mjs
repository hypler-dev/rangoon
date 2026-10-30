import test from 'node:test';
import assert from 'node:assert/strict';
import { RECORD_KINDS, createOperationsController } from '../preview/operations-model.mjs';

const deferred = () => { let resolve; return { promise: new Promise(done => { resolve = done; }), resolve }; };
const recordCounts = () => RECORD_KINDS.map(kind => ({ kind, count: kind === 'request' ? 1 : 0 }));
const decisionCounts = () => ({
  evaluation: { allow: 0, deny: 0, unknown: 0 },
  approvalDecision: { approve: 0, deny: 0, abstain: 0 },
  authorization: { reportedIssued: 0, reportedRefused: 0, reportedExpired: 0, reportedRevoked: 0, unknown: 0 },
  attempt: { reportedAccepted: 0, reportedActive: 0, reportedCompleted: 0, reportedFailed: 0, reportedCancelled: 0, outcomeUnknown: 0, reportedReconciledCompleted: 0, reportedReconciledFailed: 0 },
});
const view = () => ({
  schemaVersion: 'rangoon.ops.operator-view.v1', authority: 'none', authenticity: 'unverified', eligibility: 'unknown', execution: 'unavailable', outcome: 'unknown', coverage: 'unknown', structuralState: 'structural_unverified',
  sourceCounts: { eventCount: 1, producerCount: 1, topologyNodeCount: 1, topologyEdgeCount: 1, recordCounts: recordCounts() },
  diagnosticCounts: { structural: 0, relationships: 0, delegations: 0, approvals: 0, lifecycle: 0 }, decisionCounts: decisionCounts(),
  events: [{ inputOrdinal: 0, producerOrdinal: 0, fenceOrdinal: 0, sequence: 1, recordKind: 'request', reportedValue: null, structuralDiagnostics: [], issueStages: [], references: [{ referenceIndex: 0, status: 'unique_unverified', matchingEventOrdinals: [0] }] }],
  topology: { nodes: [{ inputNodeIndex: 0, kind: 'workflow', matchingEventOrdinals: [0], hasIssue: false }], edges: [{ inputEdgeIndex: 0, kind: 'request_flow', fromNodeIndices: [0], toNodeIndices: [0], hasIssue: false }] },
});
const inspected = value => ({ outcome: 'inspected', schemaVersion: 'rangoon.ops.local-inspection.v1', view: value });
const rejected = () => ({ outcome: 'rejected', schemaVersion: 'rangoon.ops.local-inspection.v1', error: { code: 'operations_window_invalid', message: 'This file is not a valid Rangoon operations window.' } });

test('unavailable bridge stays an explicit empty state', async () => {
  const model = createOperationsController();
  assert.equal(model.getState().status, 'unavailable');
  assert.equal(await model.choose(), false);
});

test('valid inspection retains one view and filters without a second reducer', async () => {
  const calls = [];
  const model = createOperationsController({ invoke: (...args) => { calls.push(args); return inspected(view()); } });
  assert.equal(await model.choose(), true);
  assert.deepEqual(calls, [['select_and_inspect_operations']]);
  assert.equal(model.getState().view.events.length, 1);
  model.setFilters({ kind: 'request', ordinal: '0' });
  assert.equal(model.getState().filteredEvents.length, 1);
  model.setFilters({ diagnostic: 'sequence_gap' });
  assert.equal(model.getState().filteredEvents.length, 0);
  model.select(0);
  assert.equal(model.getState().selectedOrdinal, 0);
});

test('malformed replacement is rejected atomically and prior view remains labelled', async () => {
  let calls = 0;
  const model = createOperationsController({ invoke: () => ++calls === 1 ? inspected(view()) : inspected({ ...view(), sourceCounts: { ...view().sourceCounts, recordCounts: [] } }) });
  await model.choose();
  assert.equal(await model.choose(), false);
  const state = model.getState();
  assert.equal(state.status, 'failed');
  assert.equal(state.view.events.length, 1);
  assert.equal(state.notice, 'Previous inspected window — replacement failed');
});

test('late result and workspace purge cannot restore inspected data', async () => {
  const waiting = deferred();
  const model = createOperationsController({ invoke: () => waiting.promise });
  const choosing = model.choose();
  model.purgeForWorkspaceLock();
  waiting.resolve(inspected(view()));
  assert.equal(await choosing, false);
  assert.equal(model.getState().view, null);
  assert.equal(model.getState().status, 'stale');
  assert.equal(model.getState().notice, 'Stale inspection result discarded. No local window selected.');
});

test('closed DTO rejects unknown keys, ids, and non-dense display indices', async () => {
  const malformed = view();
  malformed.events[0].sourceId = 'secret-like-source-id';
  const model = createOperationsController({ invoke: () => inspected(malformed) });
  await model.choose();
  assert.equal(model.getState().view, null);
  const gap = view(); gap.events[0].inputOrdinal = 1;
  const next = createOperationsController({ invoke: () => inspected(gap) });
  await next.choose();
  assert.equal(next.getState().view, null);
});

test('rejected replacement keeps prior view and fixed error result', async () => {
  let calls = 0;
  const model = createOperationsController({ invoke: () => ++calls === 1 ? inspected(view()) : rejected() });
  await model.choose();
  await model.choose();
  assert.equal(model.getState().status, 'rejected');
  assert.equal(model.getState().view.events[0].recordKind, 'request');
  assert.equal(model.getState().notice, 'Previous inspected window — replacement failed');
});

test('clear erases the view and does not describe a selected window', async () => {
  const model = createOperationsController({ invoke: () => inspected(view()) });
  await model.choose();
  model.clear();
  const state = model.getState();
  assert.equal(state.status, 'cleared');
  assert.equal(state.view, null);
  assert.equal(state.notice, 'Inspection cleared. No local window selected.');
});

test('decision counts reject a mismatched group denominator', async () => {
  const invalid = view(); invalid.decisionCounts.attempt.reportedCompleted = 1;
  const model = createOperationsController({ invoke: () => inspected(invalid) });
  await model.choose();
  assert.equal(model.getState().view, null);
});

test('decision counts reject a same-sum bucket swap against reported event values', async () => {
  const invalid = view();
  invalid.sourceCounts.recordCounts = RECORD_KINDS.map(kind => ({ kind, count: kind === 'evaluation' ? 1 : 0 }));
  invalid.events[0] = { ...invalid.events[0], recordKind: 'evaluation', reportedValue: 'allow' };
  invalid.decisionCounts.evaluation = { allow: 0, deny: 1, unknown: 0 };
  const model = createOperationsController({ invoke: () => inspected(invalid) });
  await model.choose();
  assert.equal(model.getState().view, null);
});

test('record counts reject every per-kind count mismatch', async () => {
  for (const kind of RECORD_KINDS) {
    const invalid = view();
    invalid.sourceCounts.recordCounts.find(row => row.kind === kind).count += 1;
    const model = createOperationsController({ invoke: () => inspected(invalid) });
    assert.equal(await model.choose(), false, `contradictory ${kind} count`);
    assert.equal(model.getState().view, null, `no partial ${kind} view`);
    assert.equal(model.getState().errorCode, 'operations_response_invalid');
  }
});

test('same-total record-kind substitution rejects replacement and retains prior view', async () => {
  const invalid = view();
  invalid.sourceCounts.recordCounts.find(row => row.kind === 'request').count = 0;
  invalid.sourceCounts.recordCounts.find(row => row.kind === 'actor').count = 1;
  let calls = 0;
  const model = createOperationsController({ invoke: () => inspected(++calls === 1 ? view() : invalid) });
  assert.equal(await model.choose(), true);
  model.select(0);
  assert.equal(await model.choose(), false);
  const state = model.getState();
  assert.equal(state.status, 'failed');
  assert.equal(state.errorCode, 'operations_response_invalid');
  assert.equal(state.notice, 'Previous inspected window — replacement failed');
  assert.equal(state.selectedOrdinal, 0);
  assert.equal(state.view.sourceCounts.recordCounts.find(row => row.kind === 'request').count, 1);
  assert.equal(state.view.sourceCounts.recordCounts.find(row => row.kind === 'actor').count, 0);
  assert.equal(state.view.events[0].recordKind, 'request');
});

test('zero-event view accepts zero record counts and rejects invented inventory', async () => {
  const empty = view();
  empty.sourceCounts = { eventCount: 0, producerCount: 0, topologyNodeCount: 0, topologyEdgeCount: 0, recordCounts: RECORD_KINDS.map(kind => ({ kind, count: 0 })) };
  empty.events = [];
  empty.topology = { nodes: [], edges: [] };
  const invented = structuredClone(empty);
  invented.sourceCounts.recordCounts.find(row => row.kind === 'evidence').count = 1;
  let calls = 0;
  const model = createOperationsController({ invoke: () => inspected(++calls === 1 ? empty : invented) });
  assert.equal(await model.choose(), true);
  assert.equal(model.getState().view.events.length, 0);
  assert.equal(await model.choose(), false);
  assert.equal(model.getState().view.sourceCounts.recordCounts.find(row => row.kind === 'evidence').count, 0);
});

test('an old completion never replaces a newer pending or inspected request', async () => {
  const first = deferred(); const second = deferred(); let calls = 0;
  const model = createOperationsController({ invoke: () => ++calls === 1 ? first.promise : second.promise });
  const old = model.choose();
  model.clear();
  const current = model.choose();
  first.resolve(inspected(view()));
  assert.equal(await old, false);
  assert.equal(model.getState().status, 'choosing');
  second.resolve(inspected(view()));
  assert.equal(await current, true);
  assert.equal(model.getState().status, 'inspected');
  assert.equal(model.getState().notice, 'Selected local window — unverified');
});

function populatedView(total) {
  const input = view();
  input.events = Array.from({ length: total }, (_, inputOrdinal) => ({ ...input.events[0], inputOrdinal, sequence: inputOrdinal + 1, references: [] }));
  input.sourceCounts = { eventCount: total, producerCount: total ? 1 : 0, topologyNodeCount: 0, topologyEdgeCount: 0, recordCounts: RECORD_KINDS.map(kind => ({ kind, count: kind === 'request' ? total : 0 })) };
  input.topology = { nodes: [], edges: [] };
  return input;
}

test('producer inventory may include prefix-only producers without inventing tail observations', async () => {
  for (const total of [0, 1]) {
    const input = populatedView(total); input.sourceCounts.producerCount = 2;
    const model = createOperationsController({ invoke: () => inspected(input) });
    assert.equal(await model.choose(), true);
    assert.equal(model.getState().view.sourceCounts.producerCount, 2);
    assert.equal(model.getState().view.events.length, total);
    assert.equal(model.getState().view.authority, 'none');
  }
  for (const count of [0, 33]) {
    const input = populatedView(1); input.sourceCounts.producerCount = count;
    const model = createOperationsController({ invoke: () => inspected(input) });
    assert.equal(await model.choose(), false, `invalid producer count ${count}`);
    assert.equal(model.getState().view, null);
  }
});

test('event pages bound display work and keep every record reachable without extra native calls', async () => {
  let calls = 0;
  const model = createOperationsController({ invoke: () => { calls++; return inspected(populatedView(1024)); } });
  await model.choose();
  assert.equal(model.getState().pageCount, 21);
  const seen = [];
  for (let page = 0; page < 21; page++) {
    model.setPage(page);
    const state = model.getState();
    assert.ok(state.pageEvents.length <= 50);
    seen.push(...state.pageEvents.map(event => event.inputOrdinal));
  }
  assert.deepEqual(seen, Array.from({ length: 1024 }, (_, ordinal) => ordinal));
  for (const invalid of [-1, 21, 0.5, NaN, Infinity, '0']) model.setPage(invalid);
  assert.equal(model.getState().page, 20);
  model.select(1023);
  model.setFilters({ ordinal: '0' });
  assert.equal(model.getState().page, 0);
  assert.equal(model.getState().selectedOrdinal, 1023);
  model.revealSelected();
  assert.equal(model.getState().page, 20);
  assert.equal(model.getState().ordinal, '');
  assert.ok(model.getState().pageEvents.some(event => event.inputOrdinal === 1023));
  model.resetFilters();
  assert.equal(model.getState().page, 0);
  assert.equal(model.getState().selectedOrdinal, 1023);
  assert.equal(calls, 1);
});

test('replacement success clears stale filters; cancel and rejection retain inspection and page', async () => {
  let result = inspected(populatedView(101));
  const model = createOperationsController({ invoke: () => result });
  await model.choose();
  model.setPage(2); model.select(100);
  result = { outcome: 'cancelled', schemaVersion: 'rangoon.ops.local-inspection.v1' };
  assert.equal(await model.choose(), false);
  assert.equal(model.getState().page, 2);
  assert.equal(model.getState().selectedOrdinal, 100);
  model.setFilters({ kind: 'attempt', diagnostic: 'sequence_gap', ordinal: '100' });
  result = rejected();
  assert.equal(await model.choose(), false);
  assert.equal(model.getState().kind, 'attempt');
  assert.equal(model.getState().diagnostic, 'sequence_gap');
  assert.equal(model.getState().ordinal, '100');
  assert.equal(model.getState().selectedOrdinal, 100);
  result = inspected(populatedView(1));
  assert.equal(await model.choose(), true);
  assert.equal(model.getState().kind, 'all');
  assert.equal(model.getState().diagnostic, 'all');
  assert.equal(model.getState().ordinal, '');
  assert.equal(model.getState().page, 0);
  assert.equal(model.getState().filteredEvents.length, 1);
});

test('clear and workspace lock purge filters, paging and retained inspection', async () => {
  const model = createOperationsController({ invoke: () => inspected(populatedView(101)) });
  for (const action of ['clear', 'purgeForWorkspaceLock']) {
    await model.choose();
    model.setFilters({ kind: 'request', diagnostic: 'sequence_gap', ordinal: '100' });
    model[action]();
    const state = model.getState();
    assert.equal(state.view, null);
    assert.equal(state.selectedOrdinal, null);
    assert.equal(state.kind, 'all'); assert.equal(state.diagnostic, 'all'); assert.equal(state.ordinal, '');
    assert.equal(state.page, 0); assert.deepEqual(state.pageEvents, []);
    model.revealSelected();
    assert.equal(model.getState().selectedOrdinal, null);
  }
});

test('unchanged and invalid local controls do not publish or repeat native work', async () => {
  let renders = 0; let calls = 0;
  const model = createOperationsController({ invoke: () => { calls++; return inspected(populatedView(1)); }, onChange: () => renders++ });
  await model.choose(); const before = renders;
  model.setFilters({ kind: 'all', diagnostic: 'all', ordinal: '' });
  model.setFilters({ kind: 'invalid', diagnostic: 'invalid', ordinal: '-1' });
  model.setMode('timeline'); model.setMode('invalid'); model.select(0); model.select(999);
  model.setPage(0); model.setPage(-1); model.resetFilters(); model.revealSelected();
  assert.equal(renders, before);
  assert.equal(calls, 1);
  model.setFilters({ ordinal: '999' });
  assert.equal(renders, before + 1);
  assert.equal(model.getState().filteredEvents.length, 0);
});
