import { AUTHORITY_AREAS, AUTHORITY_CATALOG, AUTHORITY_CATALOG_SCHEMA, authorityRouteFor, projectAuthorityState } from './authority-catalog.mjs';
import { AUTHORITY_GLOSSARY, AUTHORITY_RESEARCH_DATE, getAuthorityComparison } from './authority-comparison.mjs';
import { AUTHORITY_CONTROL_KEY, AUTHORITY_COVERAGE_KEY, getAuthorityReadiness } from './authority-readiness.mjs';
import { icon } from './icons.mjs';
import { createAuthorityPlanner } from './authority-planner-model.mjs';
import { renderAuthorityPlanner, bindAuthorityPlanner } from './authority-planner-view.mjs';

const escape = value => String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
const operationLabel = value => value.replaceAll('_', ' ');
const areaById = new Map(AUTHORITY_AREAS.map(area => [area.id, area]));
const meterLabel = Object.freeze({ configuration_preview: 'Config', connection_check: 'Connect', evidence_read: 'Evidence', authorization_evaluation: 'Policy', guardrail_evaluation: 'Guardrail' });

export function renderAuthorityContext(area) {
  if (!areaById.has(area)) return '';
  const entries = AUTHORITY_CATALOG.filter(item => item.areas.some(reference => reference.id === area));
  const label = areaById.get(area).label;
  return `<details class="authority-context"><summary>Authority references</summary><div>${entries.length ? entries.map(item => `<a href="${authorityRouteFor(item.id, area)}">${escape(item.id)}</a>`).join('') : '<a href="#authority-engines">Open local catalog</a>'}<small>${entries.length ? `Design references for ${escape(label)}; no active binding.` : 'Local workflow requires no authority engine.'}</small></div></details>`;
}

const presentedEntries = entries => {
  const recommended = entries.find(item => item.id === 'lnsat');
  return recommended ? [recommended, ...entries.filter(item => item !== recommended)] : [...entries];
};
const recommendation = item => item.id === 'lnsat' ? '<span class="authority-recommended">Recommended</span>' : '';
const card = (item, state) => {
  const comparison = getAuthorityComparison(item.id);
  const readiness = getAuthorityReadiness(item.id);
  return `<article class="authority-card${state.selectedId === item.id ? ' authority-card--selected' : ''}" data-authority-card="${escape(item.id)}"><div class="authority-card__identity">${item.logoPath ? `<span class="authority-logo-tile"><img src="${escape(item.logoPath)}" alt="" aria-hidden="true"></span>` : `<span class="authority-monogram" aria-hidden="true">${escape(item.family.slice(0, 2).toUpperCase())}</span>`}<div><p class="analysis-kicker">${escape(item.category)}</p><h3>${escape(item.productTarget)}</h3>${recommendation(item)}</div></div><p class="authority-card__purpose">${escape(comparison?.purpose ?? item.summary)}</p><p class="authority-card__fact">Unavailable · ${readiness?.availableCount ?? 0} / ${readiness?.operationCount ?? 5} available</p><div class="authority-card__foot"><span>${escape(item.badge)}</span><button type="button" class="analysis-button analysis-button--small" data-authority-select="${escape(item.id)}" data-authority-focus="select:${escape(item.id)}" aria-pressed="${state.selectedId === item.id}">Inspect ${escape(item.productTarget)}</button></div></article>`;
};
const unavailable = item => `<section class="authority-module-section"><h4>Adapter details</h4><dl class="authority-matrix"><div><dt>Adapter</dt><dd>Unavailable</dd></div><div><dt>Reason</dt><dd><code>${escape(item.reason)}</code></dd></div><div><dt>Observed version</dt><dd>Unknown</dd></div><div><dt>Compatibility</dt><dd>Unknown</dd></div><div><dt>Connection</dt><dd>Unknown</dd></div><div><dt>Runtime</dt><dd>Unknown</dd></div><div><dt>Rangoon authority</dt><dd>None</dd></div><div><dt>Network attempted</dt><dd>No</dd></div></dl></section>`;
const statusKey = (label, entries) => `<section><h4>${escape(label)}</h4><ul>${entries.map(entry => `<li><span class="authority-status-key__example authority-status-key__example--${escape(entry.tone)}"><span aria-hidden="true">${escape(entry.symbol)}</span>${escape(entry.label)}</span><small>${escape(entry.explanation)}</small></li>`).join('')}</ul></section>`;
const functionDetails = readiness => `<details class="authority-fold authority-function-details" data-authority-function-details><summary>Function details (${readiness.operationCount})</summary><p>Possible Rangoon adapter functions. None are available.</p><ul class="authority-readiness__operations">${readiness.operations.map(operation => { const key = `${readiness.catalogId}-${operation.operation}`; const reasonId = `authority-readiness-${key}-reason`; const detailsId = `authority-readiness-${key}-details`; return `<li><h4>${escape(operation.label)}</h4><button type="button" class="analysis-button analysis-button--small" disabled aria-describedby="${escape(reasonId)}">${escape(operation.actionLabel)}</button><p id="${escape(reasonId)}"><code>${escape(operation.reason)}</code> ${escape(operation.explanation)}</p><details id="${escape(detailsId)}" data-authority-readiness-details><summary>Details: ${escape(operation.label)}</summary><p>${escape(operation.explanation)} <code>${escape(operation.reason)}</code> means no adapter function can check, configure, install, approve, or execute. A separately qualified adapter and host path remain required.</p></details></li>`; }).join('')}</ul></details>`;
const evidenceMeanings = readiness => `<details class="authority-fold authority-evidence" data-authority-evidence><summary>Evidence meanings</summary><div class="authority-readiness__chips"><span class="authority-readiness__chip authority-readiness__chip--neutral" data-authority-readiness-coverage="${escape(readiness.coverage.state)}"><span aria-hidden="true">◇</span>${escape(readiness.coverage.label)}</span><span class="authority-readiness__chip authority-readiness__chip--neutral" data-authority-readiness-control="${escape(readiness.controlAssessment.state)}"><span aria-hidden="true">○</span>${escape(readiness.controlAssessment.label)}</span></div><details class="authority-status-key" data-authority-status-key><summary>Status key</summary><p>Examples show meanings only. They are not current status or evidence.</p><div>${statusKey('Coverage meanings', AUTHORITY_COVERAGE_KEY)}${statusKey('Control assessment meanings', AUTHORITY_CONTROL_KEY)}</div></details></details>`;
const functionMeter = readiness => `<ol class="authority-meter" aria-label="${readiness.availableCount} of ${readiness.operationCount} functions available">${readiness.operations.map(operation => `<li aria-label="${escape(operation.label)}: ${escape(operation.state)}"><span aria-hidden="true"></span><small>${escape(meterLabel[operation.operation] ?? operation.label)}</small></li>`).join('')}</ol>`;
const moduleDetails = (item, chain) => `<details class="authority-fold authority-module-details" data-authority-module-details><summary>Module details</summary><div class="authority-module-section"><h4>Module context</h4><dl class="authority-matrix"><div><dt>Catalog ID</dt><dd><code>${escape(item.id)}</code></dd></div><div><dt>Module context</dt><dd>${escape(chain)}</dd></div></dl><p>Design references only; no provider instance, request, or observation.</p></div><div class="authority-module-section"><h4>Module summary</h4><p>${escape(item.summary)}</p></div><div class="authority-module-section"><h4>Researched abilities</h4><ul>${item.abilities.map(ability => `<li>${escape(ability.label)} <small>${escape(operationLabel(ability.operation))}</small></li>`).join('')}</ul></div><div class="authority-module-section"><h4>Limits and qualification</h4><p>${escape(item.limitations)}</p></div><div class="authority-module-section"><h4>Official research references</h4><ul>${item.references.map(reference => `<li><a href="${escape(reference.url)}" target="_blank" rel="noopener noreferrer">${escape(reference.label)}</a></li>`).join('')}</ul></div>${unavailable(item)}<div class="authority-module-section"><h4>Application map</h4><ul>${item.areas.map(area => `<li><a href="#${escape(area.id)}">${escape(areaById.get(area.id)?.label ?? area.id)}</a><small>${escape(area.relationship)}</small></li>`).join('')}</ul></div></details>`;
const inspector = state => {
  if (state.inspector === 'unavailable') return `<aside class="authority-inspector" aria-labelledby="authority-inspector-title"><header><p class="analysis-kicker">UNAVAILABLE SELECTION</p><h2 id="authority-inspector-title" tabindex="-1">Module unavailable</h2></header><p>Route is not recognized. Choose a catalog module.</p></aside>`;
  if (state.inspector === 'filtered-empty') return `<aside class="authority-inspector" aria-labelledby="authority-inspector-title"><header><p class="analysis-kicker">FILTERED SELECTION</p><h2 id="authority-inspector-title" tabindex="-1">Selected module is hidden</h2></header><p>Clear or change filters to inspect it.</p><button type="button" class="analysis-button" data-authority-clear data-authority-focus="clear-filtered">Clear filters</button></aside>`;
  if (!state.selected) return `<aside class="authority-inspector" aria-labelledby="authority-inspector-title"><header><p class="analysis-kicker">REFERENCE INSPECTOR</p><h2 id="authority-inspector-title" tabindex="-1">Choose a module</h2></header><p>Select a catalog module. Adapters remain unavailable.</p><details class="authority-fold"><summary>Adapter details</summary><dl class="authority-matrix"><div><dt>Catalog schema</dt><dd><code>${escape(AUTHORITY_CATALOG_SCHEMA)}</code></dd></div><div><dt>Rangoon authority</dt><dd>None</dd></div><div><dt>Network attempted</dt><dd>No</dd></div></dl></details></aside>`;
  const item = state.selected;
  const comparison = getAuthorityComparison(item.id);
  const readiness = getAuthorityReadiness(item.id);
  if (!readiness) return '';
  const origin = state.originArea && areaById.get(state.originArea);
  const chain = `${origin ? `${origin.label} → ` : ''}${item.id} → unavailable adapter`;
  return `<aside class="authority-inspector" aria-labelledby="authority-inspector-title"><header class="authority-inspector__identity">${item.logoPath ? `<span class="authority-mark authority-logo-tile"><img src="${escape(item.logoPath)}" alt="" aria-hidden="true"></span>` : `<span class="authority-mark authority-monogram" aria-hidden="true">${escape(item.family.slice(0, 2).toUpperCase())}</span>`}<div><p class="analysis-kicker">${escape(item.family)} / ${escape(item.category)}</p><h2 id="authority-inspector-title" tabindex="-1">${escape(item.productTarget)}</h2>${recommendation(item)}<p>${escape(comparison?.purpose ?? item.summary)}</p></div></header><section class="authority-inspector__availability"><p><strong>No adapter available.</strong> ${readiness.availableCount} of ${readiness.operationCount} functions available.</p>${functionMeter(readiness)}</section>${functionDetails(readiness)}${evidenceMeanings(readiness)}${moduleDetails(item, chain)}${origin ? `<a class="analysis-button analysis-button--small authority-origin" href="#${escape(state.originArea)}">Return to ${escape(origin.label)}</a>` : ''}</aside>`;
};

const authorityResults = state => `${state.entries.length} of 8 modules${state.inspector === 'filtered-empty' ? '; selected module hidden by filters' : ''}.`;
const authorityLibraryKey = state => `${state.selectedId ?? ''}|${presentedEntries(state.entries).map(item => item.id).join(',')}`;
const authorityInspectorKey = state => `${state.inspector}|${state.selectedId ?? ''}|${state.originArea ?? ''}`;
const renderAuthorityLibrary = state => { const entries = presentedEntries(state.entries); return `<section class="authority-library" aria-label="Authority module library">${entries.length ? `<div class="authority-grid">${entries.map(item => card(item, state)).join('')}</div>` : '<div class="authority-empty"><h2>No modules match</h2><p>Clear filters to return to the immutable local catalog.</p><button type="button" class="analysis-button" data-authority-clear data-authority-focus="clear-empty">Clear filters</button></div>'}</section>`; };
const comparisonField = (label, value) => `<div><dt>${label}</dt><dd>${escape(value)}</dd></div>`;
const comparison = item => {
  const record = getAuthorityComparison(item.id);
  if (!record) return '';
  return `<details class="authority-comparison__item" data-authority-comparison="${escape(record.id)}"><summary><span><span class="authority-comparison__cue" aria-hidden="true"></span><strong>${escape(item.productTarget)}</strong><small>${escape(item.badge)} · Research only</small></span><span>${escape(record.purpose)}</span></summary><div class="authority-comparison__body"><dl>${comparisonField('Purpose', record.purpose)}${comparisonField('Permission scope', record.permissions)}${comparisonField('Where controls act', record.placement)}${comparisonField('Approval meaning', record.approval)}${comparisonField('Evidence', record.evidence)}${comparisonField('Privacy', record.privacy)}${comparisonField('Example', record.example)}${comparisonField('Limits', record.limit)}</dl><p class="authority-comparison__sources">Primary research: ${record.references.map(reference => `<a href="${escape(reference.url)}" target="_blank" rel="noopener noreferrer">${escape(reference.label)}</a>`).join(', ')}</p></div></details>`;
};
const authorityComparison = () => `<section class="authority-comparison" aria-labelledby="authority-comparison-title"><header><p class="analysis-kicker">RESEARCH ONLY / NO PROVIDER CONNECTION</p><h2 id="authority-comparison-title">Module references</h2><p>Public research dated <time datetime="${AUTHORITY_RESEARCH_DATE}">${AUTHORITY_RESEARCH_DATE}</time>; not an observed or pinned version. Observed product version: Unknown. Rangoon adapter: Unavailable for every target.</p></header><div class="authority-comparison__list">${AUTHORITY_CATALOG.map(comparison).join('')}</div><details class="authority-glossary"><summary>Comparison glossary</summary><dl>${AUTHORITY_GLOSSARY.map(item => `<div><dt>${escape(item.term)}</dt><dd>${escape(item.meaning)}</dd></div>`).join('')}</dl></details></section>`;

export function renderAuthorityView(rawState) {
  const state = projectAuthorityState(rawState);
  return `<section class="authority-page" aria-labelledby="authority-title"><header class="authority-hero"><span class="authority-hero__diagram" aria-hidden="true">${icon('gate', { size: 34 })}</span><div><p class="analysis-kicker">LOCAL REFERENCE / NO PROVIDER CONNECTION</p><h1 id="authority-title" tabindex="-1">Authority <em>engines</em></h1><p>Compare optional authority and safety modules.</p></div><p class="authority-hero__state"><span>Adapters</span> Unavailable</p></header><div class="authority-layout"><section class="authority-controls" aria-label="Catalog filters"><label>Search catalog <input data-authority-query data-authority-focus="query" type="search" maxlength="160" value="${escape(state.query)}" placeholder="ID, target, or ability"></label><label>Category <select data-authority-category data-authority-focus="category"><option value="all" ${state.category === 'all' ? 'selected' : ''}>All categories</option><option value="gateway" ${state.category === 'gateway' ? 'selected' : ''}>Gateway</option><option value="authorization" ${state.category === 'authorization' ? 'selected' : ''}>Authorization</option><option value="guardrail" ${state.category === 'guardrail' ? 'selected' : ''}>Guardrail</option><option value="action-authority" ${state.category === 'action-authority' ? 'selected' : ''}>Action authority</option></select></label><button type="button" class="analysis-button" data-authority-clear data-authority-focus="clear-primary">Clear filters</button><p aria-live="polite" class="authority-results">${authorityResults(state)}</p></section><div class="authority-workbench">${renderAuthorityLibrary(state)}${inspector(state)}</div><details class="authority-disclosure authority-disclosure--planner"><summary>Plan module roles</summary><div data-authority-planner-root>${renderAuthorityPlanner()}</div></details><details class="authority-disclosure authority-disclosure--comparison"><summary>Compare module references</summary>${authorityComparison()}</details></div></section>`;
}

export function bindAuthorityView({ container, controller, navigate = hash => { location.hash = hash; } }) {
  let disposed = false;
  let initialized = false;
  let libraryKey = '';
  let inspectorKey = '';
  let restoreFocusKey = null;
  let library = null;
  let grid = null;
  let empty = null;
  let cards = new Map();
  let plannerView = null;
  const createEmpty = () => {
    const element = container.ownerDocument.createElement('div');
    element.className = 'authority-empty';
    element.innerHTML = '<h2>No modules match</h2><p>Clear filters to return to the immutable local catalog.</p><button type="button" class="analysis-button" data-authority-clear data-authority-focus="clear-empty">Clear filters</button>';
    return element;
  };
  const bindLibrary = () => {
    library = container.querySelector('.authority-library');
    grid = library?.querySelector('.authority-grid') ?? null;
    cards = new Map([...container.querySelectorAll('[data-authority-card]')].map(element => [element.dataset.authorityCard, element]));
    empty = createEmpty();
  };
  const syncLibrary = state => {
    if (!library || !grid || !empty) { bindLibrary(); if (!library || !grid || !empty) return; }
    const entries = presentedEntries(state.entries);
    const shown = new Set(entries.map(item => item.id));
    for (const [id, element] of cards) {
      if (!shown.has(id) && element.parentNode === grid) element.remove();
      const selected = id === state.selectedId;
      element.classList.toggle('authority-card--selected', selected);
      element.querySelector('[data-authority-select]')?.setAttribute('aria-pressed', String(selected));
    }
    if (!state.entries.length) {
      if (grid.isConnected) grid.remove();
      if (!empty.isConnected) library.append(empty);
      return;
    }
    if (empty.isConnected) empty.remove();
    if (!grid.isConnected) library.append(grid);
    for (const item of entries) grid.append(cards.get(item.id));
  };
  const render = () => {
    if (disposed) return;
    const state = projectAuthorityState(controller.getState());
    if (!initialized) {
      // Seed every immutable card once, including when reentering with filters.
      const initial = projectAuthorityState({ ...state, query: '', category: 'all' });
      container.innerHTML = renderAuthorityView(initial);
      initialized = true;
      libraryKey = authorityLibraryKey(initial);
      inspectorKey = authorityInspectorKey(initial);
      bindLibrary();
      plannerView = bindAuthorityPlanner({ container: container.querySelector('[data-authority-planner-root]'), controller: createAuthorityPlanner(), onInspect: id => navigate(authorityRouteFor(id, controller.getState().originArea)) });
    }
    const query = container.querySelector('[data-authority-query]');
    const category = container.querySelector('[data-authority-category]');
    if (query && query.value !== state.query) query.value = state.query;
    if (category && category.value !== state.category) category.value = state.category;
    const result = container.querySelector('.authority-results');
    if (result) result.textContent = authorityResults(state);
    const nextLibraryKey = authorityLibraryKey(state);
    if (nextLibraryKey !== libraryKey) {
      syncLibrary(state);
      libraryKey = nextLibraryKey;
    }
    const nextInspectorKey = authorityInspectorKey(state);
    if (nextInspectorKey !== inspectorKey) {
      container.querySelector('.authority-inspector')?.replaceWith(container.ownerDocument.createRange().createContextualFragment(inspector(state)));
      inspectorKey = nextInspectorKey;
    }
    if (restoreFocusKey) {
      if (restoreFocusKey.type === 'inspector') {
        if (state.selectedId === restoreFocusKey.id) {
          if (state.inspector === 'selected') container.querySelector('#authority-inspector-title')?.focus();
          restoreFocusKey = null;
        }
      } else {
        container.querySelector(`[data-authority-focus="${restoreFocusKey}"]`)?.focus({ preventScroll: true });
        restoreFocusKey = null;
      }
    }
  };
  const cancelInspectorFocus = () => { if (restoreFocusKey?.type === 'inspector') restoreFocusKey = null; };
  const onInput = event => { if (event.target.matches('[data-authority-query]')) { cancelInspectorFocus(); controller.setQuery(event.target.value); } };
  const onChange = event => { if (event.target.matches('[data-authority-category]')) { cancelInspectorFocus(); controller.setCategory(event.target.value); } };
  const onKeydown = event => { if (event.key === 'Escape' && event.target.matches('[data-authority-query]')) { event.preventDefault(); cancelInspectorFocus(); controller.setQuery(''); } };
  const onClick = event => {
    const select = event.target.closest('[data-authority-select]');
    if (select) {
      const id = select.dataset.authoritySelect;
      restoreFocusKey = { type: 'inspector', id };
      navigate(authorityRouteFor(id, controller.getState().originArea));
      if (controller.getState().selectedId === id) render();
      return;
    }
    if (event.target.closest('[data-authority-clear]')) { restoreFocusKey = 'clear-primary'; controller.clearFilters(); }
  };
  container.addEventListener('input', onInput);
  container.addEventListener('change', onChange);
  container.addEventListener('keydown', onKeydown);
  container.addEventListener('click', onClick);
  render();
  return { render, dispose() { if (!disposed) { disposed = true; plannerView?.dispose(); container.removeEventListener('input', onInput); container.removeEventListener('change', onChange); container.removeEventListener('keydown', onKeydown); container.removeEventListener('click', onClick); } } };
}
