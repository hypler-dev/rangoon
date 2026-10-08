// Transient canvas motion. Only a completed gesture may change the local draft.
export function createWorkflowDrag({ getState, requestFrame, cancelFrame, redraw, commit }) {
  let active = null;
  let frame = null;
  let disposed = false;
  const position = (state, index) => state.layout?.positions?.find(item => item.nodeIndex === index) ?? { x: 0, y: 0 };
  const editable = state => !state.pending && !state.fieldEdit && Boolean(state.session) && (state.bridgeAvailable || state.playground === true);
  const valid = () => {
    const state = getState();
    if (!active || !editable(state) || !active.node.isConnected) return false;
    const current = position(state, active.index);
    return JSON.stringify(state.session) === active.session && state.definition?.nodes?.[active.index]?.id === active.id && current.x === active.x && current.y === active.y && Number(state.viewport?.zoom ?? 1) === active.zoom;
  };
  const coords = event => ({
    x: Math.max(-100000, Math.min(100000, Math.round(active.x + (event.clientX - active.startX) / active.zoom))),
    y: Math.max(-100000, Math.min(100000, Math.round(active.y + (event.clientY - active.startY) / active.zoom))),
  });
  const cancel = pointerId => {
    if (!active || pointerId !== undefined && pointerId !== active.pointerId) return false;
    const gesture = active; active = null;
    if (frame !== null) { cancelFrame(frame); frame = null; }
    gesture.node.style.removeProperty('--node-drag-x');
    gesture.node.style.removeProperty('--node-drag-y');
    gesture.node.classList.remove('workflow-node--dragging');
    try { if (gesture.node.hasPointerCapture?.(gesture.pointerId)) gesture.node.releasePointerCapture(gesture.pointerId); } catch { /* Capture may already be lost. */ }
    redraw();
    return true;
  };
  return {
    start(event, node) {
      if (disposed || active || event.button !== 0 || event.isPrimary === false || !Number.isFinite(event.clientX) || !Number.isFinite(event.clientY)) return false;
      const state = getState(); const index = Number(node.dataset.workflowNode); const zoom = Number(state.viewport?.zoom ?? 1);
      if (!editable(state) || !Number.isSafeInteger(index) || index < 0 || !state.definition?.nodes?.[index] || !Number.isFinite(zoom) || zoom < .25 || zoom > 4) return false;
      const origin = position(state, index);
      active = { node, index, id: state.definition.nodes[index].id, session: JSON.stringify(state.session), pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, x: origin.x, y: origin.y, nextX: origin.x, nextY: origin.y, zoom };
      try { node.setPointerCapture?.(event.pointerId); } catch { cancel(); return false; }
      event.preventDefault();
      return true;
    },
    move(event) {
      if (!active || event.pointerId !== active.pointerId) return false;
      if (!valid() || !Number.isFinite(event.clientX) || !Number.isFinite(event.clientY)) { cancel(); return false; }
      const next = coords(event); active.nextX = next.x; active.nextY = next.y;
      if (frame === null) frame = requestFrame(() => {
        frame = null;
        if (!valid()) { cancel(); return; }
        active.node.style.setProperty('--node-drag-x', `${active.nextX - active.x}px`);
        active.node.style.setProperty('--node-drag-y', `${active.nextY - active.y}px`);
        active.node.classList.add('workflow-node--dragging');
        redraw();
      });
      return true;
    },
    finish(event) {
      if (!active || event.pointerId !== active.pointerId) return false;
      if (!valid() || !Number.isFinite(event.clientX) || !Number.isFinite(event.clientY)) { cancel(); return false; }
      const next = coords(event); const gesture = active;
      cancel();
      if (next.x === gesture.x && next.y === gesture.y) return false;
      commit(gesture.index, next.x, next.y);
      return true;
    },
    cancel,
    isActive: () => active !== null,
    dispose() { cancel(); disposed = true; },
  };
}
