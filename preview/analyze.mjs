import { createAnalysisController, escapeText } from './analysis-model.mjs';

const app = document.querySelector('#analysis-app');
const announcer = document.querySelector('#analysis-announcer');
const bridge = typeof window.__TAURI__?.core?.invoke === 'function'
  ? command => window.__TAURI__.core.invoke(command)
  : undefined;

let theme = 'dark';
try {
  const savedTheme = localStorage.getItem('rangoon-theme');
  if (savedTheme === 'light' || savedTheme === 'dark') theme = savedTheme;
} catch {}
document.documentElement.dataset.theme = theme;
let requestedFocus = null;

const text = value => escapeText(value);
const formatBytes = value => new Intl.NumberFormat().format(value ?? 0);
const span = fragment => `bytes ${fragment.span.startByte}–${fragment.span.endByte} · lines ${fragment.span.startLine}–${fragment.span.endLine}`;
const title = fragment => fragment.heading ? `${'#'.repeat(fragment.heading.level)} ${fragment.heading.title}` : 'Preamble';

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
  if (!report) return `<section class="analysis-empty" aria-label="No analysis loaded" aria-busy="${state.status === 'pending'}"><p class="analysis-kicker">LOCAL ONLY</p><h2>Pick one Markdown file.</h2><p>Choose a file, then read its sections and inspect the original text here.</p></section>`;
  const fragments = report.fragments ?? [];
  const diagnostics = report.diagnostics ?? [];
  const active = fragments.find(fragment => fragment.id === state.selectedFragmentId) ?? fragments[0];
  return `<div class="analysis-workbench" aria-busy="${state.status === 'pending'}">
    <aside class="analysis-panel analysis-fragments" aria-label="Source sections">
      <div class="analysis-panel__head"><div><p class="analysis-kicker">SOURCE MAP</p><h2>Read source sections</h2></div><span>${fragments.length} sections</span></div>
      <div class="analysis-fragment-list">${fragments.length ? fragments.map(fragment => `<button type="button" class="analysis-fragment${fragment.id === active?.id ? ' analysis-fragment--active' : ''}" data-fragment="${text(fragment.id)}" aria-pressed="${fragment.id === active?.id}"><strong>${text(title(fragment))}</strong><small>${text(span(fragment))}</small></button>`).join('') : '<p class="analysis-note">The analyzer returned no fragments.</p>'}</div>
      <div class="analysis-panel__foot">Review state: <strong>${text(active?.reviewState ?? 'unreviewed')}</strong><br>Authority: <strong>${text(report.authority)}</strong></div>
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

function render(state) {
  const unavailable = state.status === 'unavailable';
  const pending = state.status === 'pending';
  const hasReport = Boolean(state.report);
  const error = ['rejected', 'failed'].includes(state.status);
  app.innerHTML = `<div class="analysis-shell">
    <aside class="analysis-rail"><a class="analysis-brand" href="index.html"><img src="assets/brand-symbol.png" alt=""><span>Rangoon<span>.</span></span></a><nav class="analysis-nav" aria-label="Desktop areas"><a href="#analysis-main" aria-current="page">Import &amp; Analyze</a><a href="index.html">Sample design preview</a></nav><button id="analysis-theme" class="analysis-theme" type="button">${theme === 'light' ? 'Dark mode' : 'Light mode'}</button></aside>
    <main id="analysis-main" tabindex="-1">
      <section class="analysis-hero"><p class="analysis-kicker">DESKTOP / SOURCE ANALYSIS</p><h1>Import &amp; <em>Analyze</em></h1><p>Choose one Markdown file. Read source sections. Inspect original text. Results stay in memory for this app session.</p><div class="analysis-actions"><button id="analysis-choose" class="analysis-button analysis-button--primary" type="button" ${unavailable || pending ? 'disabled' : ''}>${pending ? 'Waiting for selection and analysis…' : 'Choose Markdown file'}</button><button id="analysis-clear" class="analysis-button" type="button" ${hasReport || pending ? '' : 'disabled'}>Clear analysis</button></div><p id="analysis-status" class="analysis-status${error ? ' analysis-status--error' : ''}">${text(unavailable ? 'Open this page in the Rangoon desktop app to choose and analyze a local file.' : state.message)}</p>${error ? `<p class="analysis-alert" role="alert">${text(state.message)}${state.errorCode ? ` Error code: ${text(state.errorCode)}.` : ''}${hasReport ? ' Current analysis remains available.' : ''}</p>` : ''}</section>
      <section class="analysis-truth" aria-label="Analysis boundaries"><span>Native picker only</span><span>No scan or upload</span><span>Memory-only result</span><span>Sections are proposals, not semantic skills</span></section>
      ${reportView(state)}
    </main>
  </div>`;
  announcer.textContent = unavailable ? 'Open this page in the Rangoon desktop app to choose and analyze a local file.' : state.message;
  if (!pending && requestedFocus === 'choose' && !unavailable) document.querySelector('#analysis-choose')?.focus();
  if (!pending && requestedFocus?.startsWith('fragment:')) document.querySelector(`[data-fragment="${CSS.escape(requestedFocus.slice(9))}"]`)?.focus();
}

const controller = createAnalysisController({ invoke: bridge, onChange: render });
render(controller.getState());

app.addEventListener('click', event => {
  const fragment = event.target.closest('[data-fragment]');
  if (fragment) { requestedFocus = `fragment:${fragment.dataset.fragment}`; return controller.selectFragment(fragment.dataset.fragment); }
  if (event.target.closest('#analysis-choose')) { requestedFocus = 'choose'; return controller.choose(); }
  if (event.target.closest('#analysis-clear')) { requestedFocus = 'choose'; return controller.clear(); }
  if (event.target.closest('#analysis-theme')) {
    requestedFocus = 'theme';
    theme = theme === 'light' ? 'dark' : 'light';
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem('rangoon-theme', theme); } catch {}
    render(controller.getState());
    document.querySelector('#analysis-theme')?.focus();
  }
});
