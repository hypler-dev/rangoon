// Disposable synthetic transport for the real renderer/controller; never Tauri.
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { createWorkspaceLockQAServer } from './qa-workspace-lock.mjs';

function installCompositionFixture() {
  const original = window.__TAURI__?.core?.invoke;
  if (!original) return;
  const controls = document.createElement('details');
  controls.id = 'qa-composition-controls';
  controls.style.cssText = 'position:fixed;bottom:8px;right:8px;z-index:500;padding:8px;background:Canvas;color:CanvasText;border:1px solid GrayText;font:12px system-ui;max-width:240px';
  controls.innerHTML = '<summary>Synthetic composition QA · no native data</summary><button type="button" id="qa-composition-hold">Hold next preview</button><button type="button" id="qa-composition-release">Release preview</button>';
  document.body.append(controls);
  const trace = document.createElement('output'); trace.id = 'qa-composition-calls'; trace.hidden = true; document.body.append(trace);
  const calls = []; let hold = false; let release = null;
  controls.querySelector('#qa-composition-hold').addEventListener('click', () => { hold = true; });
  controls.querySelector('#qa-composition-release').addEventListener('click', () => { hold = false; release?.(); release = null; });
  window.__TAURI__.core.invoke = async (command, args, options) => {
    calls.push(command); trace.textContent = JSON.stringify(calls);
    if (command === 'preview_composition') {
      if (hold) await new Promise(resolve => { release = resolve; });
      return { outcome: 'failed', error: { code: 'synthetic_preview_refused', message: 'Synthetic QA does not provide native composition previews or saves.' } };
    }
    const result = await original(command, args, options);
    if (command === 'get_workspace_data' && result?.outcome === 'loaded') {
      const workspace = structuredClone(result.workspace);
      workspace.capabilities.push({ ...workspace.capabilities[0], id: `capability:${'6'.repeat(64)}`, title: 'QA additional instructions', latestRevisionId: `revision:${'8'.repeat(64)}` });
      workspace.records.capabilities = 2; workspace.records.revisions = 2;
      return { ...result, workspace };
    }
    if (command === 'list_capabilities' && result?.outcome === 'listed') {
      return { ...result, capabilities: [...result.capabilities, { ...result.capabilities[0], id: `capability:${'6'.repeat(64)}`, title: 'QA additional instructions', latestRevisionId: `revision:${'8'.repeat(64)}` }] };
    }
    if (command === 'open_capability' && args?.capabilityId === `capability:${'6'.repeat(64)}` && result?.outcome === 'opened') {
      const capability = structuredClone(result.capability);
      capability.id = args.capabilityId; capability.latestRevisionId = `revision:${'8'.repeat(64)}`;
      capability.revision.id = capability.latestRevisionId; capability.revision.title = 'QA additional instructions';
      capability.history[0].id = capability.latestRevisionId; capability.history[0].title = capability.revision.title;
      return { ...result, capability };
    }
    return result;
  };
}

export async function createCompositionInteractionQAServer() {
  const server = await createWorkspaceLockQAServer();
  const original = server.listeners('request')[0]; server.removeAllListeners('request');
  const script = `\n(${installCompositionFixture.toString()})();`;
  server.on('request', (request, response) => {
    if (new URL(request.url, 'http://localhost').pathname === '/workspace-lock-fixture.mjs') {
      const end = response.end.bind(response); response.end = (chunk, ...rest) => end(String(chunk ?? '') + script, ...rest);
    }
    original(request, response);
  });
  return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const server = await createCompositionInteractionQAServer();
  server.listen(0, '127.0.0.1', () => console.log(`Synthetic composition QA: http://127.0.0.1:${server.address().port}/analyze.html?qa=open#decompose`));
}
