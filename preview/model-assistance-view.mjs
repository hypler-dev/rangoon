import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';
import { cloudCredentialAction, renderCloudCredentialView } from './cloud-credential-view.mjs';

const text = escapeText;
const unknown = value => value === undefined || value === null || value === '' ? 'Unknown' : text(value);
const number = value => Number.isFinite(Number(value)) ? Number(value).toLocaleString() : 'Unknown';
const enabled = state => state.bridgeAvailable === true;
const busy = state => Boolean(state.pending || state.draining);
const action = (controller, name, ...args) => {
  if (typeof controller[name] === 'function') void controller[name](...args);
};
const records = state => [
  ...(Array.isArray(state.snapshots) ? state.snapshots : []).map(item => ({ kind: 'source', sourceId: item.sourceId, title: item.displayName, sha256: item.sha256, byteLength: item.byteLength })),
  ...(Array.isArray(state.capabilities) ? state.capabilities : []).map(item => ({ kind: 'capability', capabilityId: item.id, title: item.title, revisionId: item.latestRevisionId, revisionCount: item.revisionCount }))
];
const selection = state => ({ task: state.task ?? 'classify_v1', inputs: state.inputs ?? [] });
const selected = (state, record) => (selection(state).inputs ?? []).some(input => input.kind === record.kind && input.sourceId === record.sourceId && input.capabilityId === record.capabilityId && input.revisionId === record.revisionId);

function profile(state) {
  const value = state.profile ?? state.session?.profile ?? {};
  const canUse = enabled(state) && !busy(state);
  return `<section class="model-panel model-profile" aria-labelledby="model-profile-title"><header><div><p class="analysis-kicker">SESSION-ONLY PROFILE</p><h2 id="model-profile-title">${icon('connector', { size: 20 })} Local endpoint</h2></div><span class="model-state">${enabled(state) ? 'Desktop session' : 'Browser preview'}</span></header><p class="model-note">Profile, payload and response stay in memory for this session. Reachability never proves model availability, trust or local-only processing.</p><form id="model-profile-form" class="model-fields"><label>Profile ID<input name="profileId" type="text" autocomplete="off" value="${text(value.profileId ?? 'local')}" ${canUse ? '' : 'disabled'}></label><label>Loopback host<input name="host" type="text" autocomplete="off" value="${text(value.host ?? '127.0.0.1')}" placeholder="127.0.0.1 or ::1" ${canUse ? '' : 'disabled'}></label><label>Port<input name="port" type="number" min="1" max="65535" inputmode="numeric" value="${text(value.port ?? '')}" ${canUse ? '' : 'disabled'}></label><label>Model<input id="model-name" name="model" type="text" maxlength="128" autocomplete="off" value="${text(value.model ?? '')}" placeholder="family:model" ${canUse ? '' : 'disabled'}></label><label>Output token cap<input name="maxOutputTokens" type="number" min="1" max="32768" inputmode="numeric" value="${text(value.maxOutputTokens ?? '')}" ${canUse ? '' : 'disabled'}></label><div class="model-profile-actions"><button id="model-configure" class="analysis-button analysis-button--primary" type="submit" ${canUse ? '' : 'disabled'}>${busy(state) && state.pending === 'configure' ? 'Configuring…' : 'Configure session'}</button><button id="model-clear" class="analysis-button" type="button" ${enabled(state) ? '' : 'disabled'}>Clear session</button></div></form><dl class="model-facts"><div><dt>Processing</dt><dd>Unknown</dd></div><div><dt>Retention</dt><dd>Unknown</dd></div><div><dt>Location</dt><dd>Unknown</dd></div><div><dt>Cost</dt><dd>Unknown</dd></div></dl></section>`;
}

function selector(state) {
  const task = selection(state).task ?? 'classify_v1';
  const inputCount = (selection(state).inputs ?? []).length;
  const canUse = enabled(state) && !busy(state);
  const list = records(state).length ? records(state).map((record, index) => {
    const checked = selected(state, record);
    const capability = record.kind === 'capability';
    const id = capability ? record.capabilityId : record.sourceId;
    const label = capability ? `${record.title ?? 'Saved capability'} · explicit revision` : record.title ?? 'Saved source';
    const revision = capability ? record.revisionId : record.sha256;
    const detail = capability ? state.selectedCapabilities?.[record.capabilityId] : null;
    const capabilityControl = capability && !detail ? `<button class="model-record-load" type="button" data-model-capability="${index}" ${canUse ? '' : 'disabled'}>Load explicit revisions</button>` : '';
    const revisions = detail?.history ? `<div class="model-revisions">${detail.history.map((revisionItem, revisionIndex) => `<label><input type="checkbox" data-model-revision="${index}:${revisionIndex}" ${selected(state, { kind: 'capability', capabilityId: record.capabilityId, revisionId: revisionItem.id }) ? 'checked' : ''} ${canUse ? '' : 'disabled'}><span class="analysis-mono">${text(revisionItem.id)}</span></label>`).join('')}</div>` : '';
    return `<div class="model-record"><label><input type="checkbox" data-model-record="${index}" ${checked ? 'checked' : ''} ${capability || !canUse ? 'disabled' : ''}><span><strong>${text(label)}</strong><small class="analysis-mono">${text(record.kind ?? 'record')} · ${text(id ?? 'Unavailable')}</small><small class="analysis-mono">${text(revision ?? 'Unavailable')}</small></span></label>${capabilityControl}${revisions}</div>`;
  }).join('') : '<p class="model-empty">No saved inputs are available. Browser preview never invents workspace records.</p>';
  return `<section class="model-panel model-selection" aria-labelledby="model-selection-title"><header><div><p class="analysis-kicker">EXACT SAVED INPUTS</p><h2 id="model-selection-title">${icon('source', { size: 20 })} Select records</h2></div><span>${inputCount} selected</span></header><div class="model-refresh"><button id="model-reload" class="analysis-button" type="button" ${canUse ? '' : 'disabled'}>Refresh saved records</button></div><div class="model-task"><label for="model-task">Directed task</label><select id="model-task" ${canUse ? '' : 'disabled'}><option value="classify_v1" ${task === 'classify_v1' ? 'selected' : ''}>Classify · 1–16 records</option><option value="decompose_v1" ${task === 'decompose_v1' ? 'selected' : ''}>Decompose · 1–16 records</option><option value="compare_v1" ${task === 'compare_v1' ? 'selected' : ''}>Compare · exactly 2 revisions</option></select></div><div id="model-records" class="model-records">${list}</div><p class="model-note">Only selected saved records are included. Complete text is preserved; oversized requests are rejected.</p><div class="model-selection-actions"><button id="model-sourcefree-check" class="analysis-button" type="button" ${canUse ? '' : 'disabled'}>${state.pending === 'check' ? 'Checking…' : 'Check endpoint without source'}</button><button id="model-prepare" class="analysis-button analysis-button--primary" type="button" ${canUse && Boolean(state.session?.profile) && inputCount > 0 && (task !== 'compare_v1' || inputCount === 2) ? '' : 'disabled'}>${state.pending === 'prepare' ? 'Preparing…' : 'Prepare exact payload'}</button></div></section>`;
}

function payload(state) {
  const prepared = state.prepared ?? state.session?.prepared ?? null;
  const inputs = prepared?.inputs ?? selection(state).inputs ?? [];
  const body = prepared?.bodyJson ?? '';
  const coverage = inputs.length ? inputs.map(dependency => {
    const input = dependency.input ?? dependency;
    return `<li><strong>${text(input.kind ?? 'record')}</strong><span class="analysis-mono">${text(input.sourceId ?? input.capabilityId ?? 'Unavailable')}</span>${input.revisionId ? `<span class="analysis-mono">revision ${text(input.revisionId)}</span>` : ''}<span class="analysis-mono">SHA-256 ${text(input.sha256 ?? input.digest ?? 'Unavailable')}</span><span>${number(dependency.byteLength)} bytes · protected [0, ${number(dependency.byteLength)})</span>${dependency.observedHead ? `<span class="analysis-mono">observed head ${text(dependency.observedHead)}</span>` : ''}</li>`;
  }).join('') : '<li>No prepared records. Select saved records and prepare locally.</li>';
  const facts = prepared ? `<dl class="model-facts model-payload-facts"><div><dt>Origin</dt><dd>${unknown(prepared.origin)}</dd></div><div><dt>Model</dt><dd>${unknown(prepared.model)}</dd></div><div><dt>Pack ID</dt><dd class="analysis-mono">${unknown(prepared.pack?.packId)}</dd></div><div><dt>Prepared ID</dt><dd class="analysis-mono">${unknown(prepared.preparedId)}</dd></div><div><dt>Request ID</dt><dd class="analysis-mono">${unknown(prepared.requestId)}</dd></div><div><dt>Body bytes</dt><dd>${number(prepared.bodyBytes ?? body.length)}</dd></div><div><dt>Tokens</dt><dd>Unknown</dd></div></dl>` : '<p class="model-empty">Prepare retains exact bytes locally. It performs no network request.</p>';
  return `<section class="model-panel model-payload" aria-labelledby="model-payload-title"><header><div><p class="analysis-kicker">EXACT PAYLOAD AND COVERAGE</p><h2 id="model-payload-title">${icon('bundle', { size: 20 })} Inspect before consent</h2></div><span>${prepared ? 'Prepared locally' : 'Not prepared'}</span></header><div class="model-tabs" role="tablist" aria-label="Model payload panels"><button id="model-tab-payload" type="button" role="tab" aria-selected="${(state.tab ?? 'payload') === 'payload'}" aria-controls="model-panel-payload" tabindex="${(state.tab ?? 'payload') === 'payload' ? '0' : '-1'}" data-model-tab="payload">Payload</button><button id="model-tab-coverage" type="button" role="tab" aria-selected="${state.tab === 'coverage'}" aria-controls="model-panel-coverage" tabindex="${state.tab === 'coverage' ? '0' : '-1'}" data-model-tab="coverage">Coverage</button><button id="model-tab-facts" type="button" role="tab" aria-selected="${state.tab === 'facts'}" aria-controls="model-panel-facts" tabindex="${state.tab === 'facts' ? '0' : '-1'}" data-model-tab="facts">Facts</button></div><section id="model-panel-payload" role="tabpanel" aria-labelledby="model-tab-payload" ${state.tab && state.tab !== 'payload' ? 'hidden' : ''}><pre id="model-payload-body" class="model-payload-body" tabindex="0">${text(body || 'No prepared payload.')}</pre></section><section id="model-panel-coverage" role="tabpanel" aria-labelledby="model-tab-coverage" ${state.tab === 'coverage' ? '' : 'hidden'}><ol class="model-coverage">${coverage}</ol></section><section id="model-panel-facts" role="tabpanel" aria-labelledby="model-tab-facts" ${state.tab === 'facts' ? '' : 'hidden'}>${facts}</section></section>`;
}

function result(state) {
  const completionState = state.completion ?? null;
  const completion = completionState?.completion ?? null;
  const current = completionState?.freshness === 'current';
  const canSend = enabled(state) && !busy(state) && Boolean(state.prepared);
  const message = state.message ? `<p class="model-status" role="status">${text(state.message)}</p>` : '';
  const error = state.error ? `<p class="model-alert" role="alert">${text(state.error.message ?? 'Model assistance could not complete.')}${state.error.code ? ` Error code: ${text(state.error.code)}.` : ''}</p>` : '';
  const proposal = proposalItem => `<article class="model-proposal"><h4>${text(proposalItem.title ?? 'Untitled proposal')}</h4><p>${text(proposalItem.kind ?? 'advisory')}</p><h5>Authored output</h5><pre tabindex="0">${text(proposalItem.authoredText ?? '')}</pre><h5>Explanation</h5><pre tabindex="0">${text(proposalItem.explanation ?? '')}</pre><h5>Citations</h5><ul class="model-citations">${Array.isArray(proposalItem.citations) && proposalItem.citations.length ? proposalItem.citations.map(item => `<li class="analysis-mono">${text(item.input?.sourceId ?? item.input?.capabilityId ?? 'Unavailable')} ${text(item.input?.revisionId ?? '')} SHA-256 ${text(item.input?.sha256 ?? 'Unavailable')} [${number(item.startByte)}, ${number(item.endByte)})</li>`).join('') : '<li>No citations returned.</li>'}</ul></article>`;
  const outputHtml = completion ? `<article class="model-output"><h3>Advisory output</h3><p class="model-advisory">Authority: none. This response cannot review, approve, execute, navigate, or apply changes.</p><dl class="model-output-facts"><div><dt>Freshness</dt><dd>${text(completionState?.freshness ?? 'unavailable')}</dd></div><div><dt>Pack ID</dt><dd class="analysis-mono">${unknown(completion.packId)}</dd></div><div><dt>Response SHA-256</dt><dd class="analysis-mono">${unknown(completion.responseSha256)}</dd></div></dl>${Array.isArray(completion.proposals?.proposals) && completion.proposals.proposals.length ? completion.proposals.proposals.map(proposal).join('') : '<p class="model-empty">The validated result has no proposals.</p>'}<h4>Uncertainty</h4><ul class="model-citations">${Array.isArray(completion.proposals?.uncertainties) && completion.proposals.uncertainties.length ? completion.proposals.uncertainties.map(item => `<li>${text(item)}</li>`).join('') : '<li>No uncertainty entries returned.</li>'}</ul><p class="model-note">Model-authored links and markup are displayed as inert text. Proposal application is unavailable.</p></article>` : '<p class="model-empty">No advisory result. Prepared payloads remain local until native consent completes.</p>';
  return `<aside class="model-panel model-results" aria-labelledby="model-results-title"><header><div><p class="analysis-kicker">ADVISORY RESULT</p><h2 id="model-results-title" tabindex="-1">${icon('outcome', { size: 20 })} Review outcome</h2></div><span>${text(state.status ?? state.pending ?? 'Idle')}</span></header><div class="model-result-actions"><button id="model-send" class="analysis-button analysis-button--primary" type="button" ${canSend ? '' : 'disabled'}>${state.pending === 'send' ? 'Review or request active…' : 'Review & send'}</button><button id="model-cancel" class="analysis-button" type="button" ${enabled(state) && busy(state) && state.activeRunId ? '' : 'disabled'}>Cancel</button></div><p class="model-note">Review & send opens native confirmation for retained exact bytes. Cancel requests cancellation; it cannot recall text already accepted by a remote endpoint.</p><p class="model-note">Proposal application is unavailable. Model output has no review, approval or execution authority.</p>${!enabled(state) ? '<p class="model-alert" role="status">Native model assistance is unavailable in browser preview. No endpoint call was made.</p>' : ''}${message}${error}${completion && !current ? '<p class="model-stale">Completion remains advisory. Freshness is not current.</p>' : ''}${outputHtml}</aside>`;
}

export function renderModelAssistanceView(state = {}, cloudState = {}) {
  return `<section class="model-assistance-page" aria-labelledby="model-assistance-title" aria-busy="${busy(state)}"><header class="model-assistance-hero"><p class="analysis-kicker">MODEL ASSISTANCE / OPTIONAL ADVISORY</p><h1 id="model-assistance-title" tabindex="-1">Keep source <em>in view.</em></h1><p>Prepare exact saved text for an optional, explicitly confirmed model request. Model output is untrusted advisory text with no application authority.</p></header><div class="model-workbench">${profile(state)}${selector(state)}${payload(state)}${result(state)}${renderCloudCredentialView(cloudState)}</div></section>`;
}

export function bindModelAssistanceView(root, controller, cloudController = null) {
  let lastGeneration = null;
  let returnFocus = null;
  let wasBusy = false;
  let cloudReturnFocus = null;
  let cloudWasBusy = false;
  const render = () => {
    const state = controller.getState();
    const cloudBusy = Boolean(cloudController?.getState().pending);
    const active = root.ownerDocument?.activeElement;
    const focus = active && root.contains(active) ? active.id ? { id: active.id } : active.dataset?.modelRecord ? { record: active.dataset.modelRecord } : active.dataset?.modelRevision ? { revision: active.dataset.modelRevision } : active.name ? { name: active.name } : null : null;
    const draft = lastGeneration === state.generation ? [...root.querySelectorAll('#model-profile-form input')].map(input => [input.name,input.value]) : [];
    const scroll = ['model-payload-body','model-records'].map(id => [id,root.querySelector(`#${id}`)?.scrollTop ?? 0,root.querySelector(`#${id}`)?.scrollLeft ?? 0]);
    if (busy(state) && !wasBusy && focus?.id) returnFocus = focus.id;
    if (cloudBusy && !cloudWasBusy && focus?.id?.startsWith('cloud-credential-')) cloudReturnFocus = focus.id;
    root.innerHTML = renderModelAssistanceView(state, cloudController?.getState());
    lastGeneration = state.generation;
    for (const [name,value] of draft) { const input = root.querySelector(`[name="${name}"]`); if (input) input.value = value; }
    for (const [id,top,left] of scroll) { const pane = root.querySelector(`#${id}`); if (pane) { pane.scrollTop = top; pane.scrollLeft = left; } }
    const target = focus?.id ? root.querySelector(`#${focus.id}`) : focus?.record ? root.querySelector(`[data-model-record="${focus.record}"]`) : focus?.revision ? root.querySelector(`[data-model-revision="${focus.revision}"]`) : focus?.name ? root.querySelector(`[name="${focus.name}"]`) : null;
    if (target && !target.disabled) target.focus({preventScroll:true});
    else if (focus?.id?.startsWith('cloud-credential-') && cloudBusy) root.querySelector('#cloud-credential-title')?.focus({preventScroll:true});
    else if (focus && busy(state)) root.querySelector('#model-results-title')?.focus({preventScroll:true});
    if (wasBusy && !busy(state) && returnFocus) {
      const current = root.ownerDocument?.activeElement;
      if (!current || current === root.ownerDocument?.body || current.id === 'model-results-title') {
        const prior = root.querySelector(`#${returnFocus}`);
        if (prior && !prior.disabled) prior.focus({preventScroll:true});
        else root.querySelector('#model-results-title')?.focus({preventScroll:true});
      }
      returnFocus = null;
    }
    if (cloudWasBusy && !cloudBusy && cloudReturnFocus) {
      const current = root.ownerDocument?.activeElement;
      if (!current || current === root.ownerDocument?.body || current.id === 'cloud-credential-title') {
        const prior = root.querySelector(`#${cloudReturnFocus}`);
        const recovery = prior && !prior.disabled ? prior : root.querySelector('#cloud-credential-inspect');
        recovery?.focus({preventScroll:true});
      }
      cloudReturnFocus = null;
    }
    cloudWasBusy = cloudBusy;
    wasBusy = busy(state);
  };
  const click = event => {
    const target = event.target.closest('button');
    if (!target || target.disabled) return;
    if (cloudController && cloudCredentialAction(target, cloudController)) return;
    if (target.matches('#model-reload')) return action(controller, 'load');
    if (target.matches('#model-clear')) return action(controller, 'clear');
    if (target.matches('#model-sourcefree-check')) return action(controller, 'check');
    if (target.matches('#model-prepare')) return action(controller, 'prepare');
    if (target.matches('#model-send')) return action(controller, 'send');
    if (target.matches('#model-cancel')) return action(controller, 'cancel');
    if (target.dataset.modelTab) return action(controller, 'setTab', target.dataset.modelTab);
    if (target.dataset.modelCapability) {
      const item = records(controller.getState())[Number(target.dataset.modelCapability)];
      if (item?.kind === 'capability') return action(controller, 'selectCapability', item.capabilityId);
    }
  };
  const change = event => {
    if (event.target.matches('#model-task')) return action(controller, 'setTask', event.target.value);
    const revision = event.target.dataset.modelRevision;
    if (revision && event.target.type === 'checkbox') {
      const [recordIndex, revisionIndex] = revision.split(':').map(Number);
      const capability = records(controller.getState())[recordIndex];
      const detail = controller.getState().selectedCapabilities?.[capability?.capabilityId];
      const itemRevision = detail?.history?.[revisionIndex];
      if (capability && itemRevision) return action(controller, 'toggleRevision', capability.capabilityId, itemRevision.id);
    }
    const record = event.target.closest('[data-model-record]');
    if (!record || event.target.type !== 'checkbox') return;
    const item = records(controller.getState())[Number(record.dataset.modelRecord)];
    if (!item) return;
    if (item.kind === 'source') return action(controller, 'toggleSource', item.sourceId);
    if (item.kind === 'capability') return action(controller, 'toggleRevision', item.capabilityId, item.revisionId);
  };
  const submit = event => {
    if (!event.target.matches('#model-profile-form')) return;
    event.preventDefault();
    const form = new FormData(event.target);
    action(controller, 'configure', { profileId: String(form.get('profileId') ?? ''), host: String(form.get('host') ?? ''), port: Number(form.get('port')), model: String(form.get('model') ?? ''), maxOutputTokens: Number(form.get('maxOutputTokens')) });
  };
  const keydown = event => {
    const tabs = [...root.querySelectorAll('[data-model-tab]')];
    if (!event.target.matches('[data-model-tab]') || !['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const here = tabs.indexOf(event.target);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (here + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
    const tab = tabs[next].dataset.modelTab;
    action(controller, 'setTab', tab);
    root.querySelector(`[data-model-tab="${tab}"]`)?.focus();
  };
  root.addEventListener('click', click);
  root.addEventListener('change', change);
  root.addEventListener('submit', submit);
  root.addEventListener('keydown', keydown);
  render();
  return { render, dispose: () => { root.removeEventListener('click', click); root.removeEventListener('change', change); root.removeEventListener('submit', submit); root.removeEventListener('keydown', keydown); } };
}
