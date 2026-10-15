import test from 'node:test';
import assert from 'node:assert/strict';
import { AUTHORITY_CATALOG, AUTHORITY_OPERATIONS } from '../preview/authority-catalog.mjs';
import { AUTHORITY_CONTROL_KEY, AUTHORITY_COVERAGE_KEY, describeAuthorityState, getAuthorityReadiness } from '../preview/authority-readiness.mjs';

const ids = ['agentgateway', 'cerbos', 'permit', 'permit-fastmcp', 'permit-mcp-gateway', 'invariant-local', 'invariant-remote', 'lnsat'];
const operationLabels = ['Configuration preview', 'Connection check', 'Evidence read', 'Authorization evaluation', 'Guardrail evaluation'];
const actionLabels = ['Preview configuration', 'Check connection', 'Read evidence', 'Evaluate authorization', 'Evaluate guardrail'];
const operationIntent = ['preview adapter configuration without applying it', 'source-free explicit connectivity check; it is not approval', 'read bounded provider evidence; it is not authority', 'request a provider policy decision; it is not a Rangoon grant', 'evaluate guardrails; it is not permission'];

test('every immutable catalog id projects the exact five unavailable adapter operations', () => {
  assert.deepEqual(AUTHORITY_CATALOG.map(item => item.id), ids);
  for (const id of ids) {
    const readiness = getAuthorityReadiness(id);
    assert.deepEqual(Object.keys(readiness), ['catalogId', 'availableCount', 'operationCount', 'operations', 'qualification', 'coverage', 'controlAssessment', 'authority', 'execution', 'networkAttempted']);
    assert.equal(readiness.catalogId, id); assert.equal(readiness.availableCount, 0); assert.equal(readiness.operationCount, 5);
    assert.equal(readiness.authority, 'none'); assert.equal(readiness.execution, 'unavailable'); assert.equal(readiness.networkAttempted, false);
    assert.deepEqual(readiness.operations.map(item => item.operation), AUTHORITY_OPERATIONS);
    assert.deepEqual(readiness.operations.map(item => item.label), operationLabels);
    assert.deepEqual(readiness.operations.map(item => item.actionLabel), actionLabels);
    for (const [index, operation] of readiness.operations.entries()) {
      assert.deepEqual(Object.keys(operation), ['operation', 'label', 'actionLabel', 'explanation', 'state', 'reason', 'tone', 'disabled']);
      assert.equal(operation.state, 'unavailable'); assert.equal(operation.reason, 'adapter_not_implemented'); assert.equal(operation.tone, 'neutral'); assert.equal(operation.disabled, true);
      assert.match(operation.explanation, new RegExp(operationIntent[index]));
      assert.match(operation.explanation, /No qualified adapter is implemented/);
      assert.ok(Object.isFrozen(operation));
    }
    for (const [name, state, label] of [['qualification', 'unqualified', 'Unqualified'], ['coverage', 'unknown', 'Coverage unknown'], ['controlAssessment', 'not_assessed', 'Control assessment not assessed']]) {
      assert.equal(readiness[name].state, state); assert.equal(readiness[name].label, label); assert.equal(readiness[name].tone, 'neutral'); assert.equal(readiness[name].evidence, null); assert.ok(Object.isFrozen(readiness[name]));
    }
    assert.ok(Object.isFrozen(readiness)); assert.ok(Object.isFrozen(readiness.operations));
    assert.throws(() => { readiness.operations[0].disabled = false; }, TypeError);
  }
});

test('state descriptions accept only exact primitive I1 states and never enable action or authority', () => {
  const expected = [
    ['unavailable', 'neutral', 'The adapter or required facility is unavailable for this function.'],
    ['unsupported', 'neutral', 'This operation or version is outside the selected adapter profile.'],
    ['disconnected', 'neutral', 'Configured transport is not connected for this function.'],
    ['error', 'danger', 'Inspection failed or returned malformed data.'],
    ['inspected', 'informational', 'An inspected result is read-only; it is not connection, compatibility, approval, enforcement, or execution.'],
  ];
  for (const [state, tone, explanation] of expected) {
    const record = describeAuthorityState(state);
    assert.equal(record.state, state); assert.equal(record.tone, tone); assert.equal(record.explanation, explanation); assert.equal(record.actionEnabled, false); assert.equal(record.authority, 'none'); assert.equal(record.execution, 'unavailable'); assert.ok(Object.isFrozen(record));
  }
  for (const value of ['allow', 'deny', 'approved', 'succeeded', 'available', 'UNAVAILABLE', ' unavailable', 'unavailable ', '__proto__', 'constructor', { toString: () => 'unavailable' }, ['unavailable'], null, undefined, 1]) assert.equal(describeAuthorityState(value), null);
  for (const value of ['missing', 'lnsat ', 'LNSAT', '__proto__', 'constructor', { toString: () => 'lnsat' }, ['lnsat'], null, undefined, 1]) assert.equal(getAuthorityReadiness(value), null);
});

test('frozen status keys are labeled meanings while production readiness stays neutral', () => {
  assert.deepEqual(AUTHORITY_COVERAGE_KEY.map(item => item.state), ['qualified', 'partial', 'advisory', 'unknown', 'blocked']);
  assert.deepEqual(AUTHORITY_CONTROL_KEY.map(item => item.state), ['met', 'partial', 'unmet', 'not_assessed', 'not_applicable']);
  assert.equal(AUTHORITY_COVERAGE_KEY[0].label, 'Fully mediated for this path');
  assert.match(AUTHORITY_COVERAGE_KEY[0].explanation, /exact operation, artifact, configuration, and resource path/);
  assert.doesNotMatch(AUTHORITY_COVERAGE_KEY[0].explanation, /assessed scope/i);
  for (const key of [AUTHORITY_COVERAGE_KEY, AUTHORITY_CONTROL_KEY]) {
    assert.ok(Object.isFrozen(key)); assert.ok(Object.isFrozen(key[0]));
    assert.throws(() => { key[0].tone = 'positive'; }, TypeError);
  }
  for (const id of ids) {
    const readiness = getAuthorityReadiness(id);
    assert.equal(readiness.qualification.tone, 'neutral'); assert.equal(readiness.coverage.tone, 'neutral'); assert.equal(readiness.controlAssessment.tone, 'neutral');
    assert.equal(readiness.operations.every(operation => operation.tone === 'neutral'), true);
  }
});
