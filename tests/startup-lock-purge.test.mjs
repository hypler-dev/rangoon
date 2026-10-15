import test from 'node:test';
import assert from 'node:assert/strict';

function surface() {
  const docListeners = new Map(), listeners = new Map(), timers = new Map();
  let serial = 0, focuses = 0;
  const status = { textContent: '' };
  const actions = { innerHTML: '', textContent: '', replaceChildren() { this.innerHTML = ''; this.textContent = ''; }, querySelector() { return { focus() { ++focuses; } }; } };
  const skip = { hidden: false, inert: true, removeAttribute() { this.inert = false; }, setAttribute() { this.inert = true; } };
  const document = { activeElement: null, querySelector: selector => selector === '#analysis-skip' ? skip : null, addEventListener: (name, fn) => docListeners.set(name, fn), removeEventListener: (name, fn) => { if (docListeners.get(name) === fn) docListeners.delete(name); } };
  const window = { setTimeout: fn => { timers.set(++serial, fn); return serial; }, clearTimeout: id => timers.delete(id) };
  const container = { innerHTML: '', hidden: false, querySelector: selector => selector === '[data-splash-status]' ? status : actions, contains: () => false, addEventListener: (name, fn) => listeners.set(name, fn), removeEventListener: (name, fn) => { if (listeners.get(name) === fn) listeners.delete(name); } };
  const app = { inert: true, removeAttribute() {}, querySelector: () => ({ focus() { ++focuses; } }) };
  return { document, window, container, app, skip, status, actions, listeners, docListeners, timers, focuses: () => focuses };
}
const environment = surface();
globalThis.document = environment.document;
globalThis.window = environment.window;
const { createStartupSplash } = await import('../preview/launch.mjs');
const click = selector => ({ target: { closest: candidate => selector === candidate } });

test('startup retry works normally, then purge neutralizes retained callbacks, timers and listeners', () => {
  const f = surface(); globalThis.document = f.document; globalThis.window = f.window;
  const splash = createStartupSplash(f);
  let reads = 0; splash.setRetry(() => ++reads);
  const oldClick = f.listeners.get('click'), oldTimeout = f.timers.values().next().value, oldKeydown = f.docListeners.get('keydown');
  oldTimeout(); assert.match(f.status.textContent, /longer/);
  oldClick(click('[data-splash-retry]')); assert.equal(reads, 1);
  splash.purgeForWorkspaceLock();
  assert.equal(f.timers.size, 0); assert.equal(f.listeners.size, 0); assert.equal(f.docListeners.size, 0);
  assert.equal(f.container.hidden, true); assert.equal(f.app.inert, false);
  assert.equal(f.skip.inert, true); assert.equal(f.skip.hidden, true);
  assert.equal(f.status.textContent, ''); assert.equal(f.actions.innerHTML, '');
  splash.setRetry(() => ++reads);
  oldClick(click('[data-splash-retry]')); oldTimeout(); oldKeydown({ key: 'Escape' });
  splash.sync({ snapshotsStatus: 'ready' }); splash.openWorkbench(); splash.purgeForWorkspaceLock();
  assert.equal(reads, 1); assert.equal(f.timers.size, 0); assert.equal(f.focuses(), 0);
  assert.equal(f.status.textContent, ''); assert.equal(f.actions.innerHTML, '');
});

test('ordinary startup completion removes retry and cannot later restore loading overlay', () => {
  const f = surface(); globalThis.document = f.document; globalThis.window = f.window;
  const splash = createStartupSplash(f); let reads = 0; splash.setRetry(() => ++reads);
  const oldClick = f.listeners.get('click'), oldTimeout = f.timers.values().next().value;
  splash.sync({ snapshotsStatus: 'ready' });
  assert.equal(f.container.hidden, true); assert.equal(f.skip.inert, false); assert.equal(f.skip.hidden, false); assert.equal(f.timers.size, 0); assert.equal(f.listeners.size, 0);
  oldClick(click('[data-splash-retry]')); oldTimeout();
  assert.equal(reads, 0); assert.equal(f.timers.size, 0);
});
