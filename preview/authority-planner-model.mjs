import { AUTHORITY_CATALOG } from './authority-catalog.mjs';
const freeze = value => { if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.values(value).forEach(freeze); Object.freeze(value); } return value; };
export const AUTHORITY_PLANNER_ROLES = freeze([
  { id: 'gateway', label: 'Gateway', description: 'Checks routed traffic.', targets: ['agentgateway', 'permit-mcp-gateway'] },
  { id: 'access-decision', label: 'Access decision', description: 'Answers who may do what.', targets: ['cerbos', 'permit'] },
  { id: 'server-middleware', label: 'Server middleware', description: 'Checks requests inside an owned server.', targets: ['permit-fastmcp'] },
  { id: 'behavior-guardrail', label: 'Behavior guardrail', description: 'Checks content and behavior patterns.', targets: ['invariant-local', 'invariant-remote'] },
  { id: 'action-authority', label: 'Action authority', description: 'Targets exact action and outcome evidence.', targets: ['lnsat'] },
]);
const items = new Map(AUTHORITY_CATALOG.map(item => [item.id, item]));
const roles = new Map(AUTHORITY_PLANNER_ROLES.map(role => [role.id, role]));
export const getAuthorityRole = id => AUTHORITY_PLANNER_ROLES.find(role => role.targets.includes(id)) ?? null;
export const getAuthorityAlternatives = id => freeze((getAuthorityRole(id)?.targets ?? []).filter(target => target !== id));
const keyFor = (left, right) => [left, right].sort().join('|');
const overrides = new Map([
  [keyFor('permit', 'permit-fastmcp'), { kind: 'documented_dependency', label: 'Documented provider relationship', basis: 'provider_documentation', rationale: 'The middleware asks a configured Permit PDP for authorization. They have different roles; one does not replace the other.', requirements: 'Qualify PDP/version, identity and resource mapping, credentials, privacy and refusal behavior. No Rangoon adapter is implemented.', reference: { label: 'Permit middleware mapping', url: 'https://github.com/permitio/permit-fastmcp/blob/main/README.md' } }],
  [keyFor('permit', 'permit-mcp-gateway'), { kind: 'documented_dependency', label: 'Documented provider relationship', basis: 'provider_documentation', rationale: 'The gateway asks Permit for tool-call decisions and uses Permit policy objects. They are parts of a provider architecture, not independent replacements.', requirements: 'Qualify gateway/PDP/version, identity and environment mapping, consent, retention and refusal behavior. No Rangoon adapter is implemented.', reference: { label: 'Gateway and Permit responsibilities', url: 'https://docs.permit.io/permit-mcp-gateway/permit-integration/' } }],
  [keyFor('cerbos', 'permit-fastmcp'), { kind: 'not_assessed', label: 'Not assessed', basis: 'not_assessed', rationale: 'This middleware is designed for Permit. Substituting Cerbos as its decision backend is not established here.', requirements: 'A separately reviewed adapter and policy/identity mapping would be required; do not assume backend substitution.' }],
  [keyFor('cerbos', 'permit-mcp-gateway'), { kind: 'not_assessed', label: 'Not assessed', basis: 'not_assessed', rationale: 'This gateway documents Permit-backed decisions. Using Cerbos instead is not established here.', requirements: 'A separately reviewed integration and policy/identity mapping would be required; do not assume backend substitution.' }],
]);
function relationshipFor(left, right) {
  const leftRole = getAuthorityRole(left.id), rightRole = getAuthorityRole(right.id);
  let detail = overrides.get(keyFor(left.id, right.id));
  if (!detail && leftRole.id === rightRole.id) detail = {
    kind: 'alternative', label: 'Same-role alternative · migration required', basis: 'architectural_inference',
    rationale: `Both address the ${leftRole.label.toLowerCase()} responsibility. This is role overlap, not interchangeable APIs, policies, identities or evidence.`,
    requirements: leftRole.id === 'behavior-guardrail' ? 'Local and remote modes have distinct egress/consent, detector, endpoint and retention requirements. Never switch automatically.' : 'Compare exact policy semantics, supported operations, identity/context, deployment and evidence. Migration and conformance tests are required.',
  };
  if (!detail && [leftRole.id, rightRole.id].includes('gateway') && [leftRole.id, rightRole.id].includes('server-middleware')) detail = {
    kind: 'overlap', label: 'Overlapping checkpoints', basis: 'architectural_inference',
    rationale: 'Gateway and owned-server middleware can check different boundaries. This plan establishes neither the traffic path nor combined enforcement.',
    requirements: 'Review duplicated decisions, precedence, identity/context consistency, denial propagation, bypasses and privacy before layering.',
  };
  if (!detail) detail = {
    kind: 'complement', label: 'Potential complement · unqualified', basis: 'architectural_inference',
    rationale: `${leftRole.label} and ${rightRole.label.toLowerCase()} describe different responsibilities that may be planned together. No product integration or control order is established.`,
    requirements: 'Qualify adapters/versions, actual traffic coverage, trusted identity/context, denial propagation, consent, revocation and evidence. Every required execution-time check must still apply.',
  };
  const references = detail.reference ? [detail.reference] : [...left.references.slice(0, 1), ...right.references.slice(0, 1)];
  return freeze({ ids: [left.id, right.id], kind: detail.kind, label: detail.label, basis: detail.basis, rationale: detail.rationale, requirements: detail.requirements, references, compatibility: 'unqualified', dropInReplacement: false, authority: 'none', networkAttempted: false });
}
export const AUTHORITY_RELATIONSHIPS = freeze(AUTHORITY_CATALOG.flatMap((left, index) => AUTHORITY_CATALOG.slice(index + 1).map(right => relationshipFor(left, right))));
const relationships = new Map(AUTHORITY_RELATIONSHIPS.map(relation => [keyFor(...relation.ids), relation]));
export const getAuthorityRelationship = (left, right) => items.has(left) && items.has(right) && left !== right ? relationships.get(keyFor(left, right)) ?? null : null;
export function projectAuthorityPlanner(raw = {}) {
  const value = raw && typeof raw === 'object' ? raw : {};
  const assignments = Object.fromEntries(AUTHORITY_PLANNER_ROLES.map(role => [role.id, role.targets.includes(value.assignments?.[role.id]) ? value.assignments[role.id] : null]));
  const pair = [0, 1].map(index => items.has(value.pair?.[index]) ? value.pair[index] : null);
  if (pair[0] === pair[1]) pair[1] = null;
  const selected = Object.values(assignments).filter(Boolean);
  return freeze({ revision: Number.isSafeInteger(value.revision) && value.revision >= 0 ? value.revision : 0, assignments, pair, relationship: getAuthorityRelationship(...pair), relationships: selected.flatMap((left, index) => selected.slice(index + 1).map(right => getAuthorityRelationship(left, right))), message: typeof value.message === 'string' ? value.message.slice(0, 240) : 'Local plan only. No provider connected.', authority: 'none', execution: 'unavailable', networkAttempted: false, persistence: 'memory_only' });
}
export function createAuthorityPlanner() {
  let state = projectAuthorityPlanner();
  const listeners = new Set();
  const replacements = new WeakMap();
  const commit = (id, role) => { update({ assignments: { ...state.assignments, [role]: id }, revision: state.revision + 1, message: `${items.get(id).productTarget} planned for ${roles.get(role).label}. Local plan only; no providers changed.` }); return true; };
  const update = patch => { state = projectAuthorityPlanner({ ...state, ...patch }); for (const listener of listeners) listener(state); };
  const reject = () => { update({ message: 'Plan unchanged. Choose a known target for its matching role.' }); return false; };
  return {
    getState: () => state,
    subscribe(listener) { if (typeof listener !== 'function') return () => {}; listeners.add(listener); return () => listeners.delete(listener); },
    choose(id, role = getAuthorityRole(id)?.id) {
      if (!roles.has(role) || !roles.get(role).targets.includes(id)) return reject();
      if (state.assignments[role] === id) return false;
      if (state.assignments[role]) return reject();
      return commit(id, role);
    },
    requestReplacement(id, role = getAuthorityRole(id)?.id) {
      if (!roles.has(role) || !roles.get(role).targets.includes(id) || !state.assignments[role] || state.assignments[role] === id) return null;
      const token = freeze({ role, id, occupant: state.assignments[role], revision: state.revision });
      replacements.set(token, token);
      return token;
    },
    confirmReplacement(token) {
      const binding = token && typeof token === 'object' ? replacements.get(token) : null;
      if (!binding) return reject();
      replacements.delete(token);
      if (state.revision !== binding.revision || state.assignments[binding.role] !== binding.occupant) return reject();
      return commit(binding.id, binding.role);
    },
    cancelReplacement(token) { return token && typeof token === 'object' ? replacements.delete(token) : false; },
    remove(role) {
      if (!roles.has(role)) return reject();
      if (!state.assignments[role]) return false;
      update({ assignments: { ...state.assignments, [role]: null }, revision: state.revision + 1, message: `${roles.get(role).label} cleared from the local plan.` }); return true;
    },
    clear() {
      update({ assignments: {}, pair: [null, null], revision: state.revision + 1, message: 'Local plan cleared. No providers changed.' });
    },
    setPair(side, id) {
      const index = side === 'left' ? 0 : side === 'right' ? 1 : -1;
      if (index < 0 || id !== null && !items.has(id) || id !== null && state.pair[1 - index] === id) return reject();
      const pair = [...state.pair]; pair[index] = id;
      update({ pair, message: 'Comparing research roles only; compatibility remains unqualified.' }); return true;
    },
    compare(left, right) {
      if (!getAuthorityRelationship(left, right)) return reject();
      update({ pair: [left, right], message: 'Comparing research roles only; compatibility remains unqualified.' }); return true;
    },
  };
}
