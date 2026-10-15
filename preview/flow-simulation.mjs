// Closed synthetic scripts illustrate boundaries; they are not admitted operations records.
const freeze = value => {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
};

export const FLOW_NODES = freeze([
  { id: 'agent', title: 'Requesting agent', kind: 'request', x: 30, y: 100 },
  { id: 'gateway', title: 'Scoped gateway', kind: 'routing', x: 260, y: 100 },
  { id: 'policy', title: 'Policy evaluator', kind: 'recommendation', x: 490, y: 100 },
  { id: 'human', title: 'Human approver', kind: 'approval', x: 720, y: 100 },
  { id: 'authority', title: 'Optional authority engine', kind: 'authorization', x: 720, y: 330 },
  { id: 'executor', title: 'Execution-time check', kind: 'consumption', x: 490, y: 330 },
  { id: 'tool', title: 'Tool / resource', kind: 'attempt', x: 260, y: 330 },
  { id: 'evidence', title: 'Outcome evidence', kind: 'evidence', x: 30, y: 330 },
]);

export const FLOW_REQUEST = freeze({ title: 'Open a pull request', repository: 'example/project', branch: 'fix/input-validation', action: 'pull_request.create', sourceKind: 'synthetic' });
export const FLOW_SYSTEMS = freeze({
  agent: { label: 'Code agent', type: 'Agent', role: 'Proposes work on the selected repository.', receives: 'Task and source context', sends: 'A proposed action request', boundary: 'A request is not permission.', next: 'Inspect the routing and policy records.' },
  gateway: { label: 'Gateway', type: 'Routing', role: 'Routes the proposed request to the configured checks.', receives: 'Proposed action', sends: 'Request and scoped context', boundary: 'Connectivity does not grant authority.', next: 'Inspect the policy recommendation.' },
  policy: { label: 'Policy service', type: 'Policy', role: 'Reports a policy recommendation for this request.', receives: 'Request and pinned policy revision', sends: 'Allow recommendation or refusal', boundary: 'Allow is not human approval or an execution grant.', next: 'Inspect the recommendation and required human decision.' },
  human: { label: 'Human reviewer', type: 'Person', role: 'Records the required human decision.', receives: 'Request and recommendation', sends: 'Separately scoped decision', boundary: 'Approval cannot bypass execution-time checking.', next: 'Inspect the scoped authorization reference.' },
  authority: { label: 'Authority module', type: 'Authority', role: 'Illustrates issuing an exact scoped reference.', receives: 'Request and required decision', sends: 'Scoped authorization reference', boundary: 'LNSAT is an optional example, not a connected backend.', next: 'Inspect scope and the independent consumption check.' },
  executor: { label: 'Execution gate', type: 'Checkpoint', role: 'Rechecks binding, scope and expiry at consumption.', receives: 'Request and scoped reference', sends: 'Accepted consumption or refusal', boundary: 'An earlier approval does not authorize expired scope.', next: 'Inspect refusal or subsequent outcome evidence.' },
  tool: { label: 'Repository', type: 'Resource', role: 'Illustrates the repository affected by the proposed action.', receives: 'A request after independent consumption', sends: 'An outcome observation, when available', boundary: 'An accepted check is not proof of a completed effect.', next: 'Inspect the outcome or missing-evidence state.' },
  evidence: { label: 'Evidence store', type: 'Evidence', role: 'Keeps the illustrated outcome distinct from earlier decisions.', receives: 'Outcome observation or a known evidence gap', sends: 'Inspectable evidence, or unknown effect', boundary: 'Missing evidence never means success or no effect.', next: 'Inspect provenance; reconcile missing outcomes before drawing conclusions.' },
});

const edge = (id, from, to, kind, points) => ({ id, from, to, kind, points });
export const FLOW_EDGES = freeze([
  edge('request', 'agent', 'gateway', 'request', [[220, 150], [260, 150]]),
  edge('evaluate', 'gateway', 'policy', 'policy input', [[450, 150], [490, 150]]),
  edge('recommend', 'policy', 'human', 'reported recommendation', [[680, 150], [720, 150]]),
  edge('approve', 'human', 'authority', 'approval record', [[815, 200], [815, 330]]),
  edge('authorize', 'authority', 'executor', 'scoped authorization reference', [[720, 380], [680, 380]]),
  edge('consume', 'executor', 'tool', 'independent consumption check', [[490, 380], [450, 380]]),
  edge('outcome', 'tool', 'evidence', 'outcome observation', [[260, 380], [220, 380]]),
  edge('denial', 'policy', 'gateway', 'refusal', [[585, 100], [585, 60], [355, 60], [355, 100]]),
  edge('refusal', 'executor', 'evidence', 'refusal evidence', [[585, 430], [585, 480], [125, 480], [125, 430]]),
  edge('gap', 'tool', 'evidence', 'missing outcome', [[355, 430], [355, 520], [125, 520], [125, 430]]),
]);

const event = (id, stage, title, nodeId, edgeId, reported, detail, evidenceClass = 'reported') =>
  ({ id, stage, title, nodeId, edgeId, reported, detail, evidenceClass });
const request = () => event('request', 'request', 'Exact request enters gateway', 'gateway', 'request', 'requested', 'Synthetic request for one bounded resource operation. No private payload or real actor is loaded.');
const evaluation = () => event('evaluation', 'evaluation', 'Pinned policy evaluated', 'policy', 'evaluate', 'evaluated', 'Illustrated policy revision only. No provider evaluates this fixture.');
const recommendation = () => event('recommendation', 'recommendation', 'Provider recommends allow', 'human', 'recommend', 'allow recommendation', 'Provider allow is a policy input. It is not a Rangoon approval, scoped authorization or permission to execute.');
const approval = () => event('approval', 'approval', 'Required human decision recorded', 'authority', 'approve', 'human approved', 'Synthetic human decision, separately scoped to this illustrated request. No person is authenticated and no agent receives broader authority.');
const authorization = () => event('authorization', 'authorization', 'Scoped authorization reported issued', 'executor', 'authorize', 'issued', 'Illustrated engine reference binds request and policy revision. Approval alone cannot bypass the independent execution-time check.');
const consumption = () => event('consumption', 'consumption', 'Execution-time check reported accepted', 'tool', 'consume', 'accepted', 'A distinct synthetic consumption step checks current scope, binding and expiry. No executable action occurs in this playground.');

const scenario = (id, title, description, events) => ({
  id, title, description, traceId: `synthetic-trace:${id}`, policyRevisionId: 'synthetic-policy:example-v1',
  topologyRevisionId: 'synthetic-topology:flow-v1', events: events.map(value => ({ ...value, id: `${id}:${value.id}` })),
});
export const FLOW_SCENARIOS = freeze([
  scenario('mediated', 'Mediated route', 'Follow each distinct boundary to a reported outcome.', [
    request(), evaluation(), recommendation(), approval(), authorization(), consumption(),
    event('outcome', 'outcome', 'Outcome reported recorded', 'evidence', 'outcome', 'completed (synthetic)', 'Illustrated outcome only. This is not a verified receipt, successful real operation or authenticated audit history.'),
  ]),
  scenario('denied', 'Policy denied', 'A refusal stops the request before approval or execution.', [
    request(), evaluation(), event('denial', 'refusal', 'Policy reports denial', 'gateway', 'denial', 'denied', 'No approval, authorization or execution event follows this synthetic refusal.'),
  ]),
  scenario('expired', 'Expired at consumption', 'Approval exists, but execution-time checking still refuses.', [
    request(), evaluation(), recommendation(), approval(), authorization(),
    event('expiry', 'refusal', 'Consumption refuses expired scope', 'executor', 'refusal', 'refused: expired', 'Earlier approval and issued-reference observations do not authorize a stale operation. No tool attempt or success is illustrated.'),
  ]),
  scenario('unknown', 'Outcome missing', 'An observed attempt does not prove its eventual effect.', [
    request(), evaluation(), recommendation(), approval(), authorization(), consumption(),
    event('gap', 'unknown', 'Outcome evidence unavailable', 'evidence', null, 'effect unknown', 'No outcome observation exists in this synthetic window. Absence does not prove no effect; reconciliation is required.', 'missing'),
  ]),
]);

export function pointOnFlowEdge(id, progress) {
  const item = FLOW_EDGES.find(value => value.id === id);
  if (!item || !Number.isFinite(progress) || progress < 0 || progress > 1) return null;
  const lengths = item.points.slice(1).map((point, i) => Math.hypot(point[0] - item.points[i][0], point[1] - item.points[i][1]));
  let remaining = progress * lengths.reduce((sum, length) => sum + length, 0);
  for (let i = 0; i < lengths.length; i++) {
    if (remaining <= lengths[i] || i === lengths.length - 1) {
      const fraction = remaining / lengths[i];
      return { x: item.points[i][0] + fraction * (item.points[i + 1][0] - item.points[i][0]), y: item.points[i][1] + fraction * (item.points[i + 1][1] - item.points[i][1]) };
    }
    remaining -= lengths[i];
  }
  return null;
}

export function createFlowSimulation({ onChange = () => {} } = {}) {
  let scenarioId = FLOW_SCENARIOS[0].id;
  let cursor = 0, progress = 0, selection = null, mode = 'flat', playing = false, reducedMotion = false, disposed = false;
  const current = () => FLOW_SCENARIOS.find(value => value.id === scenarioId);
  const snapshot = () => freeze({ scenarioId, scenario: current(), cursor, progress, selection, mode, playing, reducedMotion,
    sourceKind: 'synthetic', authority: 'none', authenticity: 'unverified', authoritativeEligibility: 'unknown', execution: 'unavailable' });
  const emit = () => { onChange(snapshot()); return true; };
  const pause = () => { if (disposed) return false; playing = false; return emit(); };
  const seek = value => {
    if (disposed || !Number.isInteger(value) || value < 0 || value > current().events.length) return false;
    cursor = value; progress = 0; playing = false;
    selection = cursor ? { kind: 'event', index: cursor - 1 } : null;
    return emit();
  };
  return {
    snapshot,
    choose(id) {
      if (disposed || !FLOW_SCENARIOS.some(value => value.id === id)) return false;
      scenarioId = id; cursor = 0; progress = 0; selection = null; playing = false;
      return emit();
    },
    seek,
    step(direction = 1) { return direction === 1 || direction === -1 ? seek(cursor + direction) : false; },
    selectEvent(index) {
      if (disposed || !Number.isInteger(index) || index < 0 || index >= cursor) return false;
      selection = { kind: 'event', index }; return emit();
    },
    selectNode(id) {
      if (disposed || !FLOW_NODES.some(value => value.id === id)) return false;
      selection = { kind: 'node', id }; return emit();
    },
    setMode(value) {
      if (disposed || !['flat', 'depth'].includes(value) || (value === 'depth' && reducedMotion)) return false;
      mode = value; return emit();
    },
    setReducedMotion(value) {
      if (disposed || typeof value !== 'boolean') return false;
      reducedMotion = value;
      playing = false; progress = 0;
      if (value) mode = 'flat';
      return emit();
    },
    play() {
      if (disposed || reducedMotion || cursor >= current().events.length) return false;
      playing = true; return emit();
    },
    pause,
    reset() { if (disposed) return false; mode = 'flat'; return seek(0); },
    tick(deltaMs) {
      if (disposed || !playing || !Number.isFinite(deltaMs) || deltaMs < 0) return false;
      progress += Math.min(deltaMs, 250) / 1600;
      if (progress < 1) return true;
      cursor++; progress = 0; selection = { kind: 'event', index: cursor - 1 };
      if (cursor >= current().events.length) playing = false;
      return emit();
    },
    motion() {
      if (disposed || !playing || reducedMotion) return null;
      const item = current().events[cursor];
      if (!item?.edgeId) return null;
      const point = pointOnFlowEdge(item.edgeId, progress);
      return point ? { ...point, eventId: item.id, edgeId: item.edgeId } : null;
    },
    dispose() { playing = false; progress = 0; selection = null; disposed = true; },
  };
}
