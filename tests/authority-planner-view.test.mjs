import test from 'node:test';
import assert from 'node:assert/strict';
import { createAuthorityPlanner } from '../preview/authority-planner-model.mjs';
import { bindAuthorityPlanner, renderAuthorityPlanner } from '../preview/authority-planner-view.mjs';

test('planner renders the closed five-role board, eight trusted targets, and local-only controls', () => {
  const html = renderAuthorityPlanner(createAuthorityPlanner().getState());
  assert.match(html, /data-plan-add="agentgateway"/);
  assert.match(html, /data-plan-drag="lnsat"/);
  assert.match(html, /data-plan-role="gateway"/);
  assert.match(html, /data-plan-role="action-authority"/);
  assert.match(html, /data-plan-pair="left"/);
  assert.match(html, /data-plan-pair="right"/);
  assert.match(html, /data-plan-relationship/);
  assert.match(html, /data-plan-status/);
  assert.match(html, /data-plan-confirm/);
  assert.match(html, /Connection unavailable/);
  assert.match(html, /adapter_not_implemented/);
  assert.doesNotMatch(html, />\s*(?:Check|Connect|Save|Install)\s*</i);
});

test('planner projects raw values through trusted catalog and relationship lookup', () => {
  const html = renderAuthorityPlanner({
    assignments: { gateway: 'agentgateway', 'access-decision': '<script>' },
    pair: ['permit', 'permit-fastmcp'],
    relationship: { label: '<forged>', authority: 'granted', references: [{ url: 'javascript:alert(1)' }] },
    message: '<img src=x onerror=1>',
  });
  assert.match(html, /data-plan-selected="agentgateway"/);
  assert.match(html, /Documented provider relationship/);
  assert.match(html, /Unqualified/);
  assert.match(html, /Authority<\/dt><dd>None/);
  assert.match(html, /&lt;img src=x onerror=1&gt;/);
  assert.doesNotMatch(html, /forged|granted|javascript:|<img src=x/);
});

test('planner marks an occupied exact role and exposes only applicable local menu actions', () => {
  const planner = createAuthorityPlanner();
  planner.choose('cerbos');
  const html = renderAuthorityPlanner(planner.getState(), { menu: { id: 'cerbos' } });
  assert.match(html, /data-plan-selected="cerbos"/);
  assert.match(html, /data-plan-menu-root/);
  assert.match(html, /data-plan-action="add" data-plan-target="permit">Replace with Permit authorization/);
  assert.match(html, /data-plan-action="remove" data-plan-target="cerbos"/);
  assert.match(html, /data-plan-action="inspect" data-plan-target="cerbos"/);
});

test('planned pair control has a focusable relationship inspection result', () => {
  const planner = createAuthorityPlanner();
  planner.choose('agentgateway');
  planner.choose('cerbos');
  const html = renderAuthorityPlanner(planner.getState());
  assert.match(html, /data-plan-compare="agentgateway\|cerbos"/);
  assert.match(html, /data-plan-relationship tabindex="-1"/);
});

test('planner menu focus exit removes only its popup and leaves the receiving control intact', () => {
  const listeners = new Map();
  const receiver = { closest: () => null };
  const menuRoot = { removed: false, remove() { this.removed = true; } };
  const trigger = { dataset: { planMenu: 'agentgateway' }, focused: 0, closest: selector => selector === '[data-plan-menu]' ? trigger : null, focus() { this.focused += 1; }, setAttribute() {} };
  const menuItem = { focus() {}, closest: selector => selector === '[data-plan-menu-root]' ? menuRoot : null };
  const doc = { activeElement: receiver, addEventListener() {}, removeEventListener() {}, defaultView: { addEventListener() {}, removeEventListener() {} } };
  const container = {
    ownerDocument: doc,
    html: '',
    set innerHTML(value) { this.html = value; },
    get innerHTML() { return this.html; },
    addEventListener(type, listener) { listeners.set(type, listener); },
    removeEventListener(type, listener) { if (listeners.get(type) === listener) listeners.delete(type); },
    querySelector(selector) {
      if (selector.includes('[data-plan-card="agentgateway"]') || selector === '[data-plan-menu="agentgateway"]') return trigger;
      if (selector === '[data-plan-menu-root]') return this.html.includes('data-plan-menu-root') ? menuRoot : null;
      if (selector === '[data-plan-menu-root] button') return menuItem;
      return null;
    }, querySelectorAll() { return []; },
  };
  const planner = createAuthorityPlanner();
  const controller = { getState: () => planner.getState(), subscribe: () => () => {} };
  const binding = bindAuthorityPlanner({ container, controller });
  assert.ok(listeners.has('focusout'));
  listeners.get('click')({ target: trigger });
  assert.match(container.innerHTML, /data-plan-menu-root/);
  listeners.get('focusout')({ target: menuItem, relatedTarget: receiver });
  assert.equal(menuRoot.removed, true);
  assert.equal(doc.activeElement, receiver);
  assert.equal(trigger.focused, 0);
  binding.dispose();
  assert.equal(listeners.has('focusout'), false);
});
