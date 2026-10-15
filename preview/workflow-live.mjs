const clone = value => structuredClone(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const arrays = value => Array.isArray(value) ? value : [];

// UI change history only. These events are never operational evidence or receipts.
export function createWorkflowLiveFeed({ clock = Date.now } = {}) {
  let previous = null, key = null, sequence = 0, omitted = 0, events = [], disposed = false;
  let source = 'disconnected';
  const record = (kind, subjectIds, fields = []) => {
    const timestamp = clock();
    events.push({ sequence: ++sequence, kind, subjectIds: [...subjectIds], fields: [...fields], observedAt: Number.isFinite(timestamp) ? timestamp : null });
    if (events.length > 40) { events.shift(); omitted++; }
  };
  const snapshot = () => clone({ source, events, omitted, sequence, runtime: 'disconnected' });
  return {
    observe(state) {
      if (disposed) return snapshot();
      const nextKey = state?.session?.draftId ?? state?.opened?.revision?.id ?? (state?.playground && state?.definition ? 'synthetic-session' : null);
      source = state?.playground ? 'synthetic-editor' : state?.definition ? 'local-workspace-definition' : 'disconnected';
      if (nextKey !== key) { previous = null; key = nextKey; sequence = 0; omitted = 0; events = []; }
      if (!state?.definition) { previous = null; return snapshot(); }
      const next = { definition: state.definition, layout: state.layout };
      if (!previous) record('graph-opened', []);
      else {
        const before = new Map(arrays(previous.definition.nodes).map(node => [node.id, node]));
        const after = new Map(arrays(next.definition.nodes).map(node => [node.id, node]));
        for (const [id, node] of after) {
          if (!before.has(id)) record('node-added', [id]);
          else {
            const fields = ['title', 'operation', 'inputs', 'outputs'].filter(field => !same(before.get(id)[field], node[field]));
            if (fields.length) record('node-updated', [id], fields);
          }
        }
        for (const id of before.keys()) if (!after.has(id)) record('node-removed', [id]);
        if (before.size === after.size && [...before.keys()].every(id => after.has(id)) && !same([...before.keys()], [...after.keys()])) record('node-order-updated', [...after.keys()]);
        if (previous.definition.title !== next.definition.title) record('graph-title-updated', [], ['title']);
        for (const [field, kind] of [['controlEdges', 'control'], ['dataEdges', 'data']]) {
          const old = new Set(arrays(previous.definition[field]).map(edge => JSON.stringify(edge)));
          const current = new Set(arrays(next.definition[field]).map(edge => JSON.stringify(edge)));
          for (const edge of arrays(next.definition[field])) if (!old.has(JSON.stringify(edge))) record(`${kind}-connected`, [edge.fromNode, edge.toNode]);
          for (const edge of arrays(previous.definition[field])) if (!current.has(JSON.stringify(edge))) record(`${kind}-disconnected`, [edge.fromNode, edge.toNode]);
        }
        const positions = (value, nodes) => new Map(arrays(value?.positions).map(position => [nodes[position.nodeIndex]?.id, { x: position.x, y: position.y }]));
        const oldPositions = positions(previous.layout, arrays(previous.definition.nodes));
        const currentPositions = positions(next.layout, arrays(next.definition.nodes));
        for (const [id, position] of currentPositions) if (before.has(id) && after.has(id) && !same(oldPositions.get(id), position)) record('node-moved', [id]);
      }
      previous = clone(next);
      return snapshot();
    },
    snapshot,
    dispose() { disposed = true; previous = null; events = []; source = 'disconnected'; key = null; sequence = 0; omitted = 0; },
  };
}
