import { createFlowScene, defaultCamera, normalizeCamera } from './flow-scene.mjs';

export { defaultCamera, normalizeCamera };
const roles = Object.freeze({ input: 'agent', capability: 'tool', check: 'policy', branch: 'gateway', checkpoint: 'human', output: 'evidence' });
const finite = value => Number.isFinite(value) && Math.abs(value) <= 100000;
const identifier = value => typeof value === 'string' && /^[a-zA-Z0-9_-]{1,128}$/.test(value);

// Presentation projection only. The workflow controller retains every record and gate.
export function projectWorkflowScene(state) {
  const definition = state?.definition;
  const nodes = definition?.nodes;
  const controls = definition?.controlEdges;
  const mappings = definition?.dataEdges;
  const unavailable = reason => ({ available: false, reason });
  if (!Array.isArray(nodes) || !Array.isArray(controls) || !Array.isArray(mappings)) return unavailable('No complete graph is open.');
  if (!nodes.length) return unavailable('Add a module to inspect it in 3D.');
  if (nodes.length > 24 || controls.length + mappings.length > 96) return unavailable('3D supports up to 24 nodes and 96 connections. Every record remains available in 2D and Outline.');
  if (new Set(nodes.map(node => node?.id)).size !== nodes.length || nodes.some(node => !identifier(node?.id) || !roles[node?.operation?.kind])) return unavailable('This graph cannot be projected safely. Use 2D or Outline.');
  const positions = nodes.map((node, index) => {
    const saved = state.layout?.positions?.find(item => item.nodeIndex === index);
    return { x: saved?.x ?? 0, y: saved?.y ?? 0 };
  });
  if (positions.some(point => !finite(point.x) || !finite(point.y))) return unavailable('Graph positions are unavailable. Use 2D or Outline.');
  const minX = Math.min(0, ...positions.map(point => point.x));
  const maxX = Math.max(1, ...positions.map(point => point.x));
  const minY = Math.min(0, ...positions.map(point => point.y));
  const maxY = Math.max(1, ...positions.map(point => point.y));
  const scale = Math.min(4.8 / Math.max(400, maxX - minX), 2.7 / Math.max(230, maxY - minY));
  const poses = nodes.map((node, index) => ({ id: node.id, kind: roles[node.operation.kind], position: [(positions[index].x - (minX + maxX) / 2) * scale, .15, (positions[index].y - (minY + maxY) / 2) * scale] }));
  const byId = new Map(poses.map(node => [node.id, node]));
  const edges = [...controls.map((edge, index) => ({ id: `control:${index}`, from: edge.fromNode, to: edge.toNode, kind: 'control' })), ...mappings.map((edge, index) => ({ id: `data:${index}`, from: edge.fromNode, to: edge.toNode, kind: 'data' }))];
  if (edges.some(edge => !byId.has(edge.from) || !byId.has(edge.to) || edge.from === edge.to)) return unavailable('A connection cannot be projected safely. Use 2D or Outline.');
  const routes = new Map(edges.map(edge => {
    const a = byId.get(edge.from).position, b = byId.get(edge.to).position;
    const dx = b[0] - a[0], dz = b[2] - a[2], length = Math.hypot(dx, dz);
    const socket = Math.min(.56, length / 3), ux = length ? dx / length : 1, uz = length ? dz / length : 0;
    const start = [a[0] + ux * socket, .2, a[2] + uz * socket];
    const end = [b[0] - ux * socket, .2, b[2] - uz * socket];
    const side = edge.kind === 'data' ? .18 : -.08;
    return [edge.id, [start, [(a[0] + b[0]) / 2 - uz * side, .2, (a[2] + b[2]) / 2 + ux * side], end]];
  }));
  const pointOnEdge = (id, progress) => {
    const points = routes.get(id);
    if (!points || !Number.isFinite(progress) || progress < 0 || progress > 1) return null;
    if (progress === 0) return points[0];
    if (progress === 1) return points.at(-1);
    const lengths = points.slice(1).map((point, index) => Math.hypot(...point.map((value, axis) => value - points[index][axis])));
    let remaining = progress * lengths.reduce((sum, value) => sum + value, 0);
    for (let index = 0; index < lengths.length; index++) {
      if (remaining <= lengths[index] || index === lengths.length - 1) {
        const t = lengths[index] ? remaining / lengths[index] : 0;
        return points[index].map((value, axis) => value + (points[index + 1][axis] - value) * t);
      }
      remaining -= lengths[index];
    }
    return null;
  };
  return { available: true, autoFitLabels: true, poses, edges, route: id => routes.get(id), pointOnEdge, fingerprint: JSON.stringify([poses, edges]), topologyFingerprint: JSON.stringify([poses.map(pose => [pose.id, pose.kind]), edges]), overlap: new Set(positions.map(point => `${point.x},${point.y}`)).size < positions.length };
}

export function createWorkflowScene(canvas, graph, callbacks) {
  if (!graph?.available) return null;
  return createFlowScene(canvas, { ...callbacks, topology: graph });
}

// Derive a disposable view frame from admitted records, never from provider data.
export function frameForSelection(projection, selection) {
  if (!projection?.available || !selection || !Number.isSafeInteger(selection.index) || selection.index < 0) return null;
  const poses = projection.poses;
  const edges = projection.edges;
  if (!Array.isArray(poses) || !Array.isArray(edges)) return null;
  const ids = new Set();
  let route = [];
  if (selection.kind === 'node') {
    const selected = poses[selection.index];
    if (!selected) return null;
    ids.add(selected.id);
    for (const edge of edges) if (edge.from === selected.id || edge.to === selected.id) { ids.add(edge.from); ids.add(edge.to); }
  } else if (selection.kind === 'control' || selection.kind === 'data') {
    const edge = edges.find(value => value.id === `${selection.kind}:${selection.index}`);
    if (!edge) return null;
    ids.add(edge.from); ids.add(edge.to);
    route = projection.route(edge.id);
    if (!Array.isArray(route)) return null;
  } else return null;
  const section = poses.filter(pose => ids.has(pose.id));
  const points = [...section.map(pose => pose.position), ...route];
  if (section.length !== ids.size || !points.length || points.some(point => !Array.isArray(point) || point.length !== 3 || !point.every(value => Number.isFinite(value) && Math.abs(value) <= 12))) return null;
  const minX = Math.min(...points.map(point => point[0])), maxX = Math.max(...points.map(point => point[0]));
  const minZ = Math.min(...points.map(point => point[2])), maxZ = Math.max(...points.map(point => point[2]));
  const span = Math.max(2.4, maxX - minX + 1.5, (maxZ - minZ + 1.5) * 1.6);
  return { center: [(minX + maxX) / 2, .35, (minZ + maxZ) / 2], framing: Math.max(.65, Math.min(1.6, 6 / span)), nodeIds: section.map(pose => pose.id) };
}
