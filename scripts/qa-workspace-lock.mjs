// Disposable loopback-only browser fixture. It loads the actual workbench, never Tauri.
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createPreviewServer } from './serve.mjs';

export async function createWorkspaceLockQAServer() {
  const report = JSON.parse(await readFile(new URL('../fixtures/contracts/source-analysis.golden.json', import.meta.url), 'utf8'));
  const engine = JSON.parse(await readFile(new URL('../fixtures/contracts/engine-status.json', import.meta.url), 'utf8'));
  const html = (await readFile(new URL('../preview/analyze.html', import.meta.url), 'utf8')).replace('<script type="module" src="analyze.mjs">', '<script src="workspace-lock-fixture.mjs"></script><script type="module" src="analyze.mjs">');
  const script = `(${installFixture.toString()})(${JSON.stringify(report)},${JSON.stringify(engine)});`;
  const server = createPreviewServer();
  const original = server.listeners('request')[0];
  server.removeAllListeners('request');
  server.on('request', (request, response) => {
    const pathname = new URL(request.url, 'http://localhost').pathname;
    if (!['/analyze.html', '/workspace-lock-fixture.mjs'].includes(pathname)) return original(request, response);
    const allowed = ['GET', 'HEAD'].includes(request.method);
    response.writeHead(allowed ? 200 : 405, {
      'Content-Type': pathname.endsWith('.mjs') ? 'text/javascript; charset=utf-8' : 'text/html; charset=utf-8',
      'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', 'Referrer-Policy': 'no-referrer',
      'Content-Security-Policy': "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    });
    response.end(request.method === 'HEAD' ? undefined : allowed ? pathname.endsWith('.mjs') ? script : html : 'Method not allowed');
  });
  return server;
}

function installFixture(report, engine) {
  const mode = new URLSearchParams(location.search).get('qa') || 'open';
  const commands = [];
  let closing = false, acknowledged = false, released = mode !== 'hold', pending = null;
  const token = 'ab'.repeat(32);
  const metadata = state => ({ schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'managed_encrypted', state, attemptId: state === 'open' ? null : token, epoch: state === 'open' ? null : '1', diagnostic: null });
  const id = (prefix, digit) => `${prefix}:${digit.repeat(64)}`;
  const snapshot = { sourceId: report.source.id, displayName: report.source.displayName, sha256: report.source.sha256, byteLength: report.source.byteLength, savedAtMs: 1 };
  const revision = { id: id('revision', '4'), parentRevisionId: null, title: 'QA saved instructions', content: 'QA private draft marker.\n', sha256: 'a'.repeat(64), createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } };
  const { content: ignored, ...revisionSummary } = revision; void ignored;
  const capability = { schemaVersion: 'rangoon.capability.v1', id: id('capability', '3'), origin: { kind: 'source', sourceId: report.source.id, fragmentId: report.fragments[0].id, sourceName: report.source.displayName, span: report.fragments[0].span, originalText: report.fragments[0].text }, latestRevisionId: revision.id, revision, history: [revisionSummary], authority: 'none' };
  const summary = { id: capability.id, origin: { kind: 'source', sourceId: report.source.id, fragmentId: report.fragments[0].id }, latestRevisionId: revision.id, title: revision.title, reviewed: false, revisionCount: 1 };
  const workspace = { schemaVersion: 'rangoon.workspace-data.v2', stateId: id('workspace', '7'), records: { sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 0, workflowRevisions: 0 }, databaseBytes: 4096, reusableBytes: 0, sources: [snapshot], capabilities: [summary], workflows: [] };
  const controls = document.createElement('details');
  controls.id = 'workspace-lock-qa';
  controls.style.cssText = 'position:fixed;bottom:8px;right:8px;z-index:10000;max-width:270px;padding:8px;background:Canvas;color:CanvasText;border:1px solid GrayText;font:12px system-ui';
  controls.innerHTML = '<summary>Synthetic lock QA — no native data</summary><button id="qa-release" type="button">Release fixture work</button><button id="qa-pagehide" type="button">Simulate pagehide</button><button id="qa-pageshow" type="button">Simulate BFCache return</button><pre id="qa-commands"></pre>';
  document.body.append(controls);
  const trace = () => { controls.querySelector('#qa-commands').textContent = commands.join('\n'); controls.dataset.calls = String(commands.length); };
  controls.querySelector('#qa-release').addEventListener('click', () => { released = true; if (pending) { pending(); pending = null; } });
  controls.querySelector('#qa-pagehide').addEventListener('click', () => window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true })));
  controls.querySelector('#qa-pageshow').addEventListener('click', () => window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true })));
  if (mode === 'browser') return;
  window.__TAURI__ = { core: { invoke: async (command, args, options) => {
    commands.push(command); trace();
    if (command === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: closing ? 'unavailable' : 'open', clientId: closing ? null : token };
    if (command === 'get_workspace_close_state') {
      if (mode === 'malformed') return {};
      if (mode === 'legacy') return { ...metadata('open'), storage: 'legacy_plaintext', state: 'unavailable', diagnostic: 'encrypted_close_unavailable' };
      return metadata(closing ? acknowledged && released ? 'locked' : 'closing' : 'open');
    }
    if (command === 'begin_workspace_close') { closing = true; return metadata('closing'); }
    if (command === 'acknowledge_workspace_purge') {
      controls.dataset.rawAck = String(args instanceof Uint8Array);
      const ack = args instanceof Uint8Array ? JSON.parse(new TextDecoder().decode(args)) : null;
      const valid = ack && Object.keys(ack).length === 3 && ack.schemaVersion === 'rangoon.workspace-purge-ack.v1' && ack.attemptId === token && ack.epoch === '1' && !acknowledged;
      if (!valid || mode === 'reject') return { ...metadata('locked'), diagnostic: 'invalid_workspace_close_request' };
      acknowledged = true; return metadata(released ? 'locked' : 'closing');
    }
    if (command === 'get_engine_status') return engine;
    if (mode !== 'legacy' && options?.headers?.['x-rangoon-workspace-client'] !== token) throw new Error('Fixture request identity refused.');
    if (closing) throw new Error('Fixture source access refused after close.');
    if (command === 'list_snapshots') return { outcome: 'listed', snapshots: [snapshot] };
    if (['open_snapshot', 'select_and_analyze', 'read_composition_source'].includes(command)) {
      if (mode === 'late' && command === 'open_snapshot') return new Promise(resolve => { pending = () => resolve({ outcome: 'analyzed', report }); });
      return { outcome: 'analyzed', report };
    }
    if (command === 'list_capabilities') return { outcome: 'listed', capabilities: [summary] };
    if (command === 'open_capability') return { outcome: 'opened', capability, alreadyApplied: false };
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace };
    if (command === 'list_workflows') return { outcome: 'listed', workflows: [] };
    if (command === 'get_local_model_profile') return { schemaVersion: 'rangoon.local-session-result.v1', generation: '1', runId: null, authority: 'none', outcome: 'unconfigured', session: { generation: '1', profile: null, active: null, persistence: 'session_only', processingLocation: 'unknown', retention: 'unknown', authority: 'none' } };
    return { outcome: 'failed', error: { code: 'fixture_unsupported', message: 'This synthetic fixture does not implement this operation.' } };
  } } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const server = await createWorkspaceLockQAServer();
  server.listen(0, '127.0.0.1', () => console.log(`Synthetic renderer lock QA: http://127.0.0.1:${server.address().port}/analyze.html?qa=open#analysis`));
}
