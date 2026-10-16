import { icon } from './icons.mjs';

const compactQuery = '(max-width: 800px)';
const routes = Object.freeze([
  ['Discover', [['analysis', 'import', 'Import & Analyze'], ['decompose', 'decompose', 'Decompose'], ['merge', 'merge', 'Merge'], ['split', 'decompose', 'Split']]],
  ['Build', [['skills', 'skill', 'Skills'], ['workflows', 'workflow', 'Workflows'], ['compile', 'bundle', 'Compile']]],
  ['Connect', [['model-assistance', 'research', 'Model assistance'], ['authority-engines', 'gate', 'Authority engines'], ['operations', 'evidence', 'Operations window'], ['engine', 'evidence', 'LNSAT diagnostic']]],
  ['Workspace', [['workspace', 'bundle', 'Data']]],
]);
const examples = Object.freeze([
  ['sample-design', 'command', 'Sample design preview', 'index.html'],
  ['workflow-playground', 'workflow', 'Workflow playground', 'workflow-playground.html'],
  ['flow-playground', 'workflow', 'Flow playground', 'flow-playground.html'],
]);
const areas = new Map(routes.flatMap(([, links]) => links).map(([id, , label]) => [id, label]));
const surfaces = new Set(['workbench', 'sample-design', 'workflow-playground', 'flow-playground']);

export const WORKBENCH_NAVIGATION = Object.freeze({ routes, examples, compactQuery });

export function normalizeWorkbenchArea(area) {
  return areas.has(area) ? area : 'analysis';
}

export function normalizeWorkbenchSurface(surface) {
  return surfaces.has(surface) ? surface : 'workbench';
}

export function renderWorkbenchNavigation({ area, surface, theme = 'dark', supplementary = null, workspaceGuard = null, open = true } = {}) {
  const currentArea = normalizeWorkbenchArea(area);
  const currentSurface = normalizeWorkbenchSurface(surface);
  const renderLink = ([id, glyph, label]) => {
    const href = currentSurface === 'workbench' ? `#${id}` : `analyze.html#${id}`;
    return `<a href="${href}"${currentSurface === 'workbench' && currentArea === id ? ' aria-current="page"' : ''}>${icon(glyph, { size: 16 })}<span>${label}</span></a>`;
  };
  const renderExample = ([id, glyph, label, href]) => `<a href="${href}"${currentSurface === id ? ' aria-current="page"' : ''}>${icon(glyph, { size: 16 })}<span>${label}</span></a>`;
  const currentLabel = currentSurface === 'workbench' ? areas.get(currentArea) : examples.find(([id]) => id === currentSurface)?.[2] ?? 'Navigation';
  const brandHref = currentSurface === 'workbench' ? '#analysis' : 'analyze.html#analysis';
  const extra = typeof supplementary === 'function' ? supplementary() : '';
  const guard = currentSurface === 'workbench' && typeof workspaceGuard === 'function' ? workspaceGuard() : '';
  const themeControl = currentSurface === 'workbench' ? 'id="analysis-theme"'
    : currentSurface === 'workflow-playground' ? 'id="playground-theme"'
      : currentSurface === 'flow-playground' ? 'data-flow-theme' : 'data-workbench-theme';
  return `<aside class="analysis-rail" data-workbench-nav><a class="analysis-brand" href="${brandHref}"><img src="assets/brand-symbol.png" alt=""><span>Rangoon.si</span></a><nav class="analysis-nav" aria-label="Task navigation"><details class="analysis-nav-drawer" data-workbench-nav-drawer${open ? ' open' : ''}><summary><span>${currentLabel}</span><span>Navigation</span></summary><div class="analysis-nav-groups">${routes.map(([label, links]) => `<section class="analysis-nav-group" aria-label="${label}"><p>${label}</p>${links.map(renderLink).join('')}</section>`).join('')}${extra ? `<div class="analysis-nav-supplementary">${extra}</div>` : ''}<section class="analysis-nav-group analysis-nav-group--sample" aria-label="Examples"><p>Examples</p>${examples.map(renderExample).join('')}</section></div></details></nav>${guard}<div class="analysis-companion"><img src="assets/mascot.webp" width="90" height="60" alt=""><p>Small pieces.<br>Greater possibilities.</p></div><button ${themeControl} class="analysis-theme" type="button">${icon('theme', { size: 16 })}<span>${theme === 'light' ? 'Dark mode' : 'Light mode'}</span></button></aside>`;
}

export function captureWorkbenchNavigation(root, documentRef = root?.ownerDocument ?? globalThis.document) {
  const drawer = root?.querySelector?.('[data-workbench-nav-drawer]');
  const active = documentRef?.activeElement;
  const link = active?.closest?.('[data-workbench-nav] a[href]');
  return { open: Boolean(drawer?.open), focus: active?.matches?.('[data-workbench-nav-drawer] > summary') ? 'summary' : link?.getAttribute?.('href') ?? null };
}

export function restoreWorkbenchNavigation(root, state, { compact = false, documentRef = root?.ownerDocument ?? globalThis.document } = {}) {
  const drawer = root?.querySelector?.('[data-workbench-nav-drawer]');
  if (!drawer) return;
  drawer.open = compact ? Boolean(state?.open) : true;
  const target = state?.focus === 'summary'
    ? drawer.querySelector?.('summary')
    : [...(root.querySelectorAll?.('[data-workbench-nav] a[href]') ?? [])].find(link => link.getAttribute('href') === state?.focus);
  if (target && (!compact || drawer.open)) target.focus?.({ preventScroll: true });
  else if (state?.focus && compact) drawer.querySelector?.('summary')?.focus?.({ preventScroll: true });
  void documentRef;
}

export function bindWorkbenchNavigation(root, { window: windowRef = globalThis.window, document: documentRef = root?.ownerDocument ?? globalThis.document } = {}) {
  const media = windowRef?.matchMedia?.(compactQuery);
  const compact = () => Boolean(media?.matches);
  const keydown = event => {
    if (!compact() || event.key !== 'Escape') return;
    const drawer = event.target?.closest?.('[data-workbench-nav-drawer][open]');
    if (!drawer || !root?.contains?.(drawer)) return;
    drawer.open = false;
    drawer.querySelector?.('summary')?.focus?.({ preventScroll: true });
    event.preventDefault?.();
  };
  const change = event => {
    const drawer = root?.querySelector?.('[data-workbench-nav-drawer]');
    if (!drawer) return;
    if (event.matches) {
      const active = documentRef?.activeElement;
      if (drawer.querySelector?.('.analysis-nav-groups')?.contains?.(active)) drawer.querySelector?.('summary')?.focus?.({ preventScroll: true });
      drawer.open = false;
    } else drawer.open = true;
  };
  const pagehide = event => { if (!event.persisted) binding.dispose(); };
  root?.addEventListener?.('keydown', keydown);
  media?.addEventListener?.('change', change);
  windowRef?.addEventListener?.('pagehide', pagehide);
  const binding = {
    compact,
    capture: () => captureWorkbenchNavigation(root, documentRef),
    restore: state => restoreWorkbenchNavigation(root, state, { compact: compact(), documentRef }),
    dispose() { root?.removeEventListener?.('keydown', keydown); media?.removeEventListener?.('change', change); windowRef?.removeEventListener?.('pagehide', pagehide); },
  };
  return binding;
}
