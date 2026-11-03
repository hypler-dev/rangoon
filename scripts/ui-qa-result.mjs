/** Keep executed assertions, teardown and open qualifications distinct. */
export function classifyUiQaResult({ checksPassed, cleanupComplete, pending, nativeSelectInput }) {
  if (checksPassed !== true || cleanupComplete !== true) return 'FAIL';
  if (!Array.isArray(pending) || pending.length || nativeSelectInput !== 'qualified_pointer_keyboard') return 'PARTIAL';
  return 'PASS';
}
