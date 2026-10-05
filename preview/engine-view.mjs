import { ENGINE_OPERATIONS } from './engine-model.mjs';
import { icon } from './icons.mjs';

const operationCopy = {
  negotiate_contract: 'Compare exact contract and feature surface through a qualified transport.',
  inspect_configuration: 'Read an explicitly selected diagnostic configuration view.',
  read_evidence: 'Retrieve one authorized approval or audit record.',
  submit_operation: 'Submit one governed operation through an accepted engine path.',
  reconcile_operation: 'Resolve an uncertain prior effect through provider evidence.',
};

const label = value => value.replaceAll('_', ' ');
const escape = value => String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');

export function renderEngineView(state) {
  const loading = state.status === 'loading';
  const checked = Boolean(state.result);
  const reference = state.result?.reference;
  const noBridge = state.bridgeAvailable === false;
  return `<section class="engine-page" aria-labelledby="engine-title" aria-busy="${loading}">
    <header class="engine-hero"><p class="analysis-kicker">${icon('research',{size:16})} EXPERIMENTAL V0 / LOCAL DIAGNOSTIC</p><h1 id="engine-title" tabindex="-1">LNSAT <em>engine integration</em></h1><p>Rangoon has no LNSAT transport, discovery, launch, credentials, approval, or execution path. This page keeps that boundary visible while source analysis stays useful.</p><div class="analysis-actions"><button id="engine-check" class="analysis-button analysis-button--primary" type="button" ${loading || noBridge ? 'disabled' : ''}>${icon('research',{size:16})}${loading ? 'Checking integration status…' : 'Check integration status'}</button></div><p class="engine-status${state.status === 'failed' ? ' engine-status--error' : ''}">${icon(state.status === 'failed' ? 'unavailable' : 'unknown',{size:16})}${escape(state.message)}</p></header>
    <section class="engine-grid" aria-label="Engine integration details">
      <article class="engine-card engine-card--wide"><div class="engine-card__head"><div><p class="analysis-kicker">${icon('gate',{size:16})} CAPABILITIES</p><h2>Capabilities stay unavailable</h2></div><span class="engine-state">${icon('unavailable',{size:16})}Unavailable</span></div><p class="engine-copy">No result from this page enables an action. Each seam reports <code>adapter_not_implemented</code>.</p><div class="engine-capabilities">${ENGINE_OPERATIONS.map(operation => `<details data-operation="${operation}"><summary><span>${icon('unavailable',{size:16})}${escape(label(operation))}</span><small>Unavailable · adapter_not_implemented</small></summary><p>${escape(operationCopy[operation])}</p></details>`).join('')}</div></article>
      <article class="engine-card"><div class="engine-card__head"><div><p class="analysis-kicker">${icon('source',{size:16})} CURRENT WORK</p><h2>Useful now</h2></div></div><ul class="engine-list"><li>Choose and analyze one local Markdown source.</li><li>Read exact source text and section proposals.</li><li>Save an explicit local source snapshot for later reopening.</li></ul><p class="engine-note">Source analysis and saved sources do not depend on engine status.</p></article>
      <article class="engine-card"><div class="engine-card__head"><div><p class="analysis-kicker">${icon('evidence',{size:16})} QUALIFICATION</p><h2>Still waiting</h2></div></div><ul class="engine-list"><li>Qualified transport and exact contract readback.</li><li>Evidence for authentication, permission and approval.</li><li>Consequence qualification and reconciliation rules.</li></ul><p class="engine-note">Installation and runtime are independently unknown.</p></article>
      <article class="engine-card engine-card--wide"><div class="engine-card__head"><div><p class="analysis-kicker">STATUS BOUNDARY</p><h2>Not checked is not connected</h2></div></div><dl class="engine-fields"><div><dt>Diagnostic source</dt><dd>${checked ? 'Local native response' : noBridge ? 'Browser only; no native result' : loading ? 'Reading local response' : state.status === 'failed' ? 'Local response unavailable or not accepted; host state unknown' : 'No local check requested'}</dd></div><div><dt>Adapter</dt><dd>Placeholder</dd></div><div><dt>Connection</dt><dd>Not attempted</dd></div><div><dt>Runtime</dt><dd>Not checked; unknown</dd></div><div><dt>Installed version</dt><dd>Not checked; unknown</dd></div></dl><p class="engine-note">These are A1 boundary facts. In a browser, they do not describe an installed engine or host runtime.</p></article>
      <article class="engine-card engine-card--wide"><div class="engine-card__head"><div><p class="analysis-kicker">PINNED RESEARCH REFERENCE</p><h2>Pre-release source, not observed support</h2></div></div><dl class="engine-fields"><div><dt>Repository</dt><dd>${escape(reference?.repository ?? 'https://github.com/hypler-dev/LNSAT')}</dd></div><div><dt>Source commit</dt><dd class="analysis-mono">${escape(reference?.sourceCommit ?? 'e09a6b02634b04a46f861ed8b092acc2c2e50fe8')}</dd></div><div><dt>Product source</dt><dd>${escape(reference?.productVersion ?? '0.1.0')} pre-release</dd></div><div><dt>Release support</dt><dd>Not observed; never inferred from this reference</dd></div></dl><p class="engine-note">Pinned source was inspected as research evidence. It is not an installed engine, released artifact, or native readiness result.</p></article>
      <article class="engine-card"><div class="engine-card__head"><div><p class="analysis-kicker">NEXT STEPS</p><h2>Before adapter work</h2></div></div><ol class="engine-steps"><li>Assess supported LNSAT artifacts and interfaces.</li><li>Define validated operation requests and result families.</li><li>Qualify readback and consequences before any action surface.</li></ol></article>
    </section>
  </section>`;
}
