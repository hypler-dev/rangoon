import test from 'node:test';
import assert from 'node:assert/strict';
import { canCompile, filterItems, workflowOutcome } from '../preview/model.mjs';

test('only semantic conflicts or loss block inert draft compilation', () => {
  assert.equal(canCompile({ conflicts: 1, authority: true, scenario: 'ready' }), false);
  assert.equal(canCompile({ conflicts: 0, semanticLoss: true }), false);
  assert.equal(canCompile({ conflicts: 0, authority: false }), true);
});

test('unknown workflow outcome never retries', () => {
  assert.deepEqual(workflowOutcome('unknown'), { retry: false, label: 'Unknown outcome. Do not retry.' });
});

test('skill filtering handles case, whitespace, missing fields and no results', () => {
  const skills = [{ name: 'Security Review', family: 'Security' }, { name: 'Documentation' }];
  assert.deepEqual(filterItems(skills, ' SECURITY '), [skills[0]]);
  assert.deepEqual(filterItems(skills, 'unmatched'), []);
  assert.deepEqual(filterItems(skills, ''), skills);
});
