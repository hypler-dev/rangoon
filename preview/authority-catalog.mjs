// Built-in reference metadata. This is not provider discovery or an adapter.
export const AUTHORITY_CATALOG_SCHEMA = 'rangoon.authority-catalog.v1';
export const AUTHORITY_OPERATIONS = Object.freeze([
  'configuration_preview',
  'connection_check',
  'evidence_read',
  'authorization_evaluation',
  'guardrail_evaluation',
]);

export const AUTHORITY_AREAS = Object.freeze([
  Object.freeze({ id: 'analysis', label: 'Import & Analyze' }),
  Object.freeze({ id: 'decompose', label: 'Decompose' }),
  Object.freeze({ id: 'merge', label: 'Merge' }),
  Object.freeze({ id: 'split', label: 'Split' }),
  Object.freeze({ id: 'skills', label: 'Skills' }),
  Object.freeze({ id: 'workflows', label: 'Workflows' }),
  Object.freeze({ id: 'compile', label: 'Compile' }),
  Object.freeze({ id: 'workspace', label: 'Data' }),
  Object.freeze({ id: 'model-assistance', label: 'Model assistance' }),
  Object.freeze({ id: 'engine', label: 'LNSAT diagnostic' }),
]);

const categories = new Set(['all', 'gateway', 'authorization', 'guardrail', 'action-authority']);
const areaIds = new Set(AUTHORITY_AREAS.map(area => area.id));
const unavailableCapabilities = () => AUTHORITY_OPERATIONS.map(operation => ({ operation, state: 'unavailable' }));
const entry = (details) => ({
  ...details,
  state: 'unavailable',
  reason: 'adapter_not_implemented',
  observedVersion: null,
  capabilities: unavailableCapabilities(),
  compatibility: 'unknown',
  connection: 'unknown',
  runtime: 'unknown',
  authority: 'none',
  networkAttempted: false,
});
const freeze = value => {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
};

export const AUTHORITY_CATALOG = freeze([
  entry({
    id: 'agentgateway', productTarget: 'agentgateway standalone', family: 'agentgateway', category: 'gateway', badge: 'Gateway', logoPath: null,
    summary: 'Research reference for a standalone gateway configuration and mediated access-log inspection.',
    abilities: [{ label: 'Configuration preview', operation: 'configuration_preview' }, { label: 'MCP authorization mapping', operation: 'authorization_evaluation' }, { label: 'Mediated access-log evidence', operation: 'evidence_read' }],
    limitations: 'Gateway authorization applies only to traffic it mediates. Kubernetes is a separate future target; no Rangoon profile or connection exists.',
    references: [{ label: 'Standalone configuration', url: 'https://agentgateway.dev/docs/standalone/latest/documentation/configuration/' }, { label: 'MCP authorization', url: 'https://agentgateway.dev/docs/standalone/latest/documentation/mcp/mcp-authz/' }, { label: 'Access logs', url: 'https://agentgateway.dev/docs/standalone/latest/documentation/observability/access-logs/' }],
    areas: [{ id: 'decompose', relationship: 'Local transformation needs no authority engine.' }, { id: 'merge', relationship: 'Local transformation needs no authority engine.' }, { id: 'split', relationship: 'Local transformation needs no authority engine.' }, { id: 'workflows', relationship: 'Design reference for mediated workflow traffic.' }, { id: 'compile', relationship: 'Design reference for configuration preview output.' }],
  }),
  entry({
    id: 'cerbos', productTarget: 'Cerbos PDP', family: 'cerbos', category: 'authorization', badge: 'Authorization', logoPath: null,
    summary: 'Research reference for policy configuration and explicit authorization-decision inspection.',
    abilities: [{ label: 'Policy configuration preview', operation: 'configuration_preview' }, { label: 'Resource decision inspection', operation: 'authorization_evaluation' }],
    limitations: 'CheckResources evaluates requested resource/actions. Enforcement remains with the caller; a decision is not an approval, grant consumption, or execution receipt.',
    references: [{ label: 'Cerbos API', url: 'https://docs.cerbos.dev/cerbos/latest/api/index.html' }],
    areas: [{ id: 'skills', relationship: 'Local review is separate from a provider decision.' }, { id: 'workflows', relationship: 'Design reference for explicit decision boundaries.' }, { id: 'engine', relationship: 'Contrast with the separate LNSAT diagnostic.' }],
  }),
  entry({
    id: 'permit', productTarget: 'Permit authorization', family: 'permit', category: 'authorization', badge: 'Authorization', logoPath: null,
    summary: 'Research reference for identity, resource, tenant, and policy decision evidence.',
    abilities: [{ label: 'Configuration preview', operation: 'configuration_preview' }, { label: 'Decision evidence inspection', operation: 'authorization_evaluation' }],
    limitations: 'Permit identity, resource, tenant, and policy semantics remain provider-specific. Account, PDP, credential, and retention qualification are open; decisions are not Rangoon approvals.',
    references: [{ label: 'Permit authorization documentation', url: 'https://docs.permit.io/' }],
    areas: [{ id: 'skills', relationship: 'Local review is separate from a provider decision.' }, { id: 'workflows', relationship: 'Design reference for resource-aware policy decisions.' }, { id: 'workspace', relationship: 'Design reference only; no saved assignment exists.' }],
  }),
  entry({
    id: 'permit-fastmcp', productTarget: 'Permit FastMCP middleware', family: 'permit', category: 'authorization', badge: 'Middleware', logoPath: null,
    summary: 'Research reference for server-side permission mapping in an owned FastMCP server.',
    abilities: [{ label: 'Permission-mapping preview', operation: 'configuration_preview' }, { label: 'Compatible evaluation evidence', operation: 'authorization_evaluation' }],
    limitations: 'This middleware runs inside an owned FastMCP server with its own request and identity mapping. It is not the commercial Permit MCP Gateway.',
    references: [{ label: 'Maintainer README', url: 'https://github.com/permitio/permit-fastmcp/blob/main/README.md' }],
    areas: [{ id: 'workflows', relationship: 'Design reference for owned-server middleware placement.' }, { id: 'model-assistance', relationship: 'Design reference only; no model route is bound.' }],
  }),
  entry({
    id: 'permit-mcp-gateway', productTarget: 'Permit MCP Gateway', family: 'permit', category: 'gateway', badge: 'Gateway', logoPath: null,
    summary: 'Research reference for commercial gateway configuration and decision/audit evidence.',
    abilities: [{ label: 'Gateway configuration preview', operation: 'configuration_preview' }, { label: 'Decision and audit evidence', operation: 'evidence_read' }],
    limitations: 'This is a separate commercial product with separate connection, authentication, entitlement, and retention treatment. Middleware availability does not confer gateway access.',
    references: [{ label: 'Permit MCP Gateway guide', url: 'https://docs.permit.io/permit-mcp-gateway/guide/' }],
    areas: [{ id: 'workflows', relationship: 'Design reference for mediated gateway placement.' }, { id: 'compile', relationship: 'Design reference for an inert configuration preview.' }],
  }),
  entry({
    id: 'invariant-local', productTarget: 'Invariant local', family: 'invariant', category: 'guardrail', badge: 'Local guardrail', logoPath: null,
    summary: 'Research reference for a pinned local policy preview and locally produced violation reports.',
    abilities: [{ label: 'Policy configuration preview', operation: 'configuration_preview' }, { label: 'Violation report inspection', operation: 'guardrail_evaluation' }],
    limitations: 'A future adapter must pin LocalPolicy, detectors, and dependencies. Imported policy is not trusted code; local execution, isolation, and detector egress remain unqualified. No violations is not authorization.',
    references: [{ label: 'Maintainer repository', url: 'https://github.com/invariantlabs-ai/invariant' }],
    areas: [{ id: 'analysis', relationship: 'Design reference for local content inspection.' }, { id: 'model-assistance', relationship: 'Design reference for separate guardrail reporting.' }],
  }),
  entry({
    id: 'invariant-remote', productTarget: 'Invariant remote', family: 'invariant', category: 'guardrail', badge: 'Remote guardrail', logoPath: null,
    summary: 'Research reference for explicitly consented policy and trace evaluation result inspection.',
    abilities: [{ label: 'Consented remote policy evaluation', operation: 'guardrail_evaluation' }, { label: 'Result evidence inspection', operation: 'evidence_read' }],
    limitations: 'Remote treatment has distinct payload, endpoint, credential, and retention handling. There is no automatic local-to-remote fallback; remote compatibility remains unqualified.',
    references: [{ label: 'Policy selection source', url: 'https://raw.githubusercontent.com/invariantlabs-ai/invariant/main/invariant/analyzer/policy.py' }, { label: 'Remote request source', url: 'https://raw.githubusercontent.com/invariantlabs-ai/invariant/main/invariant/analyzer/remote_policy.py' }],
    areas: [{ id: 'model-assistance', relationship: 'Design reference for explicit remote consent.' }, { id: 'analysis', relationship: 'Design reference only; no source content is sent.' }],
  }),
  entry({
    id: 'lnsat', productTarget: 'LNSAT', family: 'lnsat', category: 'action-authority', badge: 'Action authority', logoPath: 'assets/authority/lnsat-mark.webp',
    summary: 'Optional action-authority module: exact requests, human approval, one-time consumption, consequence receipts and recovery. Public 0.1.0 source reference checked October 11; no Rangoon adapter.',
    abilities: [{ label: 'Headless configuration reference', operation: 'configuration_preview' }, { label: 'Exact request and policy binding', operation: 'authorization_evaluation' }, { label: 'Authenticated human approval records', operation: 'evidence_read' }, { label: 'Authorization consumption and attempt records', operation: 'evidence_read' }, { label: 'Receipts and unknown-outcome recovery', operation: 'evidence_read' }],
    limitations: 'This catalog preserves the I1 operations only. LNSAT has five separate A1 diagnostic seams; all remain unavailable. Pre-release source and experimental bounded Git proof are not installed Rangoon support or real Docker qualification. Monitoring snapshot/watch are proposed; no live feed exists here.',
    references: [{ label: 'October 11 pinned source overview', url: 'https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/README.md' }, { label: 'Pinned implementation and qualification status', url: 'https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/docs/PROJECT_STATUS.md' }],
    areas: [{ id: 'skills', relationship: 'Local review is separate from action authority.' }, { id: 'engine', relationship: 'Open the separate local A1 diagnostic.' }, { id: 'workflows', relationship: 'Design reference for qualified action authority.' }, { id: 'compile', relationship: 'Design reference for evidence-bound exports.' }],
  }),
]);

const catalogIds = new Set(AUTHORITY_CATALOG.map(item => item.id));
const normalQuery = value => String(value ?? '').slice(0, 160);
export const authorityRouteFor = (id, area = null) => catalogIds.has(id) && (!area || areaIds.has(area))
  ? `#authority-engines/${id}${area ? `/from/${area}` : ''}` : '#authority-engines';

export function parseAuthorityHash(hash) {
  const value = typeof hash === 'string' ? hash : '';
  if (!value.startsWith('#authority-engines')) return { matched: false, valid: false, id: null, area: null };
  if (value.length > 256 || value === '#authority-engines') return { matched: true, valid: value === '#authority-engines', id: null, area: null };
  const match = /^#authority-engines\/([a-z][a-z0-9-]*)(?:\/from\/([a-z][a-z0-9-]*))?$/.exec(value);
  if (!match || !catalogIds.has(match[1]) || (match[2] && !areaIds.has(match[2]))) return { matched: true, valid: false, id: null, area: null };
  return { matched: true, valid: true, id: match[1], area: match[2] ?? null };
}

function projectedState(state) {
  const query = state.query.toLocaleLowerCase();
  const entries = AUTHORITY_CATALOG.filter(item => (state.category === 'all' || item.category === state.category) && (!query || [item.id, item.productTarget, ...item.abilities.map(ability => `${ability.label} ${ability.operation}`)].join(' ').toLocaleLowerCase().includes(query)));
  const selected = state.selectedId ? AUTHORITY_CATALOG.find(item => item.id === state.selectedId) : null;
  const selectedVisible = Boolean(selected && entries.some(item => item.id === selected.id));
  return Object.freeze({ ...state, entries: Object.freeze(entries), selected: selectedVisible ? selected : null, selectedVisible, inspector: state.invalidRoute ? 'unavailable' : state.selectedId && !selectedVisible ? 'filtered-empty' : selected ? 'selected' : 'base' });
}

export function projectAuthorityState(value = {}) {
  return projectedState({
    query: normalQuery(value.query),
    category: categories.has(value.category) ? value.category : 'all',
    selectedId: catalogIds.has(value.selectedId) ? value.selectedId : null,
    originArea: areaIds.has(value.originArea) ? value.originArea : null,
    invalidRoute: value.invalidRoute === true,
  });
}

export function createAuthorityController({ onChange = () => {} } = {}) {
  let state = { query: '', category: 'all', selectedId: null, originArea: null, invalidRoute: false };
  const publish = () => onChange(projectedState(state));
  const update = patch => { state = { ...state, ...patch }; publish(); };
  return {
    getState: () => projectAuthorityState(state),
    setRoute(hash) {
      const parsed = parseAuthorityHash(hash);
      if (!parsed.matched) return false;
      update(parsed.valid ? { selectedId: parsed.id, originArea: parsed.area, invalidRoute: false } : { selectedId: null, originArea: null, invalidRoute: true });
      return true;
    },
    setQuery(query) { update({ query: normalQuery(query) }); },
    setCategory(category) { if (categories.has(category)) update({ category }); },
    clearFilters() { update({ query: '', category: 'all' }); },
  };
}
