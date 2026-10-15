import { getAuthorityRole } from './authority-planner-model.mjs';

const finite = value => Number.isFinite(value);

// Planner-only pointer helper. It never uses dataTransfer or ambient text.
export function createAuthorityPlannerDrag({ getState, requestFrame, cancelFrame, commit, hitRole }) {
  let active = null;
  let disposed = false;

  const clear = () => {
    const current = active;
    if (!current) return false;
    active = null;
    if (current.frame !== null) cancelFrame(current.frame);
    current.source.style.removeProperty('--plan-drag-x');
    current.source.style.removeProperty('--plan-drag-y');
    current.source.classList.remove('authority-plan-dragging');
    try { if (current.source.hasPointerCapture?.(current.pointerId)) current.source.releasePointerCapture(current.pointerId); } catch {}
    return true;
  };
  const valid = () => active && active.source.isConnected && getState()?.revision === active.revision;
  const draw = () => {
    if (!valid()) return clear();
    active.frame = null;
    active.source.style.setProperty('--plan-drag-x', `${active.x - active.startX}px`);
    active.source.style.setProperty('--plan-drag-y', `${active.y - active.startY}px`);
  };
  const schedule = () => {
    if (!active || active.frame !== null) return;
    active.frame = requestFrame(draw);
  };

  return {
    start(event, source, id) {
      if (disposed || active || !source?.isConnected || event?.button !== 0 || event?.isPrimary !== true || !finite(event?.clientX) || !finite(event?.clientY) || !Number.isInteger(event?.pointerId) || !getAuthorityRole(id)) return false;
      const state = getState();
      if (!state || !Number.isSafeInteger(state.revision)) return false;
      active = { source, id, pointerId: event.pointerId, revision: state.revision, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY, moved: false, frame: null };
      try { source.setPointerCapture?.(event.pointerId); } catch { active = null; return false; }
      event.preventDefault?.();
      return true;
    },
    move(event) {
      if (!active || event?.pointerId !== active.pointerId) return false;
      if (!finite(event?.clientX) || !finite(event?.clientY)) { clear(); return false; }
      if (!valid()) return clear();
      active.x = event.clientX;
      active.y = event.clientY;
      const distance = Math.hypot(active.x - active.startX, active.y - active.startY);
      if (distance < 8) return false;
      active.moved = true;
      active.source.classList.add('authority-plan-dragging');
      schedule();
      event.preventDefault?.();
      return true;
    },
    finish(event) {
      if (!active || event?.pointerId !== active.pointerId) return false;
      if (!finite(event?.clientX) || !finite(event?.clientY)) { clear(); return false; }
      if (!valid() || !active.moved) { clear(); return false; }
      const { id } = active;
      const role = hitRole(event.clientX, event.clientY);
      clear();
      if (typeof role !== 'string' || getAuthorityRole(id)?.id !== role) return false;
      return commit(id, role) === true;
    },
    cancel(pointerId) {
      if (!active || pointerId !== undefined && pointerId !== active.pointerId) return false;
      return clear();
    },
    isActive: () => Boolean(active),
    dispose() { disposed = true; clear(); },
  };
}
