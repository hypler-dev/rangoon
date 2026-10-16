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
