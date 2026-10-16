import test from 'node:test';
import assert from 'node:assert/strict';
import {
  WORKBENCH_NAVIGATION,
  bindWorkbenchNavigation,
  captureWorkbenchNavigation,
  normalizeWorkbenchArea,
  normalizeWorkbenchSurface,
  renderWorkbenchNavigation,
  restoreWorkbenchNavigation,
} from '../preview/workbench-navigation.mjs';

function fixture({ compact = true } = {}) {
  const document = { activeElement: null };
  const summary = { matches: selector => selector.includes('summary'), focus() { document.activeElement = summary; } };
  const drawer = { open: true, querySelector: selector => selector === 'summary' ? summary : null };
  const link = href => ({
    href,
    getAttribute: name => name === 'href' ? href : null,
    focus() { document.activeElement = this; },
    closest(selector) { return selector.includes('a[href]') ? this : null; },
  });
  const links = [link('#analysis'), link('#workflows')];
  const nestedSummary = { focus() { document.activeElement = this; } };
  const brand = { focus() { document.activeElement = this; }, closest: () => brand };
  const groups = { contains: value => links.includes(value) || value === nestedSummary };
  drawer.querySelector = selector => selector === 'summary' ? summary : selector === '.analysis-nav-groups' ? groups : null;
  const listeners = new Map();
  const root = {
    ownerDocument: document,
    querySelector: selector => selector === '[data-workbench-nav-drawer]' ? drawer : null,
    querySelectorAll: selector => selector.includes('a[href]') ? links : [],
    addEventListener(type, listener) { listeners.set(type, listener); },
    removeEventListener(type) { listeners.delete(type); },
    contains(value) { return value === drawer || links.includes(value) || value === summary; },
    fire(type, event) { listeners.get(type)?.(event); },
  };
  let mediaListener = null;
  const media = { matches: compact, addEventListener(type, listener) { if (type === 'change') mediaListener = listener; }, removeEventListener(type) { if (type === 'change') mediaListener = null; } };
  const windowListeners = new Map();
  const window = {
    matchMedia: () => media,
    addEventListener(type, listener) { windowListeners.set(type, listener); },
    removeEventListener(type) { windowListeners.delete(type); },
  };
  return { document, drawer, summary, links, nestedSummary, brand, root, media, window, change(matches) { media.matches = matches; mediaListener?.({ matches }); }, hide(persisted) { windowListeners.get('pagehide')?.({ persisted }); } };
}

test('shared definition keeps all real routes, stable destinations, and one current item', () => {
  assert.equal(WORKBENCH_NAVIGATION.routes.flatMap(([, links]) => links).length, 12);
  const workbench = renderWorkbenchNavigation({ area: 'workflows', surface: 'workbench' });
  assert.match(workbench, /href="#workflows" aria-current="page"/);
  assert.equal((workbench.match(/aria-current="page"/g) ?? []).length, 1);
  assert.match(workbench, /href="#authority-engines"/);
  assert.match(workbench, /href="#operations"/);
  assert.match(workbench, /class="analysis-brand" href="#analysis"/);
  assert.match(workbench, /Sample design preview/);
  const playground = renderWorkbenchNavigation({ area: 'workflows', surface: 'workflow-playground' });
  assert.match(playground, /href="analyze\.html#workflows"/);
  assert.match(playground, /class="analysis-brand" href="analyze\.html#analysis"/);
  assert.doesNotMatch(playground, /href="analyze\.html#workflows" aria-current/);
  assert.match(playground, /href="workflow-playground\.html" aria-current="page"/);
  assert.equal((playground.match(/aria-current="page"/g) ?? []).length, 1);
});

test('malformed area and surface fall back without reflecting untrusted input', () => {
  const raw = '" autofocus onfocus=alert(1)';
  assert.equal(normalizeWorkbenchArea(raw), 'analysis');
  assert.equal(normalizeWorkbenchSurface(raw), 'workbench');
  const html = renderWorkbenchNavigation({ area: raw, surface: raw, supplementary: raw });
  assert.match(html, /href="#analysis" aria-current="page"/);
  assert.doesNotMatch(html, /autofocus|alert\(1\)/);
  assert.doesNotMatch(html, /data-flow-theme/);
});

test('workspace guard mounts only after real workbench navigation', () => {
  const guard = () => '<section data-workspace-guard>Native workspace protection</section>';
  const workbench = renderWorkbenchNavigation({ area: 'analysis', surface: 'workbench', workspaceGuard: guard });
  const playground = renderWorkbenchNavigation({ area: 'analysis', surface: 'workflow-playground', workspaceGuard: guard });
  assert.match(workbench, /<\/nav><section data-workspace-guard>/);
  assert.ok(workbench.indexOf('data-workspace-guard') < workbench.indexOf('analysis-companion'));
  assert.doesNotMatch(playground, /data-workspace-guard/);
});

test('capture and restore retain compact open state and focused drawer link', () => {
  const page = fixture();
  page.drawer.open = true;
  page.document.activeElement = page.links[1];
  const state = captureWorkbenchNavigation(page.root);
  page.drawer.open = false;
  restoreWorkbenchNavigation(page.root, state, { compact: true, documentRef: page.document });
  assert.equal(page.drawer.open, true);
  assert.equal(page.document.activeElement, page.links[1]);
});

test('Escape and media transitions relocate focus, preserve wide navigation, and dispose listeners', () => {
  const page = fixture({ compact: false });
  const binding = bindWorkbenchNavigation(page.root, { window: page.window, document: page.document });
  page.document.activeElement = page.brand;
  page.change(true);
  assert.equal(page.drawer.open, false);
  assert.equal(page.document.activeElement, page.brand);
  page.change(false);
  page.document.activeElement = page.links[0];
  page.root.fire('keydown', { key: 'Escape', target: { closest: () => page.drawer }, preventDefault() { throw new Error('desktop Escape must not prevent'); } });
  assert.equal(page.drawer.open, true);
  assert.equal(page.document.activeElement, page.links[0]);
  page.change(true);
  assert.equal(page.drawer.open, false);
  assert.equal(page.document.activeElement, page.summary);
  page.drawer.open = true;
  let prevented = false;
  page.root.fire('keydown', { key: 'Escape', target: { closest: () => page.drawer }, preventDefault() { prevented = true; } });
  assert.equal(page.drawer.open, false);
  assert.equal(page.document.activeElement, page.summary);
  assert.equal(prevented, true);
  page.change(false);
  assert.equal(page.drawer.open, true);
  binding.dispose();
  page.drawer.open = false;
  page.change(false);
  assert.equal(page.drawer.open, false);
});

test('wide-to-compact moves hidden supplementary disclosure focus to outer summary', () => {
  const page = fixture({ compact: false });
  const binding = bindWorkbenchNavigation(page.root, { window: page.window, document: page.document });
  page.document.activeElement = page.nestedSummary;
  page.change(true);
  assert.equal(page.drawer.open, false);
  assert.equal(page.document.activeElement, page.summary);
  binding.dispose();
});

test('persisted pagehide keeps compact listeners while ordinary pagehide disposes them', () => {
  const page = fixture({ compact: true });
  bindWorkbenchNavigation(page.root, { window: page.window, document: page.document });
  page.hide(true);
  page.drawer.open = true;
  page.root.fire('keydown', { key: 'Escape', target: { closest: () => page.drawer }, preventDefault() {} });
  assert.equal(page.drawer.open, false);
  page.drawer.open = true;
  page.document.activeElement = page.links[0];
  page.change(false);
  assert.equal(page.drawer.open, true);
  page.change(true);
  assert.equal(page.drawer.open, false);
  assert.equal(page.document.activeElement, page.summary);
  page.hide(false);
  page.drawer.open = true;
  page.root.fire('keydown', { key: 'Escape', target: { closest: () => page.drawer }, preventDefault() { throw new Error('disposed listener'); } });
  assert.equal(page.drawer.open, true);
  page.change(false);
  assert.equal(page.drawer.open, true);
});
