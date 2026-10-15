export const WORKSPACE_LAYOUT = Object.freeze({
  desktopViewport: 1200,
  minimumWorkbench: 934,
  gap: 13,
  center: 420,
  rail: 44,
  left: Object.freeze({ default: 240, min: 208, max: 360 }),
  right: Object.freeze({ default: 320, min: 280, max: 440 }),
});

const finite = value => Number.isFinite(Number(value));
const clamp = (value, minimum, maximum) => Math.min(maximum, Math.max(minimum, Math.round(finite(value) ? Number(value) : minimum)));
const effectiveBounds = bounds => ({
  viewportGate: Number(bounds?.viewportWidth ?? 0),
  workbenchGate: Number(bounds?.workbenchWidth ?? 0),
  workbenchLimit: finite(bounds?.workbenchWidth) ? Number(bounds.workbenchWidth) : Number.POSITIVE_INFINITY,
});
const sameWorkspaceGeometry = (left, right) => {
  const previous = effectiveBounds(left);
  const next = effectiveBounds(right);
  return Object.keys(previous).every(key => Object.is(previous[key], next[key]));
};

export function workspaceIsDesktop({ viewportWidth = 0, workbenchWidth = 0 } = {}) {
  return Number(viewportWidth) >= WORKSPACE_LAYOUT.desktopViewport && Number(workbenchWidth) >= WORKSPACE_LAYOUT.minimumWorkbench;
}

export function workspaceLimit(side, state, bounds = {}) {
  const other = side === 'left' ? 'right' : 'left';
  const fixed = WORKSPACE_LAYOUT.center + WORKSPACE_LAYOUT.gap * 2;
  const width = finite(bounds.workbenchWidth) ? Number(bounds.workbenchWidth) : Number.POSITIVE_INFINITY;
  const available = width - fixed - (state?.[`${other}Collapsed`] ? WORKSPACE_LAYOUT.rail : (finite(state?.[other]) ? Number(state[other]) : WORKSPACE_LAYOUT[other].default));
  return Math.max(WORKSPACE_LAYOUT[side].min, Math.min(WORKSPACE_LAYOUT[side].max, Math.floor(available)));
}

export function normalizeWorkspaceLayout(value = {}, bounds = {}) {
  const desktop = workspaceIsDesktop(bounds);
  let next = {
    desktop,
    left: clamp(finite(value.left) ? value.left : WORKSPACE_LAYOUT.left.default, WORKSPACE_LAYOUT.left.min, WORKSPACE_LAYOUT.left.max),
    right: clamp(finite(value.right) ? value.right : WORKSPACE_LAYOUT.right.default, WORKSPACE_LAYOUT.right.min, WORKSPACE_LAYOUT.right.max),
    leftCollapsed: desktop && Boolean(value.leftCollapsed),
    rightCollapsed: desktop && Boolean(value.rightCollapsed),
  };
  if (!desktop) return next;
  next = { ...next, left: Math.min(next.left, workspaceLimit('left', next, bounds)) };
  next = { ...next, right: Math.min(next.right, workspaceLimit('right', next, bounds)) };
  return next;
}

export function fitWorkflowGraph({ bounds, canvas } = {}) {
  const width = Number(bounds?.width); const height = Number(bounds?.height);
  const canvasWidth = Number(canvas?.width); const canvasHeight = Number(canvas?.height);
  if (![width, height, canvasWidth, canvasHeight].every(value => finite(value) && value > 0)) return null;
  const zoom = Math.min(1, Math.max(.25, Math.min(canvasWidth / width, canvasHeight / height)));
  if (!finite(zoom)) return null;
  return {
    x: Math.round((canvasWidth - width * zoom) / 2),
    y: Math.round((canvasHeight - height * zoom) / 2),
    zoom,
    minimumOverflow: Math.min(canvasWidth / width, canvasHeight / height) < .25,
  };
}

export function createWorkspaceLayout({ bounds = {}, requestFrame = callback => setTimeout(callback, 16), cancelFrame = clearTimeout, onChange = () => {} } = {}) {
  let currentBounds = bounds;
  let state = normalizeWorkspaceLayout({}, currentBounds);
  let gesture = null;
  let frame = null;

  const emit = phase => onChange({ ...state }, phase);
  const cancelFrameIfNeeded = () => { if (frame !== null) { cancelFrame(frame); frame = null; } };
  const cancelActiveResize = phase => {
    if (!gesture) return false;
    cancelFrameIfNeeded();
    state = gesture.before;
    gesture = null;
    if (phase) emit(phase);
    return true;
  };
  const restore = (phase, pointerId = null) => {
    if (!gesture || (pointerId !== null && gesture.pointerId !== pointerId)) return false;
    return cancelActiveResize(phase);
  };
  const flush = () => {
    frame = null;
    if (!gesture || !finite(gesture.pendingX)) return;
    const delta = gesture.pendingX - gesture.startX;
    const side = gesture.side;
    state = { ...state, [side]: clamp(gesture.before[side] + (side === 'left' ? delta : -delta), WORKSPACE_LAYOUT[side].min, workspaceLimit(side, gesture.before, currentBounds)) };
    emit('resize');
  };

  return {
    getState: () => ({ ...state, leftMax: workspaceLimit('left', state, currentBounds), rightMax: workspaceLimit('right', state, currentBounds) }),
    setBounds(next) {
      const nextBounds = next ?? {};
      if (!sameWorkspaceGeometry(currentBounds, nextBounds)) cancelActiveResize('cancel');
      currentBounds = nextBounds;
      state = normalizeWorkspaceLayout(state, currentBounds);
      emit('bounds');
    },
    reset() { cancelFrameIfNeeded(); gesture = null; state = normalizeWorkspaceLayout({}, currentBounds); emit('reset'); },
    toggle(side) {
      if (!['left', 'right'].includes(side) || !state.desktop) return;
      cancelActiveResize('cancel');
      state = normalizeWorkspaceLayout({ ...state, [`${side}Collapsed`]: !state[`${side}Collapsed`] }, currentBounds);
      emit('toggle');
    },
    key(side, event) {
      if (!['left', 'right'].includes(side) || !state.desktop) return false;
      const amount = event.shiftKey ? 48 : 16;
      let next = state[side];
      const maximum = workspaceLimit(side, state, currentBounds);
      if (event.key === 'Home') next = WORKSPACE_LAYOUT[side].min;
      else if (event.key === 'End') next = maximum;
      else if (event.key === 'ArrowLeft') next += side === 'left' ? -amount : amount;
      else if (event.key === 'ArrowRight') next += side === 'left' ? amount : -amount;
      else return false;
      event.preventDefault?.(); state = { ...state, [side]: clamp(next, WORKSPACE_LAYOUT[side].min, maximum) }; emit('key'); return true;
    },
    start(side, event, target) {
      if (gesture || !['left', 'right'].includes(side) || !state.desktop || event.isPrimary === false || (event.button !== undefined && event.button !== 0) || !finite(event.pointerId) || !finite(event.clientX)) return false;
      cancelFrameIfNeeded(); gesture = { side, pointerId: event.pointerId, startX: event.clientX, pendingX: event.clientX, before: { ...state } };
      try { target?.setPointerCapture?.(event.pointerId); } catch { gesture = null; return false; }
      return true;
    },
    move(event) {
      if (!gesture || event.pointerId !== gesture.pointerId || !finite(event.clientX)) return false;
      gesture.pendingX = event.clientX;
      if (frame === null) frame = requestFrame(flush);
      return true;
    },
    finish(event) {
      if (!gesture || event.pointerId !== gesture.pointerId) return false;
      gesture.pendingX = event.clientX; cancelFrameIfNeeded(); flush(); gesture = null; emit('commit'); return true;
    },
    cancel: pointerId => restore('cancel', pointerId),
    dispose: () => restore('dispose'),
  };
}
