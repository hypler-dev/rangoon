import { AUTHORITY_CATALOG } from './authority-catalog.mjs';
import { AUTHORITY_PLANNER_ROLES, getAuthorityRole, projectAuthorityPlanner } from './authority-planner-model.mjs';
import { createAuthorityPlannerDrag } from './authority-planner-drag.mjs';

const items = new Map(AUTHORITY_CATALOG.map(item => [item.id, item]));
const roles = new Map(AUTHORITY_PLANNER_ROLES.map(role => [role.id, role]));
const escape = value => String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
const monogram = item => item.logoPath
  ? `<span class="authority-plan__logo"><img src="${escape(item.logoPath)}" alt="" aria-hidden="true"></span>`
  : `<span class="authority-plan__logo authority-plan__monogram" aria-hidden="true">${escape(item.family.slice(0, 2).toUpperCase())}</span>`;
const targetName = id => items.get(id)?.productTarget ?? 'Unknown target';
const targetOption = (id, chosen) => `<option value="${escape(id)}" ${id === chosen ? 'selected' : ''}>${escape(targetName(id))}</option>`;

const paletteItem = (item, ui) => {
  const selected = ui.menu?.id === item.id;
  return `<article class="authority-plan__source" data-plan-card="${escape(item.id)}"><header>${monogram(item)}<div><p>${escape(item.badge)}</p><h3>${escape(item.productTarget)}</h3></div><button type="button" class="authority-plan__more" data-plan-menu="${escape(item.id)}" aria-label="Actions for ${escape(item.productTarget)}" aria-expanded="${selected}">More</button></header><p>${escape(item.summary)}</p><footer><button type="button" class="analysis-button analysis-button--small" data-plan-add="${escape(item.id)}">Add</button><button type="button" class="authority-plan__drag" data-plan-drag="${escape(item.id)}" aria-label="Drag ${escape(item.productTarget)} to ${escape(getAuthorityRole(item.id)?.label ?? 'its role')}">Drag</button></footer></article>`;
};
const roleSlot = (role, state, ui) => {
  const id = state.assignments[role.id];
  const item = id ? items.get(id) : null;
  const menuOpen = item && ui.menu?.id === id;
  return `<article class="authority-plan__role${item ? ' authority-plan__role--filled' : ''}" data-plan-role="${escape(role.id)}"${item ? ` data-plan-selected="${escape(item.id)}"` : ''}><header><div><p>OPTIONAL RESPONSIBILITY</p><h3>${escape(role.label)}</h3></div><span>${item ? '1 / 1 planned' : 'empty'}</span></header><p>${escape(role.description)}</p>${item ? `<div class="authority-plan__assignment"><div>${monogram(item)}<strong>${escape(item.productTarget)}</strong></div><div><button type="button" class="authority-plan__more" data-plan-menu="${escape(item.id)}" aria-label="Actions for ${escape(item.productTarget)}" aria-expanded="${menuOpen}">More</button><button type="button" class="authority-plan__drag" data-plan-drag="${escape(item.id)}" aria-label="Drag ${escape(item.productTarget)}">Drag</button></div></div>` : `<small>Choose one matching target. Placement makes no connection or authorization.</small>`}</article>`;
};
const menu = ui => {
  if (!ui.menu || !items.has(ui.menu.id)) return '';
  const item = items.get(ui.menu.id);
  const role = getAuthorityRole(item.id);
  const assigned = ui.state.assignments[role?.id] === item.id;
  const alternatives = assigned ? role.targets.filter(id => id !== item.id).map(id => `<button type="button" role="menuitem" data-plan-action="add" data-plan-target="${escape(id)}">Replace with ${escape(targetName(id))}</button>`).join('') : `<button type="button" role="menuitem" data-plan-action="add" data-plan-target="${escape(item.id)}">Add to role</button>`;
  return `<div class="authority-plan__menu" data-plan-menu-root role="menu" aria-label="Actions for ${escape(item.productTarget)}" tabindex="-1"><p>${escape(item.productTarget)}</p>${alternatives}<button type="button" role="menuitem" data-plan-action="compare" data-plan-target="${escape(item.id)}">Compare relationship</button><button type="button" role="menuitem" data-plan-action="inspect" data-plan-target="${escape(item.id)}">Inspect catalog entry</button>${assigned ? `<button type="button" role="menuitem" data-plan-action="remove" data-plan-target="${escape(item.id)}">Remove from plan</button>` : ''}</div>`;
};
const relationship = state => {
  const value = state.relationship;
  if (!value) return `<section class="authority-plan__relationship" data-plan-relationship tabindex="-1"><h3>Relationship inspection</h3><p>Choose two distinct targets. Local research only; no compatibility, adapter, authority, or network action is implied.</p></section>`;
  return `<section class="authority-plan__relationship" data-plan-relationship tabindex="-1"><header><div><p>RESEARCH RELATIONSHIP</p><h3>${escape(value.label)}</h3></div><span>Unqualified</span></header><p>${escape(value.rationale)}</p><dl><div><dt>Basis</dt><dd>${escape(value.basis.replaceAll('_', ' '))}</dd></div><div><dt>Required later</dt><dd>${escape(value.requirements)}</dd></div><div><dt>Authority</dt><dd>None</dd></div><div><dt>Network attempted</dt><dd>No</dd></div></dl><p class="authority-plan__sources">${value.references.map(reference => `<a href="${escape(reference.url)}" target="_blank" rel="noopener noreferrer">${escape(reference.label)}</a>`).join(' · ')}</p></section>`;
};

export function renderAuthorityPlanner(raw = {}, ui = {}) {
  const state = projectAuthorityPlanner(raw);
  const safeUi = { menu: ui.menu && items.has(ui.menu.id) ? ui.menu : null, state };
  const selected = Object.values(state.assignments).filter(Boolean);
  const plannedPairs = state.relationships.map(value => `<button type="button" class="authority-plan__pair-link" data-plan-compare="${escape(value.ids.join('|'))}">${escape(targetName(value.ids[0]))} + ${escape(targetName(value.ids[1]))}<small>${escape(value.label)}</small></button>`).join('');
  return `<section class="authority-plan" aria-labelledby="authority-plan-title"><header class="authority-plan__hero"><div><p class="analysis-kicker">LOCAL ARRANGEMENT / MEMORY ONLY</p><h2 id="authority-plan-title">Plan complementary responsibilities</h2><p>Roles describe responsibilities, not traffic order. Arranging a card changes only this local view.</p></div><div class="authority-plan__unavailable"><button type="button" disabled>Connection unavailable</button><code>adapter_not_implemented</code></div></header><p class="authority-plan__status" data-plan-status role="status" aria-live="polite">${escape(state.message)}</p><div class="authority-plan__board"><section class="authority-plan__palette" aria-label="Available authority targets"><header><p>8 BUILT-IN TARGETS</p><h3>Palette</h3></header><div>${AUTHORITY_CATALOG.map(item => paletteItem(item, safeUi)).join('')}</div></section><section class="authority-plan__roles" aria-label="Optional responsibility slots">${AUTHORITY_PLANNER_ROLES.map(role => roleSlot(role, state, safeUi)).join('')}<div class="authority-plan__links" aria-label="Planned design references">${selected.length > 1 ? selected.map(id => `<span>${escape(targetName(id))}</span>`).join('<i aria-hidden="true"></i>') : '<span>Add two targets to show a dashed design reference.</span>'}</div>${plannedPairs ? `<div class="authority-plan__pair-links" aria-label="Inspect planned pairs">${plannedPairs}</div>` : ''}</section></div><section class="authority-plan__pair"><header><div><p>PAIR INSPECTION</p><h3>Compare two research targets</h3></div><button type="button" class="analysis-button analysis-button--small" data-plan-clear>Clear local plan</button></header><div class="authority-plan__pair-controls"><label>Left target<select data-plan-pair="left"><option value="">Choose target</option>${AUTHORITY_CATALOG.map(item => targetOption(item.id, state.pair[0])).join('')}</select></label><label>Right target<select data-plan-pair="right"><option value="">Choose other target</option>${AUTHORITY_CATALOG.map(item => targetOption(item.id, state.pair[1])).join('')}</select></label></div>${relationship(state)}</section>${menu(safeUi)}<dialog class="authority-plan__confirm" data-plan-confirm aria-labelledby="authority-plan-confirm-title"><form method="dialog"><h3 id="authority-plan-confirm-title">Replace local role?</h3><p data-plan-confirm-copy>Confirm this local replacement. It does not connect or authorize a provider.</p><div><button type="button" class="analysis-button" data-plan-confirm-cancel>Cancel</button><button type="button" class="analysis-button analysis-button--primary" data-plan-confirm-accept>Replace</button></div></form></dialog></section>`;
}

export function bindAuthorityPlanner({ container, controller, onInspect = () => {} } = {}) {
  if (!container || !controller || typeof controller.getState !== 'function' || typeof controller.subscribe !== 'function') return { dispose() {} };
  const doc = container.ownerDocument;
  let disposed = false, menuState = null, replacement = null, returnFocus = null, dialogReturnFocus = null;
  let drag;
  const menuSelector = id => `[data-plan-menu="${id}"]`;
  const render = () => {
    if (disposed) return;
    container.innerHTML = renderAuthorityPlanner(controller.getState(), { menu: menuState });
    if (menuState) {
      const anchor = menuState.anchor === 'assigned' ? `[data-plan-role][data-plan-selected="${menuState.id}"] [data-plan-menu]` : `[data-plan-card="${menuState.id}"] [data-plan-menu]`;
      const invoker = container.querySelector(anchor), popup = container.querySelector('[data-plan-menu-root]');
      const base = container.getBoundingClientRect?.(), box = invoker?.getBoundingClientRect?.();
      if (popup && base && box) {
        const maxX = Math.max(8, base.width - popup.offsetWidth - 8);
        const preferredY = Math.max(8, box.bottom - base.top + 4);
        const viewportHeight = doc.defaultView?.innerHeight ?? Infinity;
        const maxY = viewportHeight - base.top - popup.offsetHeight - 8;
        const aboveY = box.top - base.top - popup.offsetHeight - 4;
        popup.style.setProperty('--plan-menu-x', `${Math.min(maxX, Math.max(8, box.left - base.left))}px`);
        popup.style.setProperty('--plan-menu-y', `${Math.max(8, preferredY > maxY ? aboveY : preferredY)}px`);
      }
    }
    if (returnFocus) {
      const focus = typeof returnFocus === 'string' ? { selector: returnFocus, preventScroll: true } : returnFocus;
      container.querySelector(focus.selector)?.focus({ preventScroll: focus.preventScroll });
      returnFocus = null;
    }
  };
  const closeMenu = ({ restoreFocus = true } = {}) => { if (!menuState) return; const { id, anchor } = menuState; menuState = null; returnFocus = restoreFocus ? (anchor === 'assigned' ? `[data-plan-role][data-plan-selected="${id}"] [data-plan-menu]` : menuSelector(id)) : null; render(); };
  const dismissMenu = () => { if (!menuState) return; const id = menuState.id; menuState = null; returnFocus = null; container.querySelector('[data-plan-menu-root]')?.remove(); container.querySelector(menuSelector(id))?.setAttribute('aria-expanded', 'false'); };
  const openMenu = (id, invoker) => { if (!items.has(id)) return; menuState = { id, anchor: invoker?.closest?.('[data-plan-role][data-plan-selected]') ? 'assigned' : 'palette' }; returnFocus = '[data-plan-menu-root] button'; render(); };
  const closeDialog = () => { const dialog = container.querySelector('[data-plan-confirm]'); if (dialog?.open) dialog.close(); };
  const replacementDialog = token => { replacement = token; dialogReturnFocus = `[data-plan-add="${token.id}"]`; const dialog = container.querySelector('[data-plan-confirm]'), copy = container.querySelector('[data-plan-confirm-copy]'); if (copy) copy.textContent = `Replace ${targetName(token.occupant)} with ${targetName(token.id)} in ${roles.get(token.role)?.label ?? 'this role'}? Local plan only; no provider changes.`; if (dialog?.showModal && !dialog.open) dialog.showModal(); else if (dialog) dialog.setAttribute('open', ''); };
  const choose = (id, role = getAuthorityRole(id)?.id) => { if (!items.has(id) || !roles.has(role)) return false; returnFocus = `[data-plan-add="${id}"]`; if (controller.choose(id, role)) return true; const token = controller.requestReplacement(id, role); if (token) { replacementDialog(token); return true; } return false; };
  const hitRole = (x, y) => [...container.querySelectorAll('[data-plan-role]')].find(element => { const box = element.getBoundingClientRect?.(); return box && x >= box.left && x <= box.right && y >= box.top && y <= box.bottom; })?.dataset.planRole ?? null;
  drag = createAuthorityPlannerDrag({ getState: controller.getState, requestFrame: callback => doc.defaultView?.requestAnimationFrame(callback) ?? setTimeout(callback, 16), cancelFrame: id => (doc.defaultView?.cancelAnimationFrame ?? clearTimeout)(id), hitRole, commit: choose });
  const subscription = () => { if (replacement) { controller.cancelReplacement(replacement); replacement = null; dialogReturnFocus = null; } render(); };
  const unsubscribe = controller.subscribe(subscription);
  const click = event => {
    const add = event.target.closest?.('[data-plan-add]'); if (add) { choose(add.dataset.planAdd); return; }
    const more = event.target.closest?.('[data-plan-menu]'); if (more) { openMenu(more.dataset.planMenu, more); return; }
    const action = event.target.closest?.('[data-plan-action]');
    if (action && items.has(action.dataset.planTarget)) { const id = action.dataset.planTarget; menuState = null; render(); if (action.dataset.planAction === 'add') choose(id); if (action.dataset.planAction === 'remove') { returnFocus = '[data-plan-clear]'; controller.remove(getAuthorityRole(id)?.id); } if (action.dataset.planAction === 'inspect') onInspect(id); if (action.dataset.planAction === 'compare') { const [left] = controller.getState().pair; returnFocus = '[data-plan-pair="right"]'; if (!left || left === id) controller.setPair('left', id); else controller.compare(left, id); } return; }
    const pairLink = event.target.closest?.('[data-plan-compare]'); if (pairLink) { const [left, right] = pairLink.dataset.planCompare.split('|'); returnFocus = { selector: '[data-plan-relationship]', preventScroll: false }; controller.compare(left, right); return; }
    if (event.target.closest?.('[data-plan-clear]')) { returnFocus = '[data-plan-clear]'; controller.clear(); return; }
    if (event.target.closest?.('[data-plan-confirm-accept]')) { returnFocus = dialogReturnFocus; const token = replacement; replacement = null; dialogReturnFocus = null; closeDialog(); if (token) controller.confirmReplacement(token); return; }
    if (event.target.closest?.('[data-plan-confirm-cancel]')) { if (replacement) controller.cancelReplacement(replacement); replacement = null; returnFocus = dialogReturnFocus; dialogReturnFocus = null; closeDialog(); render(); }
  };
  const change = event => { const pair = event.target.closest?.('[data-plan-pair]'); if (!pair) return; returnFocus = `[data-plan-pair="${pair.dataset.planPair}"]`; controller.setPair(pair.dataset.planPair, pair.value || null); };
  const pointerdown = event => { const handle = event.target.closest?.('[data-plan-drag]'); if (handle && drag.start(event, handle, handle.dataset.planDrag)) handle.focus({ preventScroll: true }); };
  const contextmenu = event => { const source = event.target.closest?.('[data-plan-card],[data-plan-selected]'), id = source?.dataset.planCard ?? source?.dataset.planSelected; if (!items.has(id)) return; event.preventDefault(); openMenu(id, source); };
  const keydown = event => { const active = doc.activeElement; if (event.key === 'Escape') { if (replacement) { if (replacement) controller.cancelReplacement(replacement); replacement = null; returnFocus = dialogReturnFocus; dialogReturnFocus = null; closeDialog(); render(); return; } drag.cancel(); closeMenu(); return; } if ((event.key === 'ContextMenu' || event.shiftKey && event.key === 'F10')) { const source = active?.closest?.('[data-plan-card],[data-plan-selected]'), id = source?.dataset.planCard ?? source?.dataset.planSelected; if (items.has(id)) { event.preventDefault(); openMenu(id, source); } return; } const menu = active?.closest?.('[data-plan-menu-root]'); if (!menu || !['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return; const buttons = [...menu.querySelectorAll('[role="menuitem"]:not([disabled])')], index = buttons.indexOf(active), next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length; event.preventDefault(); buttons[next]?.focus(); };
  const documentKeydown = event => { if (event.key === 'Escape' && drag.isActive()) drag.cancel(); };
  const outside = event => { if (menuState && !event.target.closest?.('[data-plan-menu-root],[data-plan-menu]')) dismissMenu(); };
  const menuFocusout = event => {
    if (!menuState || !event.target.closest?.('[data-plan-menu-root]')) return;
    if (event.relatedTarget?.closest?.('[data-plan-menu-root]')) return;
    dismissMenu();
  };
  const dialogCancel = () => { if (replacement) controller.cancelReplacement(replacement); replacement = null; returnFocus = dialogReturnFocus; dialogReturnFocus = null; render(); };
  const dialogClose = () => { if (replacement) dialogCancel(); };
  const pointermove = event => drag.move(event);
  const pointerup = event => drag.finish(event);
  const pointercancel = event => drag.cancel(event.pointerId);
  const lostcapture = event => drag.cancel(event.pointerId);
  const blur = () => drag.cancel();
  container.addEventListener('click', click); container.addEventListener('change', change); container.addEventListener('pointerdown', pointerdown); container.addEventListener('pointermove', pointermove); container.addEventListener('pointerup', pointerup); container.addEventListener('pointercancel', pointercancel); container.addEventListener('lostpointercapture', lostcapture); container.addEventListener('contextmenu', contextmenu); container.addEventListener('keydown', keydown); container.addEventListener('focusout', menuFocusout); container.addEventListener('cancel', dialogCancel, true); container.addEventListener('close', dialogClose, true); doc.addEventListener('pointerdown', outside); doc.addEventListener('keydown', documentKeydown); doc.defaultView?.addEventListener('blur', blur);
  render();
  return { dispose() { if (disposed) return; drag.dispose(); if (replacement) controller.cancelReplacement(replacement); replacement = null; closeDialog(); unsubscribe(); container.removeEventListener('click', click); container.removeEventListener('change', change); container.removeEventListener('pointerdown', pointerdown); container.removeEventListener('pointermove', pointermove); container.removeEventListener('pointerup', pointerup); container.removeEventListener('pointercancel', pointercancel); container.removeEventListener('lostpointercapture', lostcapture); container.removeEventListener('contextmenu', contextmenu); container.removeEventListener('keydown', keydown); container.removeEventListener('focusout', menuFocusout); container.removeEventListener('cancel', dialogCancel, true); container.removeEventListener('close', dialogClose, true); doc.removeEventListener('pointerdown', outside); doc.removeEventListener('keydown', documentKeydown); doc.defaultView?.removeEventListener('blur', blur); disposed = true; } };
}
