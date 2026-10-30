// Disposable synthetic transport for the real Skills renderer; never native data.
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { createWorkspaceLockQAServer } from './qa-workspace-lock.mjs';

function installSkillsFixture() {
  const original = window.__TAURI__?.core?.invoke;
  if (!original) return;
  const controls = document.createElement('details');
  controls.id = 'qa-skills-controls';
  controls.style.cssText = 'position:fixed;bottom:8px;left:8px;z-index:10001;padding:8px;background:Canvas;color:CanvasText;border:1px solid GrayText;font:12px system-ui;max-width:240px';
  controls.innerHTML = '<summary>Synthetic Skills QA · no native data</summary><button type="button" id="qa-skills-hold">Hold next open</button><button type="button" id="qa-skills-release">Release open</button><button type="button" id="qa-skills-conflict">Conflict next save</button><button type="button" id="qa-skills-fail-list">Fail next list</button><button type="button" id="qa-skills-fail-open">Fail next open</button>';
  document.body.append(controls);
  const trace = document.createElement('output'); trace.id = 'qa-skills-calls'; trace.hidden = true; document.body.append(trace);
  let hold = false, release = null, conflict = false, failList = false, failOpen = false;
  const calls = [];
  const requests = document.createElement('output'); requests.id = 'qa-skills-requests'; requests.hidden = true; document.body.append(requests);
  const requestLog = [];
  controls.querySelector('#qa-skills-hold').onclick = () => { hold = true; };
  controls.querySelector('#qa-skills-release').onclick = () => { hold = false; release?.(); release = null; };
  controls.querySelector('#qa-skills-conflict').onclick = () => { conflict = true; };
  controls.querySelector('#qa-skills-fail-list').onclick = () => { failList = true; };
  controls.querySelector('#qa-skills-fail-open').onclick = () => { failOpen = true; };
  const id = (prefix, digit) => `${prefix}:${digit.repeat(64)}`;
  const summary = revision => { const { content, ...rest } = revision; void content; return rest; };
  const records = async options => {
    const base = await original('open_capability', { capabilityId: id('capability', '3'), revisionId: null }, options);
    const first = structuredClone(base.capability);
    const old = { ...first.revision, createdAtMs: 1792944000000 };
    const current = { ...old, id: id('revision', '5'), parentRevisionId: old.id, title: 'Project instructions', content: 'Keep changes bounded.\nReview before publishing.\n', sha256: 'b'.repeat(64), createdAtMs: 1793030400000 };
    first.latestRevisionId = current.id; first.revision = current; first.history = [summary(old), summary(current)];
    const second = structuredClone(first); second.id = id('capability', '6'); second.latestRevisionId = id('revision', '8');
    second.revision = { ...old, id: second.latestRevisionId, title: 'Review guidelines', content: 'Review the exact diff.\n', createdAtMs: 1793034000000 }; second.history = [summary(second.revision)];
    return [first, second];
  };
  const library = items => items.map(item => ({ id: item.id, origin: { kind: 'source', sourceId: item.origin.sourceId, fragmentId: item.origin.fragmentId }, latestRevisionId: item.latestRevisionId, title: item.revision.title, reviewed: false, revisionCount: item.history.length }));
  window.__TAURI__.core.invoke = async (command, args, options) => {
    calls.push(command); trace.textContent = JSON.stringify(calls);
    requestLog.push({ command, capabilityId: args?.capabilityId ?? null, revisionId: args?.revisionId ?? null }); requests.textContent = JSON.stringify(requestLog);
    if (command === 'revise_capability' || command === 'review_capability' || command === 'create_capability') {
      const code = conflict ? 'capability_conflict' : 'synthetic_mutation_refused'; conflict = false;
      return { outcome: 'failed', error: { code, message: 'Synthetic QA never provides native persistence or review.' } };
    }
    if (command === 'list_capabilities') {
      if (failList) { failList = false; return { outcome: 'failed', error: { code: 'synthetic_list_refused', message: 'Synthetic list failure.' } }; }
      return { outcome: 'listed', capabilities: library(await records(options)) };
    }
    if (command === 'open_capability') {
      if (failOpen) { failOpen = false; return { outcome: 'failed', error: { code: 'synthetic_open_refused', message: 'Synthetic open failure.' } }; }
      if (hold) await new Promise(resolve => { release = resolve; });
      const items = await records(options); const item = items.find(item => item.id === args?.capabilityId);
      if (!item) return { outcome: 'failed', error: { code: 'synthetic_missing_record', message: 'Unknown synthetic record.' } };
      if (args.revisionId && args.revisionId !== item.latestRevisionId) {
        if (item.history[0].id !== args.revisionId) return { outcome: 'failed', error: { code: 'synthetic_missing_revision', message: 'Unknown synthetic revision.' } };
        item.revision = { ...item.history[0], content: 'QA private draft marker.\n' };
      }
      return { outcome: 'opened', capability: item, alreadyApplied: false };
    }
    const result = await original(command, args, options);
    if (command === 'get_workspace_data' && result?.outcome === 'loaded') {
      const workspace = structuredClone(result.workspace); workspace.capabilities = library(await records(options)); workspace.records.capabilities = 2; workspace.records.revisions = 3;
      return { ...result, workspace };
    }
    return result;
  };
}

export async function createSkillsInteractionQAServer() {
  const server = await createWorkspaceLockQAServer();
  const original = server.listeners('request')[0]; server.removeAllListeners('request');
  const script = `\n(${installSkillsFixture.toString()})();`;
  server.on('request', (request, response) => {
    if (new URL(request.url, 'http://localhost').pathname === '/workspace-lock-fixture.mjs') {
      const end = response.end.bind(response); response.end = (chunk, ...rest) => end(String(chunk ?? '') + script, ...rest);
    }
    original(request, response);
  });
  return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const server = await createSkillsInteractionQAServer();
  server.listen(0, '127.0.0.1', () => console.log(`Synthetic Skills QA: http://127.0.0.1:${server.address().port}/analyze.html?qa=open#skills`));
}
