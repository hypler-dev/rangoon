import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createOperationsController, RECORD_KINDS } from '../preview/operations-model.mjs';
import { renderOperationsView } from '../preview/operations-view.mjs';

const fixture = JSON.parse(readFileSync(new URL('../fixtures/operations/synthetic-operator-view-v1.json', import.meta.url), 'utf8'));

test('Rust populated operator view enters the renderer contract without source disclosure', async () => {
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
