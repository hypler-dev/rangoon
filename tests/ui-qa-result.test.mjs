import test from 'node:test';
import assert from 'node:assert/strict';
import { classifyUiQaResult } from '../scripts/ui-qa-result.mjs';

const complete = { checksPassed: true, cleanupComplete: true, pending: [], nativeSelectInput: 'qualified_pointer_keyboard' };
test('a qualified select cannot hide an unresolved Lock pointer check', () => {
  assert.equal(classifyUiQaResult({ ...complete, pending: ['lock_pointer_confirmation'] }), 'PARTIAL');
  assert.equal(classifyUiQaResult(complete), 'PASS');
});
test('failed checks or incomplete cleanup never become partial success', () => {
  assert.equal(classifyUiQaResult({ ...complete, checksPassed: false, pending: ['native_select_pointer_keyboard'] }), 'FAIL');
  assert.equal(classifyUiQaResult({ ...complete, cleanupComplete: false }), 'FAIL');
});
test('missing or unqualified evidence remains partial', () => {
  for (const value of [{ ...complete, pending: undefined }, { ...complete, pending: null }, { ...complete, nativeSelectInput: undefined }, { ...complete, nativeSelectInput: 'unqualified_headless_macos' }]) {
    assert.equal(classifyUiQaResult(value), 'PARTIAL');
  }
});
