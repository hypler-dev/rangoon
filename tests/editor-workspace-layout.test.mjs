import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceLayout, fitWorkflowGraph, normalizeWorkspaceLayout, workspaceIsDesktop } from '../preview/editor-workspace-layout.mjs';

const centerWidth = (state, workbenchWidth) => workbenchWidth
  - (state.leftCollapsed ? 44 : state.left)
  - (state.rightCollapsed ? 44 : state.right)
  - 26;

test('workspace layout uses desktop and available-workbench gates with open narrow fallback', () => {
  assert.equal(workspaceIsDesktop({ viewportWidth: 1200, workbenchWidth: 934 }), true);
  assert.equal(workspaceIsDesktop({ viewportWidth: 1199, workbenchWidth: 1500 }), false);
  assert.deepEqual(normalizeWorkspaceLayout({ leftCollapsed: true, rightCollapsed: true }, { viewportWidth: 800, workbenchWidth: 934 }), { desktop: false, left: 240, right: 320, leftCollapsed: false, rightCollapsed: false });
  assert.deepEqual(normalizeWorkspaceLayout({}, { viewportWidth: 1200, workbenchWidth: 934 }), { desktop: true, left: 208, right: 280, leftCollapsed: false, rightCollapsed: false });
  assert.deepEqual(normalizeWorkspaceLayout({ left: Number.NaN, right: Infinity }, { viewportWidth: 1200, workbenchWidth: 1006 }), { desktop: true, left: 240, right: 320, leftCollapsed: false, rightCollapsed: false });
});

test('workspace layout clamps keyboard changes and resets local values', () => {
  const layout = createWorkspaceLayout({ bounds: { viewportWidth: 1440, workbenchWidth: 1200 } });
  assert.equal(layout.key('left', { key: 'ArrowRight', preventDefault() {} }), true);
  assert.equal(layout.getState().left, 256);
  layout.key('right', { key: 'End', preventDefault() {} });
  assert.equal(layout.getState().right, 440);
  layout.toggle('left'); assert.equal(layout.getState().leftCollapsed, true);
  layout.reset(); assert.deepEqual(((state) => ({ desktop: state.desktop, left: state.left, right: state.right, leftCollapsed: state.leftCollapsed, rightCollapsed: state.rightCollapsed }))(layout.getState()), { desktop: true, left: 240, right: 320, leftCollapsed: false, rightCollapsed: false });
});

test('workspace limits keep the 420px center at the minimum workbench', () => {
  const layout = createWorkspaceLayout({ bounds: { viewportWidth: 1200, workbenchWidth: 934 } });
  assert.equal(layout.getState().leftMax, 208);
  assert.equal(layout.getState().rightMax, 280);
  layout.key('left', { key: 'End', preventDefault() {} });
  assert.equal(layout.getState().left, 208);
});

test('workspace dock restoration normalizes symmetric keyboard and pointer resizes at desktop boundaries', () => {
  for (const workbenchWidth of [934, 1200]) {
    for (const [collapsed, resized] of [['left', 'right'], ['right', 'left']]) {
      const keyboard = createWorkspaceLayout({ bounds: { viewportWidth: 1200, workbenchWidth } });
      keyboard.toggle(collapsed);
      keyboard.key(resized, { key: 'End', preventDefault() {} });
      keyboard.toggle(collapsed);
      assert.ok(centerWidth(keyboard.getState(), workbenchWidth) >= 420, `${collapsed} keyboard restore at ${workbenchWidth}`);

      const frames = [];
      const pointer = createWorkspaceLayout({
        bounds: { viewportWidth: 1200, workbenchWidth },
        requestFrame: callback => { frames.push(callback); return frames.length - 1; },
        cancelFrame() {},
      });
      pointer.toggle(collapsed);
      assert.equal(pointer.start(resized, { pointerId: 1, clientX: 200 }, { setPointerCapture() {} }), true);
      pointer.move({ pointerId: 1, clientX: resized === 'left' ? 600 : -200 });
      pointer.finish({ pointerId: 1, clientX: resized === 'left' ? 600 : -200 });
      pointer.toggle(collapsed);
      assert.ok(centerWidth(pointer.getState(), workbenchWidth) >= 420, `${collapsed} pointer restore at ${workbenchWidth}`);
    }
  }
});

test('workspace visibility and bounds changes cancel active resizes before late callbacks', () => {
  const frames = []; const cancelled = [];
  const layout = createWorkspaceLayout({
    bounds: { viewportWidth: 1200, workbenchWidth: 1200 },
    requestFrame: callback => { frames.push(callback); return frames.length - 1; },
    cancelFrame: handle => cancelled.push(handle),
  });
  const target = { setPointerCapture() {} };

  layout.start('left', { pointerId: 7, clientX: 100 }, target);
  layout.move({ pointerId: 7, clientX: 220 });
  layout.toggle('right');
  const afterToggle = layout.getState();
  frames[0]();
  assert.deepEqual(layout.getState(), afterToggle);
  assert.equal(layout.finish({ pointerId: 7, clientX: 220 }), false);
  assert.equal(layout.cancel(7), false);

  layout.start('right', { pointerId: 8, clientX: 300 }, target);
  layout.move({ pointerId: 8, clientX: 160 });
  layout.setBounds({ viewportWidth: 1200, workbenchWidth: 934 });
  const afterBounds = layout.getState();
  frames[1]();
  assert.deepEqual(layout.getState(), afterBounds);
  assert.equal(layout.finish({ pointerId: 8, clientX: 160 }), false);
  assert.equal(layout.cancel(8), false);
  assert.ok(centerWidth(layout.getState(), 934) >= 420);
  assert.deepEqual(cancelled, [0, 1]);

  const openLayout = createWorkspaceLayout({ bounds: { viewportWidth: 1200, workbenchWidth: 1200 } });
  openLayout.setBounds({ viewportWidth: 1200, workbenchWidth: 934 });
  assert.deepEqual(openLayout.getState(), {
    desktop: true,
    left: 208,
    right: 280,
    leftCollapsed: false,
    rightCollapsed: false,
    leftMax: 208,
    rightMax: 280,
  });
  assert.equal(centerWidth(openLayout.getState(), 934), 420);
});

test('workspace ignores unchanged ResizeObserver geometry while a resize is active', () => {
  const frames = []; const cancelled = [];
  const layout = createWorkspaceLayout({
    bounds: { viewportWidth: 1200, workbenchWidth: 1200 },
    requestFrame: callback => { frames.push(callback); return frames.length - 1; },
    cancelFrame: handle => cancelled.push(handle),
  });
  layout.start('left', { pointerId: 10, clientX: 100 }, { setPointerCapture() {} });
  layout.move({ pointerId: 10, clientX: 220 });
  layout.setBounds({ viewportWidth: '1200', workbenchWidth: '1200', rootHeight: 640 });
  assert.deepEqual(cancelled, []);
  frames[0]();
  assert.equal(layout.getState().left, 360);
  assert.equal(layout.finish({ pointerId: 10, clientX: 220 }), true);
  assert.equal(layout.getState().left, 360);
});

test('workspace reset and narrow fallback invalidate pending resize callbacks', () => {
  for (const nextBounds of [null, { viewportWidth: 1199, workbenchWidth: 934 }]) {
    const frames = [];
    const layout = createWorkspaceLayout({
      bounds: { viewportWidth: 1200, workbenchWidth: 1200 },
      requestFrame: callback => { frames.push(callback); return frames.length - 1; },
      cancelFrame() {},
    });
    layout.start('left', { pointerId: 9, clientX: 100 }, { setPointerCapture() {} });
    layout.move({ pointerId: 9, clientX: 240 });
    if (nextBounds) layout.setBounds(nextBounds);
    else layout.reset();
    const afterTransition = layout.getState();
    frames[0]();
    assert.deepEqual(layout.getState(), afterTransition);
    assert.equal(layout.finish({ pointerId: 9, clientX: 240 }), false);
    if (nextBounds) assert.equal(afterTransition.desktop, false);
  }
});

test('workspace resize coalesces and restores pre-gesture state on cancellation', () => {
  const frames = []; const phases = [];
  const layout = createWorkspaceLayout({ bounds: { viewportWidth: 1440, workbenchWidth: 1200 }, requestFrame: callback => { frames.push(callback); return frames.length - 1; }, cancelFrame: () => {}, onChange: (_, phase) => phases.push(phase) });
  const target = { setPointerCapture() {} };
  layout.start('left', { pointerId: 4, clientX: 100 }, target);
  layout.move({ pointerId: 4, clientX: 170 }); layout.move({ pointerId: 4, clientX: 180 });
  assert.equal(frames.length, 1); frames[0](); assert.equal(layout.getState().left, 320);
  layout.cancel(); assert.equal(layout.getState().left, 240); assert.deepEqual(phases.slice(-2), ['resize', 'cancel']);
});

test('workspace resize commits only on pointer-up and disposal restores a live gesture', () => {
  const frames = [];
  const layout = createWorkspaceLayout({ bounds: { viewportWidth: 1440, workbenchWidth: 1200 }, requestFrame: callback => { frames.push(callback); return frames.length - 1; }, cancelFrame() {} });
  layout.start('right', { pointerId: 5, clientX: 300 }, { setPointerCapture() {} });
  layout.move({ pointerId: 5, clientX: 260 });
  assert.equal(layout.getState().right, 320);
  layout.finish({ pointerId: 5, clientX: 260 });
  assert.equal(layout.getState().right, 360);
  layout.start('right', { pointerId: 6, clientX: 260 }, { setPointerCapture() {} });
  layout.move({ pointerId: 6, clientX: 180 });
  layout.dispose();
  assert.equal(layout.getState().right, 360);
});

test('workspace resize refuses secondary and unrelated pointer cancellation', () => {
  const layout = createWorkspaceLayout({ bounds: { viewportWidth: 1440, workbenchWidth: 1200 } });
  assert.equal(layout.start('left', { pointerId: 1, clientX: 10, isPrimary: false }, { setPointerCapture() {} }), false);
  assert.equal(layout.start('left', { pointerId: 2, clientX: 10 }, { setPointerCapture() { throw new Error('capture failed'); } }), false);
  assert.equal(layout.start('left', { pointerId: 3, clientX: 10 }, { setPointerCapture() {} }), true);
  assert.equal(layout.cancel(4), false);
  assert.equal(layout.cancel(3), true);
});

test('fit graph rejects bad geometry and keeps minimum zoom overflow explicit', () => {
  assert.equal(fitWorkflowGraph({ bounds: { width: 0, height: 20 }, canvas: { width: 200, height: 200 } }), null);
  assert.deepEqual(fitWorkflowGraph({ bounds: { width: 1000, height: 500 }, canvas: { width: 600, height: 400 } }), { x: 0, y: 50, zoom: .6, minimumOverflow: false });
  assert.equal(fitWorkflowGraph({ bounds: { width: 4000, height: 4000 }, canvas: { width: 400, height: 400 } }).minimumOverflow, true);
});

test('fit graph uses content geometry so the full stage remains inside when minimum zoom permits', () => {
  const fitted = fitWorkflowGraph({ bounds: { width: 640, height: 400 }, canvas: { width: 426, height: 400 } });
  assert.equal(fitted.zoom, .665625);
  assert.equal(fitted.x, 0);
  assert.equal(fitted.y, 67);
  assert.equal(fitted.minimumOverflow, false);
});
