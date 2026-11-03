// Opt-in, memory-only app replay. Never loaded by product pages or native hosts.
export async function handoffPreview(request, reads) {
  if (request?.schemaVersion !== 'rangoon.composition-application-request.v0' || request.draft?.operation !== 'merge' || request.draft.outputs?.length !== 1 || request.targets?.length !== 1 || request.targets[0].kind !== 'new') throw Error('Unsupported handoff fixture request.');
  const id = (kind, digit) => `${kind}:${digit.repeat(64)}`;
  const encoder = new TextEncoder(); const decoder = new TextDecoder('utf-8', { fatal: true });
  const mappings = []; let content = ''; let offset = 0;
  const recipe = request.draft.outputs[0];
  for (const [pieceIndex, piece] of recipe.pieces.entries()) {
    if (!['copy','authored'].includes(piece.kind)) throw Error('Unsupported handoff fixture piece.');
    let value = piece.content;
    if (piece.kind === 'copy') {
      const reference = request.draft.inputs[piece.range.inputIndex];
      const input = reads.find(item => item.id === reference.capabilityId && item.revision.id === reference.revisionId);
      if (!input) throw Error('Unknown exact handoff fixture input.');
      value = decoder.decode(encoder.encode(input.revision.content).slice(piece.range.startByte, piece.range.endByte));
    }
    const end = offset + encoder.encode(value).length;
    mappings.push({ outputIndex: 0, pieceIndex, outputStartByte: offset, outputEndByte: end, kind: piece.kind, inputRange: piece.kind === 'copy' ? piece.range : null });
    content += value; offset = end;
  }
  const sha256 = [...new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode(content)))].map(value => value.toString(16).padStart(2,'0')).join('');
  const output = { outputIndex: 0, title: recipe.title, content, sha256, capabilityId: id('capability','9'), revisionId: id('revision','9') };
  const coverage = mappings.filter(item => item.inputRange).map(item => ({ range: item.inputRange, disposition: 'copied', references: [{ outputIndex: 0, pieceIndex: item.pieceIndex, kind: item.kind }], duplicationAcknowledged: false }));
  return { outcome: 'ready', schemaVersion: 'rangoon.composition-session.v0', previewId: id('preview','8'), expectedStateId: id('workspace','5'), preview: { schemaVersion: 'rangoon.composition-application-preview.v0', core: { schemaVersion: 'rangoon.composition-core-preview.v0', compositionId: id('composition','6'), saveable: true, outputs: [output], mappings, coverage, diagnostics: [], authority: 'none' }, applicationId: id('composition-application','7'), appliedOutputs: [{ outputIndex: 0, kind: 'new', capabilityId: output.capabilityId, parentRevisionId: null, revisionId: output.revisionId }], saveable: true, authority: 'none' } };
}

export function handoffReceipt(preview, request) {
  const output = preview.preview.core.outputs[0]; const applied = preview.preview.appliedOutputs[0];
  const compositionId = preview.preview.core.compositionId; const applicationId = preview.preview.applicationId;
  const revision = { id: applied.revisionId, parentRevisionId: null, title: output.title, content: output.content, sha256: output.sha256, createdAtMs: 1, review: null, provenance: { kind: 'composition', compositionId, applicationId, outputIndex: 0 } };
  const { content, ...summary } = revision; void content;
  return { schemaVersion: 'rangoon.composition-receipt.v0', compositionId, applicationId, committedStateId: preview.expectedStateId, capabilities: [{ schemaVersion: 'rangoon.capability.v1', id: applied.capabilityId, origin: { kind: 'composition', compositionId, operation: request.draft.operation, outputIndex: 0, inputs: request.draft.inputs }, latestRevisionId: revision.id, revision, history: [summary], authority: 'none' }], authority: 'none' };
}

export function installHandoffFixture(buildPreview, buildReceipt) {
  if (new URLSearchParams(location.search).get('handoff') !== '1') return;
  const original = window.__TAURI__.core.invoke;
  const controls = document.createElement('div'); controls.id = 'qa-composition-controls';
  controls.innerHTML = '<strong>Synthetic memory-only handoff; no native save</strong><button id="qa-composition-hold">Hold result read</button><button id="qa-composition-release">Release result read</button><output id="qa-composition-requests" hidden></output>';
  document.body.append(controls);
  const reads = new Map(); const requests = []; let request, preview, receipt, hold = false, release = null;
  controls.querySelector('#qa-composition-hold').onclick = () => { hold = true; };
  controls.querySelector('#qa-composition-release').onclick = () => { hold = false; release?.(); release = null; };
  const summary = detail => { const { inputs, ...origin } = detail.origin; void inputs; return { id: detail.id, origin, latestRevisionId: detail.latestRevisionId, title: detail.revision.title, reviewed: false, revisionCount: detail.history.length }; };
  window.__TAURI__.core.invoke = async (command, args, options) => {
    const entry = { command, capabilityId: args?.capabilityId ?? null, revisionId: args?.revisionId ?? null, completed: false };
    requests.push(entry);
    controls.querySelector('#qa-composition-requests').textContent = JSON.stringify(requests);
    if (command === 'preview_composition') {
      request = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(args));
      preview = await buildPreview(request, [...reads.values()]); return preview;
    }
    if (command === 'commit_composition') {
      const confirmation = JSON.parse(new TextDecoder().decode(args));
      if (!preview || receipt || confirmation.schemaVersion !== 'rangoon.composition-confirmation.v0' || Object.keys(confirmation).length !== 4 || confirmation.previewId !== preview.previewId || confirmation.expectedStateId !== preview.expectedStateId || confirmation.acknowledged !== true) throw Error('Synthetic commit refused.');
      receipt = buildReceipt(preview, request); return { outcome: 'committed', receipt };
    }
    if (command === 'open_capability' && receipt?.capabilities[0].id === args?.capabilityId) {
      if (hold) await new Promise(resolve => { release = resolve; });
      const detail = structuredClone(receipt.capabilities[0]);
      // The saved result is historical by the time it is opened: exact pin must survive.
      const newer = { ...detail.history[0], id: `revision:${'a'.repeat(64)}`, parentRevisionId: detail.revision.id, title: 'Newer synthetic revision', createdAtMs: 2 };
      detail.latestRevisionId = newer.id; detail.history.push(newer);
      if (args.revisionId !== detail.revision.id) detail.revision = { ...newer, content: 'Newer synthetic content.\n' };
      entry.completed = true; controls.querySelector('#qa-composition-requests').textContent = JSON.stringify(requests);
      return { outcome: 'opened', capability: detail, alreadyApplied: false };
    }
    const result = await original(command, args, options);
    entry.completed = true; controls.querySelector('#qa-composition-requests').textContent = JSON.stringify(requests);
    if (command === 'open_capability' && result?.outcome === 'opened') reads.set(`${result.capability.id}:${result.capability.revision.id}`, result.capability);
    if (receipt && command === 'list_capabilities' && result?.outcome === 'listed') return { ...result, capabilities: [...result.capabilities, summary(receipt.capabilities[0])] };
    if (receipt && command === 'get_workspace_data' && result?.outcome === 'loaded') {
      const workspace = structuredClone(result.workspace); workspace.capabilities.push(summary(receipt.capabilities[0])); workspace.records.capabilities++; workspace.records.revisions++;
      return { ...result, workspace };
    }
    return result;
  };
}
