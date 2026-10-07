import { createAnalysisController, escapeText } from './analysis-model.mjs';
import { createEngineController } from './engine-model.mjs';
import { renderEngineView } from './engine-view.mjs';
import { createSkillsController, validCapabilityContent, validCapabilityTitle } from './skills-model.mjs';
import { renderSkillsView } from './skills-view.mjs';
import { createWorkspaceController } from './workspace-model.mjs';
import { bindWorkspaceView } from './workspace-view.mjs';
import { createCompositionController } from './composition-model.mjs';
import { bindCompositionView } from './composition-view.mjs';
import { createCompilationController } from './compilation-model.mjs';
import { renderCompilationPage } from './compilation-view.mjs';
import { createModelAssistanceController } from './model-assistance-model.mjs';
import { bindModelAssistanceView } from './model-assistance-view.mjs';
import { createCloudCredentialController } from './cloud-credential-model.mjs';
import { icon } from './icons.mjs';
import { createStartupSplash } from './launch.mjs';

const app = document.querySelector('#analysis-app');
const announcer = document.querySelector('#analysis-announcer');
const startupSplash = createStartupSplash({ container: document.querySelector('#startup-splash'), app });
const rawBridge = typeof window.__TAURI__?.core?.invoke === 'function'
  ? (command, args) => window.__TAURI__.core.invoke(command, args)
  : undefined;
const compositionControllers = new Map();
let compilationController = null;
let modelController = null;
let modelView = null;
let cloudCredentialController = null;
let nativeOperations = 0;
const workspaceWrites = new Set(['save_analysis', 'create_capability', 'revise_capability', 'review_capability', 'restore_workspace_backup', 'delete_workspace_record']);
function invalidateCompositions(message, except = null) {
  for (const [operation, composition] of compositionControllers) {
    if (operation !== except) composition.invalidatePreview(message);
  }
}
const bridge = rawBridge ? async (command, args) => {
  if (workspaceWrites.has(command)) invalidateCompositions('Workspace data may have changed. Preview this retained draft again before saving.');
  if (workspaceWrites.has(command) || command === 'commit_composition') { compilationController?.invalidate(); modelController?.invalidate(); }
  nativeOperations += 1;
  if (compilationController && route === 'compile') renderCompilation();
  try { return await rawBridge(command, args); }
  finally {
    nativeOperations -= 1;
    if (compilationController && route === 'compile') renderCompilation();
  }
} : undefined;
const compositionRoutes = new Set(['decompose', 'merge', 'split']);
const routeFromHash = () => {
  const name = location.hash.slice(1);
  return ['analysis', 'skills', 'compile', 'workspace', 'model-assistance', 'engine', ...compositionRoutes].includes(name) ? name : 'analysis';
};

const compactNavigation = window.matchMedia('(max-width:760px)');
compactNavigation.addEventListener('change', event => {
  const drawer = document.querySelector('.analysis-nav-drawer');
  if (!drawer) return;
  if (event.matches && drawer.contains(document.activeElement)) drawer.querySelector('summary')?.focus();
  drawer.open = !event.matches;
});
let theme = 'dark';
try {
  const savedTheme = localStorage.getItem('rangoon-theme');
  if (savedTheme === 'light' || savedTheme === 'dark') theme = savedTheme;
} catch {}
document.documentElement.dataset.theme = theme;
let requestedFocus = null;
let renderedSourceId = null;
let route = routeFromHash();
let retainedSourceScroll = null;
let routeFocusPending = route;
let engineCheckFocusPending = false;
let routeEntry = true;
let openEngineDetails = new Set();
let skillsActionFocus = null;
let skillsOriginalOpen = false;
let skillsSavedOpen = false;
let skillsCreateFocusPending = false;
let analysisReady = false;
let workspaceView = null;
let compositionView = null;
let compositionViewRoute = null;
let compilationActionFocus = null;

const text = value => escapeText(value);
const formatBytes = value => new Intl.NumberFormat().format(value ?? 0);
const span = fragment => `bytes ${fragment.span.startByte}–${fragment.span.endByte} · lines ${fragment.span.startLine}–${fragment.span.endLine}`;
const title = fragment => fragment.heading ? `${'#'.repeat(fragment.heading.level)} ${fragment.heading.title}` : 'Preamble';
const savedName = snapshot => String(snapshot.displayName ?? 'Untitled source').split(/[\\/]/).pop() || 'Untitled source';
const digest = snapshot => String(snapshot.sha256 ?? 'Unavailable');
const savedAt = snapshot => {
  const value = Number(snapshot.savedAtMs);
  if (!Number.isFinite(value)) return 'Date unavailable';
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value));
};

function renderRail() {
  const sameRoute = document.querySelector('.analysis-shell')?.dataset.area === route;
  const drawerOpen = !compactNavigation.matches || (sameRoute && document.querySelector('.analysis-nav-drawer')?.open === true);
  const groups = [
    ['Discover', [['analysis','import','Import &amp; Analyze'],['decompose','decompose','Decompose'],['merge','merge','Merge'],['split','decompose','Split']]],
    ['Build', [['skills','skill','Skills'],['compile','bundle','Compile']]],
    ['Connect', [['model-assistance','research','Model assistance'],['engine','gate','Engine integration']]],
    ['Workspace', [['workspace','bundle','Data']]],
  ];
  const item = ([name, symbol, label]) => `<a href="#${name}" ${route === name ? 'aria-current="page"' : ''}>${icon(symbol,{size:16})}<span>${label}</span></a>`;
  return `<aside class="analysis-rail"><a class="analysis-brand" href="index.html"><img src="assets/brand-symbol.png" alt=""><span>Rangoon</span></a><nav class="analysis-nav" aria-label="Desktop areas"><details class="analysis-nav-drawer" ${drawerOpen ? 'open' : ''}><summary>Navigate areas</summary><div class="analysis-nav-groups">${groups.map(([label, links]) => `<section class="analysis-nav-group" aria-label="${label}"><p>${label}</p>${links.map(item).join('')}</section>`).join('')}<section class="analysis-nav-group analysis-nav-group--sample" aria-label="Preview"><p>Preview</p><a href="index.html">${icon('command',{size:16})}<span>Sample design preview</span></a></section></div></details></nav><div class="analysis-companion"><img src="assets/mascot.webp" width="90" height="60" alt=""><p>Small pieces.<br>Greater possibilities.</p></div><button id="analysis-theme" class="analysis-theme" type="button">${icon('theme',{size:16})}${theme === 'light' ? 'Dark mode' : 'Light mode'}</button></aside>`;
}

function renderModelAssistance() {
  if (!modelController) return;
  if (!document.querySelector('#model-assistance-root')) {
    modelView?.dispose();
    app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1"><div id="model-assistance-root"></div></main></div>`;
    modelView = bindModelAssistanceView(document.querySelector('#model-assistance-root'),modelController,cloudCredentialController);
  } else modelView.render();
  routeEntry = false;
  document.querySelector('#analysis-theme').innerHTML = `${icon('theme',{size:16})}${theme === 'light' ? 'Dark mode' : 'Light mode'}`;
  const message = modelController.getState().message;
  if (announcer.textContent !== message) announcer.textContent = message;
  if (routeFocusPending === route) {
    document.querySelector('#model-assistance-title')?.focus({preventScroll:true});
    routeFocusPending = null;
  }
}

function renderComposition() {
  const composition = compositionControllers.get(route);
  if (!composition) return;
  if (!document.querySelector('#composition-root') || compositionViewRoute !== route) {
    compositionView?.dispose();
    app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1"><div id="composition-root"></div></main></div>`;
    compositionViewRoute = route;
    compositionView = bindCompositionView(document.querySelector('#composition-root'), composition, {
      onOpenSkill: id => { location.hash = '#skills'; void skillsController.open(id); },
    });
  } else compositionView.render();
  routeEntry = false;
  document.querySelector('#analysis-theme').innerHTML = `${icon('theme',{size:16})}${theme === 'light' ? 'Dark mode' : 'Light mode'}`;
  const state = composition.getState();
  announcer.textContent = state.message;
  if (routeFocusPending === route) {
    document.querySelector('#composition-root h1')?.focus({ preventScroll: true });
    routeFocusPending = null;
  }
  if (state.status === 'idle') void composition.load();
}

function renderWorkspace() {
  if (!document.querySelector('#workspace-root')) {
    workspaceView?.dispose();
    app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1"><div id="workspace-root"></div></main></div>`;
    workspaceView = bindWorkspaceView(document.querySelector('#workspace-root'), workspaceController);
  } else workspaceView.render();
  routeEntry = false;
  document.querySelector('#analysis-theme').innerHTML = `${icon('theme',{size:16})}${theme === 'light' ? 'Dark mode' : 'Light mode'}`;
  const state = workspaceController.getState();
  announcer.textContent = state.message;
  if (routeFocusPending === 'workspace') {
    if (!state.dialog) document.querySelector('#workspace-title')?.focus({ preventScroll: true });
    routeFocusPending = null;
  }
  if (state.status === 'idle') void workspaceController.load();
}

function sourceLines(report, active) {
  const activeSpan = report.fragments?.find(fragment => fragment.id === active)?.span;
  const lines = report.source.content ? report.source.content.split('\n') : [];
  if (lines.at(-1) === '') lines.pop();
  if (!lines.length) return '<p class="analysis-note">The returned source is empty.</p>';
  return lines.map((line, index) => {
    const number = index + 1;
    const selected = activeSpan && number >= activeSpan.startLine && number <= activeSpan.endLine;
    return `<span class="analysis-line${selected ? ' analysis-line--selected' : ''}"><b>${number}</b><code>${text(line) || ' '}</code></span>`;
  }).join('');
}

function reportView(state) {
  const report = state.report;
  if (!report) return `<section class="analysis-empty" aria-label="No analysis loaded" aria-busy="${state.status === 'pending'}"><img class="analysis-empty-mascot" src="assets/mascot.webp" width="120" height="80" alt=""><p class="analysis-kicker">LOCAL ONLY</p><h2>Pick one Markdown file.</h2><p>Choose a file, then read its sections and inspect the original text here.</p></section>`;
  const fragments = report.fragments ?? [];
  const diagnostics = report.diagnostics ?? [];
  const active = fragments.find(fragment => fragment.id === state.selectedFragmentId) ?? fragments[0];
  const skills = skillsController.getState();
  const derived = new Set(skills.capabilities.filter(item => item.origin.kind === 'source' && item.origin.sourceId === report.source.id).map(item => item.origin.fragmentId));
  const coverage = skills.listStatus === 'ready' ? `${fragments.filter(f => derived.has(f.id)).length} / ${fragments.length} sections linked to skills` : 'Skill coverage unavailable';
  const canCreate = active && state.snapshotsStatus === 'ready' && state.snapshots.some(s => s.sourceId === report.source.id) && !state.busyAction;

  return `<div class="analysis-workbench" aria-busy="${state.status === 'pending'}">
    <aside class="analysis-panel analysis-fragments" aria-label="Source sections">
      <div class="analysis-panel__head"><div><p class="analysis-kicker">SOURCE MAP</p><h2>Read source sections</h2></div><span>${fragments.length} sections</span></div>
      <div class="analysis-fragment-list">${fragments.length ? fragments.map(fragment => `<button type="button" class="analysis-fragment${fragment.id === active?.id ? ' analysis-fragment--active' : ''}" data-fragment="${text(fragment.id)}" aria-pressed="${fragment.id === active?.id}"><strong>${text(title(fragment))}</strong><small>${text(span(fragment))}</small></button>`).join('') : '<p class="analysis-note">The analyzer returned no fragments.</p>'}</div>
      <div class="analysis-panel__foot">Review state: <strong>${text(active?.reviewState ?? 'unreviewed')}</strong><br>Authority: <strong>${text(report.authority)}</strong><p>${text(coverage)}. Derivation does not prove meaning was preserved.</p><button id="analysis-create-skill" class="analysis-button analysis-button--small" type="button" ${canCreate ? '' : 'disabled'}>Create skill from section</button>${!canCreate ? '<p>Save this source before creating a skill.</p>' : ''}</div>
    </aside>
    <section class="analysis-panel analysis-source" aria-labelledby="analysis-source-title">
      <div class="analysis-panel__head"><div><p class="analysis-kicker">ORIGINAL SOURCE</p><h2 id="analysis-source-title">Inspect original text</h2></div><span class="analysis-mono">${text(report.source.displayName)}</span></div>
      <div class="analysis-code" aria-label="Original source with line numbers">${sourceLines(report, active?.id)}</div>
    </section>
    <aside class="analysis-panel analysis-inspector" aria-labelledby="analysis-inspector-title">
      <div class="analysis-panel__head"><div><p class="analysis-kicker">INSPECTOR</p><h2 id="analysis-inspector-title">Report digest</h2></div></div>
      <dl class="analysis-fields">
        <div><dt>Schema</dt><dd>${text(report.schemaVersion)}</dd></div>
        <div><dt>Analyzer</dt><dd>${text(report.analyzerVersion)}</dd></div>
        <div><dt>Bytes</dt><dd>${formatBytes(report.source.byteLength)}</dd></div>
        <div><dt>Lines</dt><dd>${formatBytes(report.source.lineCount)}</dd></div>
        <div><dt>Format</dt><dd>${text(report.source.format)}</dd></div>
        <div><dt>SHA-256</dt><dd class="analysis-mono">${text(report.source.sha256)}</dd></div>
        <div><dt>Source ID</dt><dd class="analysis-mono">${text(report.source.id)}</dd></div>
        ${active ? `<div><dt>Fragment</dt><dd class="analysis-mono">${text(active.id)}</dd></div><div><dt>Span</dt><dd>${text(span(active))}</dd></div>` : ''}
        ${state.errorCode ? `<div><dt>Error code</dt><dd class="analysis-mono">${text(state.errorCode)}</dd></div>` : ''}
      </dl>
      <div class="analysis-diagnostics"><h3>Diagnostics</h3>${diagnostics.length ? diagnostics.map(diagnostic => `<div><strong>${text(diagnostic.code)}</strong><p>${text(diagnostic.message)}${diagnostic.line ? ` (line ${text(diagnostic.line)})` : ''}</p></div>`).join('') : '<p>No diagnostics returned.</p>'}</div>
    </aside>
  </div>`;
}

function snapshotsView(state) {
  const snapshots = state.snapshots ?? [];
  const loading = state.snapshotsStatus === 'loading';
  const failure = state.snapshotsStatus === 'failed';
  const unavailable = state.snapshotsStatus === 'unavailable';
  const openDisabled = state.busyAction || !snapshots.length;
  const count = state.snapshotsStatus === 'ready' ? `${snapshots.length} / 128`
    : loading ? 'Loading…'
      : failure ? (snapshots.length ? 'List unavailable · prior results' : 'List unavailable')
        : unavailable ? 'Desktop app only'
          : 'Loading…';
  const contents = snapshots.length ? snapshots.map(snapshot => `<article class="analysis-snapshot"><div><h3>${text(savedName(snapshot))}</h3><p class="analysis-mono">${text(digest(snapshot))}</p><small>${text(savedAt(snapshot))} · ${formatBytes(snapshot.byteLength)} bytes</small></div><button class="analysis-button analysis-button--small" type="button" data-open-snapshot="${text(snapshot.sourceId)}" aria-label="Open saved source ${text(savedName(snapshot))}, SHA-256 ${text(digest(snapshot))}" ${openDisabled ? 'disabled' : ''}>Open</button></article>`).join('')
    : loading ? '<p class="analysis-note">Loading saved sources…</p>'
      : unavailable ? '<p class="analysis-note">Open this page in the Rangoon desktop app to view saved sources.</p>'
        : failure ? `<p class="analysis-note">Saved source list is unavailable.${state.report ? ' Current analysis remains available.' : ''}</p>`
          : '<p class="analysis-note">No saved sources yet.</p>';
  return `<aside class="analysis-snapshots" aria-labelledby="analysis-snapshots-title" aria-busy="${loading}">
    <div class="analysis-snapshots__head"><div><p class="analysis-kicker">SAVED SOURCES</p><h2 id="analysis-snapshots-title">Open a saved source</h2></div><span>${count}</span></div>
    <p class="analysis-snapshots__copy">Saved sources stay on this computer. Opening one restores its original text to this window.</p>
    ${failure ? `<div class="analysis-snapshots__error" role="alert"><p>${text(state.snapshotsError?.message ?? 'Saved sources could not load.')}${state.snapshotsError?.code ? ` Error code: ${text(state.snapshotsError.code)}.` : ''}</p><button id="analysis-retry-snapshots" class="analysis-button analysis-button--small" type="button">Retry saved list</button></div>` : ''}
    <div class="analysis-snapshot-list" aria-label="Saved sources">${contents}</div>
  </aside>`;
}

function revealSelectedLine() {
  const selectedLine = document.querySelector('.analysis-line--selected');
  const sourcePane = selectedLine?.closest('.analysis-code');
  if (!selectedLine || !sourcePane) return;
  const lineRect = selectedLine.getBoundingClientRect();
  const paneRect = sourcePane.getBoundingClientRect();
  sourcePane.scrollTop += lineRect.top - paneRect.top - Math.max(0, (sourcePane.clientHeight - selectedLine.clientHeight) / 2);
}

function render(state) {
  document.title = route === 'model-assistance' ? 'Rangoon — Model assistance' : compositionRoutes.has(route) ? `Rangoon — ${route[0].toUpperCase()}${route.slice(1)}` : route === 'compile' ? 'Rangoon — Compile inspection' : route === 'workspace' ? 'Rangoon — Workspace data' : route === 'engine' ? 'Rangoon — Engine integration' : route === 'skills' ? 'Rangoon — Local capabilities' : 'Rangoon — Import & Analyze';
  if (!compositionRoutes.has(route)) {
    compositionView?.dispose();
    compositionView = null;
    compositionViewRoute = null;
  }
  if (route === 'model-assistance') { renderModelAssistance(); return; }
  modelView?.dispose();
  modelView = null;
  if (route === 'workspace') { renderWorkspace(); return; }
  workspaceView?.dispose();
  workspaceView = null;
  if (route === 'compile') { renderCompilation(); return; }
  if (compositionRoutes.has(route)) { renderComposition(); return; }
  if (route === 'engine') {
    renderEngine();
    return;
  }
  if (route === 'skills') {
    renderSkills();
    return;
  }
  const previousFragments = document.querySelector('.analysis-fragment-list');
  const previousSource = document.querySelector('.analysis-code');
  const fragmentScrollTop = previousFragments?.scrollTop ?? 0;
  const sourceScrollTop = retainedSourceScroll?.top ?? previousSource?.scrollTop ?? 0;
  const sourceScrollLeft = retainedSourceScroll?.left ?? previousSource?.scrollLeft ?? 0;
  const sourceId = state.report?.source?.id ?? null;
  const sourceChanged = sourceId !== renderedSourceId;
  const unavailable = state.status === 'unavailable';
  const pending = Boolean(state.busyAction);
  const hasReport = Boolean(state.report);
  const error = ['rejected', 'failed'].includes(state.status);
  const saveOrOpen = state.busyAction === 'save' || state.busyAction === 'open';
  const clearDisabled = !hasReport || saveOrOpen || state.busyAction === 'clear';
  const chooseDisabled = unavailable || pending;
  const saveDisabled = !hasReport || pending;
  app.innerHTML = `<div class="analysis-shell" data-area="${route}">
    ${renderRail()}
    <main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1">
      <section class="analysis-hero"><p class="analysis-kicker">DESKTOP / SOURCE ANALYSIS</p><h1 id="analysis-title" tabindex="-1">Import &amp; <em>Analyze</em></h1><p>Choose one Markdown file. Read source sections. Inspect original text. Results stay in this window until cleared. Save locally to reopen after restart.</p><div class="analysis-actions"><button id="analysis-choose" class="analysis-button analysis-button--primary" type="button" ${chooseDisabled ? 'disabled' : ''}>${icon('import',{size:16})}${state.busyAction === 'choose' ? 'Waiting for selection and analysis…' : 'Choose Markdown file'}</button><button id="analysis-clear" class="analysis-button" type="button" ${clearDisabled ? 'disabled' : ''}>${icon('clear',{size:16})}${state.busyAction === 'clear' ? 'Clearing analysis…' : 'Clear analysis'}</button></div>${hasReport ? `<div class="analysis-save"><p>Stores original source text unencrypted on this computer. No upload. Saved does not mean reviewed.</p><button id="analysis-save" class="analysis-button analysis-button--save" type="button" ${saveDisabled ? 'disabled' : ''}>${icon('save',{size:16})}${state.busyAction === 'save' ? 'Saving locally…' : 'Save locally'}</button></div>` : ''}<p id="analysis-status" class="analysis-status${error ? ' analysis-status--error' : ''}">${text(unavailable ? 'Open this page in the Rangoon desktop app to choose and analyze a local file.' : state.message)}</p>${error ? `<p class="analysis-alert" role="alert">${text(state.message)}${state.errorCode ? ` Error code: ${text(state.errorCode)}.` : ''}${hasReport ? ' Current analysis remains available.' : ''}</p>` : ''}</section>
      <section class="analysis-truth" aria-label="Analysis boundaries"><span>Native picker only</span><span>No scan or upload</span><span>Local save available</span><span>Sections are proposals, not semantic skills</span></section>
      ${snapshotsView(state)}
      ${reportView(state)}
    </main>
  </div>`;
  renderedSourceId = sourceId;
  routeEntry = false;
  retainedSourceScroll = null;
  if (!sourceChanged) {
    const nextFragments = document.querySelector('.analysis-fragment-list');
    const nextSource = document.querySelector('.analysis-code');
    if (nextFragments) nextFragments.scrollTop = fragmentScrollTop;
    if (nextSource) {
      nextSource.scrollTop = sourceScrollTop;
      nextSource.scrollLeft = sourceScrollLeft;
    }
  }
  announcer.textContent = unavailable ? 'Open this page in the Rangoon desktop app to choose and analyze a local file.' : state.message;
  if (routeFocusPending === 'analysis') {
    document.querySelector('#analysis-title')?.focus({ preventScroll: true });
    routeFocusPending = null;
  }
  if (!pending && requestedFocus === 'choose' && !unavailable) document.querySelector('#analysis-choose')?.focus({ preventScroll: true });
  if (!pending && requestedFocus === 'save') document.querySelector('#analysis-save')?.focus({ preventScroll: true });
  if (!pending && requestedFocus?.startsWith('snapshot:')) document.querySelector(`[data-open-snapshot="${CSS.escape(requestedFocus.slice(9))}"]`)?.focus({ preventScroll: true });
  if (!pending && requestedFocus?.startsWith('fragment:')) {
    document.querySelector(`[data-fragment="${CSS.escape(requestedFocus.slice(9))}"]`)?.focus({ preventScroll: true });
    revealSelectedLine();
  }
}

function retainSourceScroll() {
  const source = document.querySelector('.analysis-code');
  if (source) retainedSourceScroll = { top: source.scrollTop, left: source.scrollLeft };
}

function renderEngine() {
  const existingDetails = document.querySelectorAll('.engine-capabilities details[open]');
  openEngineDetails = new Set([...existingDetails].map(detail => detail.dataset.operation).filter(Boolean));
  const state = engineController.getState();
  app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1">${renderEngineView(state)}</main></div>`;
  routeEntry = false;
  for (const operation of openEngineDetails) document.querySelector(`[data-operation="${operation}"]`)?.setAttribute('open', '');
  announcer.textContent = state.message;
  if (routeFocusPending === 'engine') {
    document.querySelector('#engine-title')?.focus({ preventScroll: true });
    routeFocusPending = null;
  } else if (engineCheckFocusPending && !loadingEngineStatus(state)) {
    document.querySelector('#engine-check')?.focus({ preventScroll: true });
    engineCheckFocusPending = false;
  }
}

function renderSkills() {
  const active = document.activeElement;
  const focus = active?.id ? `#${CSS.escape(active.id)}`
    : active?.dataset?.capability ? `[data-capability="${CSS.escape(active.dataset.capability)}"]`
      : active?.dataset?.revision ? `[data-revision="${CSS.escape(active.dataset.revision)}"]` : null;
  const selection = typeof active?.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd] : null;
  const editorScroll = document.querySelector('#skills-content-input')?.scrollTop ?? 0;
  const libraryScroll = document.querySelector('.skills-library__list')?.scrollTop ?? 0;
  const original = document.querySelector('.skills-original');
  if (original) skillsOriginalOpen = original.open;
  const saved = document.querySelector('.skills-saved');
  if (saved) skillsSavedOpen = saved.open;
  const state = skillsController.getState();
  app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1">${renderSkillsView(state)}</main></div>`;
  routeEntry = false;
  announcer.textContent = state.message;
  if (skillsOriginalOpen) document.querySelector('.skills-original')?.setAttribute('open', '');
  if (skillsSavedOpen) document.querySelector('.skills-saved')?.setAttribute('open', '');
  const textarea = document.querySelector('#skills-content-input');
  if (textarea) textarea.scrollTop = editorScroll;
  const library = document.querySelector('.skills-library__list');
  if (library) library.scrollTop = libraryScroll;
  if (routeFocusPending === 'skills') {
    document.querySelector(skillsCreateFocusPending ? '#skills-create-title' : '#skills-title')?.focus({ preventScroll: true });
    skillsCreateFocusPending = false;
    routeFocusPending = null;
  } else {
    const target = !state.pending && skillsActionFocus ? skillsActionFocus : focus;
    let element = target ? document.querySelector(target) : null;
    if (element?.disabled) element = document.querySelector('#skills-editor-title');
    if (!element && skillsActionFocus) element = document.querySelector('#skills-create-title');
    element?.focus({ preventScroll: true });
    if (selection && element === document.querySelector(focus ?? ':not(*)') && typeof element?.setSelectionRange === 'function') element.setSelectionRange(...selection);
    if (!state.pending) skillsActionFocus = null;
  }
}

function loadingEngineStatus(state) { return state.status === 'loading'; }

function renderCompilation() {
  if (!compilationController) return;
  const state = compilationController.getState();
  const active = document.activeElement;
  const focus = active?.id ? `#${CSS.escape(active.id)}`
    : ['compileCapability', 'compileRevision', 'compileProfile', 'compileTab'].map(key => {
      const attribute = key.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`);
      return active?.dataset?.[key] ? `[data-${attribute}="${CSS.escape(active.dataset[key])}"]` : null;
    }).find(Boolean);
  const scroll = [...document.querySelectorAll('.compile-page pre[id], .compile-selection[id], .compile-capability-list[id], .compile-revisions[id]')].map(element => [element.id, element.scrollTop, element.scrollLeft]);
  const openDetails = [...document.querySelectorAll('.compile-page details[open][id]')].map(element => element.id);
  app.innerHTML = `<div class="analysis-shell" data-area="${route}">${renderRail()}<main id="analysis-main" class="${routeEntry ? 'route-enter' : ''}" tabindex="-1">${renderCompilationPage(state, { busy: nativeOperations > 0, skillsBusy: Boolean(skillsController.getState().pending) })}</main></div>`;
  routeEntry = false;
  for (const [id, top, left] of scroll) {
    const element = id && document.getElementById(id);
    if (element) { element.scrollTop = top; element.scrollLeft = left; }
  }
  for (const id of openDetails) document.getElementById(id)?.setAttribute('open', '');
  announcer.textContent = state.error || state.needsRefresh || state.bundleExport?.error || state.externalBundle?.error ? '' : state.message;
  if (routeFocusPending === 'compile') {
    document.querySelector('#compile-title')?.focus({ preventScroll: true });
    routeFocusPending = null;
  } else {
    const target = !state.pending && nativeOperations === 0 && compilationActionFocus ? compilationActionFocus : focus;
    const element = target ? document.querySelector(target) : null;
    if (element && !element.disabled) element.focus({ preventScroll: true });
    if (!state.pending && nativeOperations === 0) compilationActionFocus = null;
  }
}

modelController = createModelAssistanceController({ invoke: bridge, onChange: () => { if (route === 'model-assistance') renderModelAssistance(); } });
cloudCredentialController = createCloudCredentialController({ invoke: bridge, onChange: () => { if (route === 'model-assistance') renderModelAssistance(); } });
const engineController = createEngineController({ invoke: bridge, onChange: () => {
  if (route === 'engine') renderEngine();
} });
const skillsController = createSkillsController({ invoke: bridge, onChange: () => {
  if (route === 'skills') renderSkills();
  else if (analysisReady && route === 'compile') renderCompilation();
  else if (analysisReady && route === 'analysis') {
    const active = document.activeElement;
    const selector = active?.id ? `#${CSS.escape(active.id)}`
      : active?.dataset?.fragment ? `[data-fragment="${CSS.escape(active.dataset.fragment)}"]`
        : active?.dataset?.openSnapshot ? `[data-open-snapshot="${CSS.escape(active.dataset.openSnapshot)}"]`
          : active?.matches?.('a[href]') ? `a[href="${CSS.escape(active.getAttribute('href'))}"]` : null;
    render(controller.getState());
    // Passive library completion must not move focus to an earlier source action.
    if (selector) document.querySelector(selector)?.focus({ preventScroll: true });
  }
} });
compilationController = createCompilationController({ invoke: bridge, onChange: () => {
  if (route === 'compile') renderCompilation();
} });
const workspaceController = createWorkspaceController({
  invoke: bridge,
  onChange: () => { if (route === 'workspace') renderWorkspace(); },
  getDraftWarning: (kind, id) => kind === 'capability' && skillsController.hasUnsavedDraft(id),
  onMutation: async event => {
    if (event.kind === 'delete') {
      if (event.recordKind === 'capability') skillsController.forgetDeleted(event.id);
      else if (event.recordKind === 'source') controller.noteSnapshotDeleted(event.id);
    }
    await Promise.all([controller.listSnapshots(), skillsController.list()]);
  },
});
const controller = createAnalysisController({ invoke: bridge, onChange: state => {
  skillsController.setSourceContext({
    report: state.report ? { ...state.report, selectedFragmentId: state.selectedFragmentId } : null,
    snapshots: state.snapshots,
    snapshotsStatus: state.snapshotsStatus,
  });
  render(state);
  startupSplash.sync(state);
} });
for (const operation of compositionRoutes) {
  compositionControllers.set(operation, createCompositionController({
    operation,
    invoke: bridge ? async (command, args) => {
      if (command === 'preview_composition' || command === 'commit_composition') {
        invalidateCompositions('Another composition used the native preview session. Preview this retained draft again before saving.', operation);
      }
      return bridge(command, args);
    } : undefined,
    onChange: () => { if (route === operation) renderComposition(); },
    onCommitted: async () => {
      invalidateCompositions('Composition saved. Preview other retained drafts against the current workspace before saving.', operation);
      await skillsController.list();
    },
  }));
}
analysisReady = true;
startupSplash.setRetry(() => controller.listSnapshots());
render(controller.getState());
void skillsController.list();
if (route === 'compile') void compilationController.refresh();
if (route === 'model-assistance') void modelController.load();

app.addEventListener('click', event => {
  const compileControl = event.target.closest('#compile-refresh, #compile-generate, #compile-open-skills, #compile-export, #compile-inspect, #compile-inspect-external, [data-compile-capability], [data-compile-revision], [data-compile-profile], [data-compile-tab]');
  if (compileControl) {
    if (compileControl.disabled) return;
    const state = compilationController.getState();
    if (compileControl.dataset.compileTab) return compilationController.setTab(compileControl.dataset.compileTab);
    if (nativeOperations || state.pending) return;
    if (compileControl.id === 'compile-export') {
      compilationActionFocus = '#compile-export';
      return compilationController.exportBundle().then(exported => {
        if (route !== 'compile') return;
        const target = document.querySelector(exported ? '#compile-export-title' : '#compile-export');
        (target?.disabled ? document.querySelector('#compile-refresh') : target)?.focus({ preventScroll: true });
      });
    }
    if (compileControl.id === 'compile-inspect' || compileControl.id === 'compile-inspect-external') {
      const launcher = `#${compileControl.id}`;
      compilationActionFocus = launcher;
      return compilationController.inspectBundle().then(inspected => {
        if (route === 'compile') document.querySelector(inspected ? '#compile-external-title' : launcher)?.focus({ preventScroll: true });
      });
    }
    if (compileControl.id === 'compile-refresh') { compilationActionFocus = '#compile-refresh'; return compilationController.refresh(); }
    if (compileControl.dataset.compileCapability) { compilationActionFocus = `[data-compile-capability="${CSS.escape(compileControl.dataset.compileCapability)}"]`; return compilationController.open(compileControl.dataset.compileCapability); }
    if (compileControl.dataset.compileRevision && state.selected) { compilationActionFocus = `[data-compile-revision="${CSS.escape(compileControl.dataset.compileRevision)}"]`; return compilationController.open(state.selected.id, compileControl.dataset.compileRevision); }
    if (compileControl.dataset.compileProfile) return compilationController.selectProfile(compileControl.dataset.compileProfile);
    if (compileControl.id === 'compile-generate') {
      compilationActionFocus = '#compile-generate';
      return compilationController.compile().then(() => {
        if (route === 'compile' && compilationController.getState().report && !compilationController.getState().error) document.querySelector('#compile-artifact-title')?.focus({ preventScroll: true });
      });
    }
    if (compileControl.id === 'compile-open-skills' && state.selected && !skillsController.getState().pending) {
      const selected = state.selected;
      location.hash = '#skills';
      return skillsController.open(selected.id, selected.revision.id).then(() => {
        if (route === 'skills') document.querySelector('#skills-editor-title')?.focus({ preventScroll: true });
      });
    }
    return;
  }
  if (event.target.closest('#engine-check')) { engineCheckFocusPending = true; return engineController.check(); }
  const capability = event.target.closest('[data-capability]');
  if (capability) { skillsActionFocus = '#skills-editor-title'; return skillsController.open(capability.dataset.capability); }
  const revision = event.target.closest('[data-revision]');
  if (revision && skillsController.getState().selected) { skillsActionFocus = `[data-revision="${CSS.escape(revision.dataset.revision)}"]`; return skillsController.open(skillsController.getState().selected.id, revision.dataset.revision); }
  if (event.target.closest('#skills-create')) { skillsActionFocus = '#skills-editor-title'; return skillsController.create(skillsController.getState().createTitle); }
  if (event.target.closest('#skills-save')) { skillsActionFocus = '#skills-save'; return skillsController.save(); }
  if (event.target.closest('#skills-review')) { skillsActionFocus = '#skills-review'; return skillsController.review(); }
  if (event.target.closest('#skills-reload-list')) return skillsController.list();
  if (event.target.closest('#skills-return-current') || event.target.closest('#skills-reload-current')) {
    skillsActionFocus = '#skills-editor-title';
    if (event.target.closest('#skills-reload-current')) { skillsSavedOpen = true; document.querySelector('.skills-saved')?.setAttribute('open', ''); return skillsController.reloadCurrent(); }
    const selected = skillsController.getState().selected;
    if (selected) return skillsController.open(selected.id, null);
  }
  if (event.target.closest('#analysis-create-skill')) { skillsCreateFocusPending = true; location.hash = '#skills'; return; }
  const fragment = event.target.closest('[data-fragment]');
  if (fragment) { requestedFocus = `fragment:${fragment.dataset.fragment}`; return controller.selectFragment(fragment.dataset.fragment); }
  const snapshot = event.target.closest('[data-open-snapshot]');
  if (snapshot) { requestedFocus = `snapshot:${snapshot.dataset.openSnapshot}`; return controller.openSnapshot(snapshot.dataset.openSnapshot); }
  if (event.target.closest('#analysis-choose')) { requestedFocus = 'choose'; return controller.choose(); }
  if (event.target.closest('#analysis-clear')) { requestedFocus = 'choose'; return controller.clear(); }
  if (event.target.closest('#analysis-save')) { requestedFocus = 'save'; return controller.save(); }
  if (event.target.closest('#analysis-retry-snapshots')) return controller.listSnapshots();
  if (event.target.closest('#analysis-theme')) {
    requestedFocus = 'theme';
    theme = theme === 'light' ? 'dark' : 'light';
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem('rangoon-theme', theme); } catch {}
    render(controller.getState());
    document.querySelector('#analysis-theme')?.focus({ preventScroll: true });
  }
});

app.addEventListener('keydown', event => {
  const tab = event.target.closest('[data-compile-tab]');
  if (!tab || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  const tabs = ['text', 'diagnostics', 'evidence', 'external'];
  const index = tabs.indexOf(tab.dataset.compileTab);
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
  compilationActionFocus = `[data-compile-tab="${tabs[next]}"]`;
  compilationController.setTab(tabs[next]);
  document.querySelector(`[data-compile-tab="${tabs[next]}"]`)?.focus({ preventScroll: true });
});

app.addEventListener('input', event => {
  if (event.target.matches('#skills-create-title')) {
    skillsController.updateCreateTitle(event.target.value);
    const state = skillsController.getState();
    document.querySelector('#skills-create').disabled = !state.bridgeAvailable || !state.source?.saved || state.pending || state.needsReload || !validCapabilityTitle(state.createTitle);
    document.querySelector('#skills-create-count').textContent = `${new TextEncoder().encode(event.target.value).length} / 160 UTF-8 bytes. Source stays unreviewed; count does not prove preservation.`;
    return;
  }
  if (!event.target.matches('#skills-title-input, #skills-content-input')) return;
  skillsController.updateDraft(event.target.matches('#skills-title-input') ? { title: event.target.value } : { content: event.target.value });
  const skills = skillsController.getState();
  const draft = skills.draft;
  const titleOK = validCapabilityTitle(draft?.title);
  const contentOK = validCapabilityContent(draft?.content);
  document.querySelector('#skills-title-count').textContent = `${new TextEncoder().encode(draft?.title ?? '').length} / 160 UTF-8 bytes${titleOK ? '' : ' · use one nonblank line'}`;
  document.querySelector('#skills-content-count').textContent = `${new TextEncoder().encode(draft?.content ?? '').length} / 262144 UTF-8 bytes${contentOK ? '' : ' · content cannot be blank or contain NUL'}`;
  const save = document.querySelector('#skills-save');
  const review = document.querySelector('#skills-review');
  const blocked = !skills.bridgeAvailable || skills.pending || skills.needsReload || skills.viewedRevisionId !== skills.selected?.latestRevisionId;
  if (save) save.disabled = Boolean(blocked || !draft?.dirty || !titleOK || !contentOK);
  if (review) review.disabled = Boolean(blocked || draft?.dirty || !titleOK || !contentOK || skills.selected?.revision.review);
  const draftStatus = document.querySelector('#skills-draft-status');
  if (draftStatus) draftStatus.textContent = draft?.dirty ? 'Unsaved edits. Save a revision before review.' : 'Local content review does not validate, publish or authorize execution.';
});

window.addEventListener('hashchange', () => {
  retainSourceScroll();
  const nextRoute = routeFromHash();
  routeEntry = route !== nextRoute;
  route = nextRoute;
  routeFocusPending = route;
  requestedFocus = null;
  engineCheckFocusPending = false;
  skillsActionFocus = null;
  compilationActionFocus = null;
  render(controller.getState());
  if (route === 'workspace') void workspaceController.load();
  if (route === 'model-assistance' && modelController.getState().status === 'idle') void modelController.load();
  if (route === 'compile' && compilationController.getState().listStatus === 'idle') void compilationController.refresh();
  if (compositionRoutes.has(route) && compositionControllers.get(route).getState().status !== 'idle') void compositionControllers.get(route).load();
});

window.addEventListener('beforeunload', event => {
  if (![...compositionControllers.values()].some(composition => composition.getState().dirty)) return;
  event.preventDefault();
  event.returnValue = '';
});
