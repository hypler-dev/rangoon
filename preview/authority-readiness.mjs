import { AUTHORITY_CATALOG, AUTHORITY_OPERATIONS } from './authority-catalog.mjs';

const freeze = value => {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
};

const operationDefinitions = Object.freeze([
  Object.freeze({ operation: 'configuration_preview', label: 'Configuration preview', actionLabel: 'Preview configuration', explanation: 'Would preview adapter configuration without applying it. No qualified adapter is implemented.' }),
  Object.freeze({ operation: 'connection_check', label: 'Connection check', actionLabel: 'Check connection', explanation: 'Would make a source-free explicit connectivity check; it is not approval. No qualified adapter is implemented.' }),
  Object.freeze({ operation: 'evidence_read', label: 'Evidence read', actionLabel: 'Read evidence', explanation: 'Would read bounded provider evidence; it is not authority. No qualified adapter is implemented.' }),
  Object.freeze({ operation: 'authorization_evaluation', label: 'Authorization evaluation', actionLabel: 'Evaluate authorization', explanation: 'Would request a provider policy decision; it is not a Rangoon grant. No qualified adapter is implemented.' }),
  Object.freeze({ operation: 'guardrail_evaluation', label: 'Guardrail evaluation', actionLabel: 'Evaluate guardrail', explanation: 'Would evaluate guardrails; it is not permission. No qualified adapter is implemented.' }),
]);

const stateRecords = freeze([
  { state: 'unavailable', label: 'Unavailable', tone: 'neutral', explanation: 'The adapter or required facility is unavailable for this function.', actionEnabled: false, authority: 'none', execution: 'unavailable' },
  { state: 'unsupported', label: 'Unsupported', tone: 'neutral', explanation: 'This operation or version is outside the selected adapter profile.', actionEnabled: false, authority: 'none', execution: 'unavailable' },
  { state: 'disconnected', label: 'Disconnected', tone: 'neutral', explanation: 'Configured transport is not connected for this function.', actionEnabled: false, authority: 'none', execution: 'unavailable' },
  { state: 'error', label: 'Inspection error', tone: 'danger', explanation: 'Inspection failed or returned malformed data.', actionEnabled: false, authority: 'none', execution: 'unavailable' },
  { state: 'inspected', label: 'Inspected read-only result', tone: 'informational', explanation: 'An inspected result is read-only; it is not connection, compatibility, approval, enforcement, or execution.', actionEnabled: false, authority: 'none', execution: 'unavailable' },
]);

const coverageKey = [
  { state: 'qualified', label: 'Fully mediated for this path', tone: 'positive', symbol: '●', explanation: 'Meaning: current qualification for an exact operation, artifact, configuration, and resource path.' },
  { state: 'partial', label: 'Partial', tone: 'warning', symbol: '◐', explanation: 'Meaning: partial mediation for a named scope.' },
  { state: 'advisory', label: 'Advisory', tone: 'informational', symbol: '○', explanation: 'Meaning: advisory observation without mediation.' },
  { state: 'unknown', label: 'Unknown / unavailable', tone: 'neutral', symbol: '◇', explanation: 'Meaning: coverage is unknown or unavailable.' },
  { state: 'blocked', label: 'Failed / blocked readiness', tone: 'danger', symbol: '×', explanation: 'Meaning: a separate readiness state is failed or blocked.' },
];

const controlKey = [
  { state: 'met', label: 'Met for assessed scope', tone: 'positive', symbol: '●', explanation: 'Meaning: control met for an assessed scope.' },
  { state: 'partial', label: 'Partial', tone: 'warning', symbol: '◐', explanation: 'Meaning: control partially met for an assessed scope.' },
  { state: 'unmet', label: 'Unmet', tone: 'danger', symbol: '×', explanation: 'Meaning: control is unmet for an assessed scope.' },
  { state: 'not_assessed', label: 'Not assessed / evidence unknown or stale', tone: 'neutral', symbol: '◇', explanation: 'Meaning: control evidence is not assessed, unknown, or stale.' },
  { state: 'not_applicable', label: 'Not applicable with scoped rationale', tone: 'neutral', symbol: '—', explanation: 'Meaning: a scoped rationale establishes not applicable.' },
];

export const AUTHORITY_COVERAGE_KEY = freeze(coverageKey);
export const AUTHORITY_CONTROL_KEY = freeze(controlKey);

const catalogIds = new Set(AUTHORITY_CATALOG.map(item => item.id));
const operations = Object.freeze(operationDefinitions.map((definition, index) => {
  if (definition.operation !== AUTHORITY_OPERATIONS[index]) throw new Error('authority readiness operation contract mismatch');
  return freeze({ ...definition, state: 'unavailable', reason: 'adapter_not_implemented', tone: 'neutral', disabled: true });
}));
const qualification = freeze({ state: 'unqualified', label: 'Unqualified', tone: 'neutral', explanation: 'No qualified adapter evidence exists.', evidence: null });
const coverage = freeze({ state: 'unknown', label: 'Coverage unknown', tone: 'neutral', explanation: 'No operation coverage evidence exists.', evidence: null });
const controlAssessment = freeze({ state: 'not_assessed', label: 'Control assessment not assessed', tone: 'neutral', explanation: 'No control assessment evidence exists.', evidence: null });
const readinessById = new Map(AUTHORITY_CATALOG.map(item => [item.id, freeze({
  catalogId: item.id,
  availableCount: 0,
  operationCount: 5,
  operations,
  qualification,
  coverage,
  controlAssessment,
  authority: 'none',
  execution: 'unavailable',
  networkAttempted: false,
})]));
const stateByName = new Map(stateRecords.map(record => [record.state, record]));

export function getAuthorityReadiness(id) {
  return typeof id === 'string' && catalogIds.has(id) ? readinessById.get(id) : null;
}

export function describeAuthorityState(state) {
  return typeof state === 'string' ? stateByName.get(state) ?? null : null;
}
