import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createOperationsController, RECORD_KINDS } from '../preview/operations-model.mjs';
import { renderOperationsView } from '../preview/operations-view.mjs';

const fixture = JSON.parse(readFileSync(new URL('../fixtures/operations/synthetic-decision-view-v1.json', import.meta.url), 'utf8'));

test('Rust decision-window view enters renderer with independent denominators and no source disclosure', async () => {
  let calls = 0;
  const controller = createOperationsController({ invoke: async command => {
    assert.equal(command, 'select_and_inspect_operations');
    calls += 1;
    return { schemaVersion: 'rangoon.ops.local-inspection.v1', outcome: 'inspected', view: fixture };
  } });

  assert.equal(await controller.choose(), true);
  assert.equal(calls, 1);
  const state = controller.getState();
  assert.equal(state.status, 'inspected');
  assert.equal(state.view?.sourceCounts.eventCount, 24);
  assert.deepEqual(state.view?.events.slice(5, 8).map(event => event.reportedValue), ['allow', 'deny', 'unknown']);
  assert.deepEqual(state.view?.events.slice(8, 11).map(event => event.reportedValue), ['approve', 'deny', 'abstain']);
  assert.deepEqual(state.view?.events.slice(11, 16).map(event => event.reportedValue), ['reported_issued', 'reported_refused', 'reported_expired', 'reported_revoked', 'unknown']);
  assert.deepEqual(state.view?.events.slice(16).map(event => event.reportedValue), ['reported_accepted', 'reported_active', 'reported_completed', 'reported_failed', 'reported_cancelled', 'outcome_unknown', 'reported_reconciled_completed', 'reported_reconciled_failed']);
  assert.equal(state.view?.authority, 'none');
  assert.equal(state.view?.authenticity, 'unverified');
  assert.equal(state.view?.eligibility, 'unknown');
  assert.equal(state.view?.execution, 'unavailable');
  assert.equal(state.view?.outcome, 'unknown');
  assert.equal(state.view?.coverage, 'unknown');
  assert.equal(state.view?.structuralState, 'conflict');
  assert.ok(state.view.events.some(event => event.structuralDiagnostics.includes('event_identity_conflict')));
  assert.deepEqual(state.view?.decisionCounts, {
    evaluation: { allow: 1, deny: 1, unknown: 1 },
    approvalDecision: { approve: 1, deny: 1, abstain: 1 },
    authorization: { reportedIssued: 1, reportedRefused: 1, reportedExpired: 1, reportedRevoked: 1, unknown: 1 },
    attempt: { reportedAccepted: 1, reportedActive: 1, reportedCompleted: 1, reportedFailed: 1, reportedCancelled: 1, outcomeUnknown: 1, reportedReconciledCompleted: 1, reportedReconciledFailed: 1 },
  });

  const html = renderOperationsView(state, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(html, /3 observed evaluation records/);
  assert.match(html, /3 observed approval decision records/);
  assert.match(html, /5 observed authorization records/);
  assert.match(html, /8 observed attempt records/);
  assert.match(html, /1 reported unknown outcome/);
  assert.match(html, /Structural conflict reported/);
  assert.doesNotMatch(html, /synthetic-decision-window-sentinel|ops:0000000000000000000000000000000000000000000000000000000000000001|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|externalRef|canonicalDigest/);

  controller.setFilters({ kind: 'attempt', ordinal: '21' });
  assert.deepEqual(controller.getState().filteredEvents.map(event => event.inputOrdinal), [21]);
  controller.select(21);
  assert.match(renderOperationsView(controller.getState(), { recordKinds: RECORD_KINDS, codes: [] }), /data-operations-select="21" aria-pressed="true"/);
  assert.match(renderOperationsView(controller.getState(), { recordKinds: RECORD_KINDS, codes: [] }), /Outcome unknown/);

  const missingReference = fixture.events.find(event => event.references.some(reference => reference.status === 'missing'));
  assert.ok(missingReference, 'fixture exposes a missing relationship reference');
  controller.setFilters({ kind: 'all', ordinal: '' });
  controller.select(missingReference.inputOrdinal);
  assert.match(renderOperationsView(controller.getState(), { recordKinds: RECORD_KINDS, codes: [] }), /missing/);
  controller.clear();
  assert.equal(controller.getState().view, null);
});

test('same-sum decision bucket swap is atomically rejected for populated golden', async () => {
  const invalid = structuredClone(fixture);
  invalid.decisionCounts.attempt.outcomeUnknown = 0;
  invalid.decisionCounts.attempt.reportedCompleted = 2;
  const controller = createOperationsController({ invoke: async () => ({ schemaVersion: 'rangoon.ops.local-inspection.v1', outcome: 'inspected', view: invalid }) });

  assert.equal(await controller.choose(), false);
  assert.equal(controller.getState().view, null);
  assert.equal(controller.getState().errorCode, 'operations_response_invalid');
});

const localFixture = JSON.parse(readFileSync(new URL('../fixtures/operations/synthetic-operator-view-v1.json', import.meta.url), 'utf8'));

test('Rust populated operator view enters the renderer contract without source disclosure', async () => {
  let calls = 0;
  const controller = createOperationsController({ invoke: async command => {
    assert.equal(command, 'select_and_inspect_operations');
    calls += 1;
    return { schemaVersion: 'rangoon.ops.local-inspection.v1', outcome: 'inspected', view: localFixture };
  } });

  assert.equal(await controller.choose(), true);
  assert.equal(calls, 1);
  const state = controller.getState();
  assert.equal(state.status, 'inspected');
  assert.equal(state.view?.sourceCounts.eventCount, 2);
  assert.deepEqual(state.view?.events.map(event => event.recordKind), ['actor', 'evidence']);
  assert.equal(state.view?.authority, 'none');
  assert.equal(state.view?.authenticity, 'unverified');
  assert.equal(state.view?.execution, 'unavailable');

  const html = renderOperationsView(state, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(html, /Event #0/);
  assert.match(html, /Event #1/);
  assert.doesNotMatch(html, /synthetic-secret-200|externalRef|canonicalDigest/);

  controller.setFilters({ kind: 'evidence' });
  assert.deepEqual(controller.getState().filteredEvents.map(event => event.inputOrdinal), [1]);
  controller.select(1);
  assert.match(renderOperationsView(controller.getState(), { recordKinds: RECORD_KINDS, codes: [] }), /data-operations-select="1" aria-pressed="true"/);
  controller.clear();
  assert.equal(controller.getState().view, null);
});
