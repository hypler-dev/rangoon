// Disposable loopback-only desktop-startup UI QA. Uses synthetic native DTOs only.
import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createPreviewServer } from './serve.mjs';

function fixture() {
  const mode = new URLSearchParams(location.search).get('qa') || 'fresh';
  const startupId = 'a'.repeat(64), initialAttempt = 'b'.repeat(64), startupAttempt = 'c'.repeat(64), clientId = 'd'.repeat(64);
  const close = (state, attemptId = initialAttempt, epoch = '1') => ({
    schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'managed_encrypted', state,
    attemptId: state === 'open' ? null : attemptId, epoch: state === 'open' ? null : epoch, diagnostic: null,
  });
  const legacyClose = () => ({
    schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'legacy_plaintext', state: 'unavailable',
    attemptId: null, epoch: null, diagnostic: 'encrypted_close_unavailable',
  });
  const startup = state => ({
    schemaVersion: 'rangoon.workspace-startup.v1', state, startupId: state === 'handoff' ? null : startupId,
    attemptId: state === 'handoff' ? null : startupAttempt, epoch: state === 'handoff' ? null : '2',
    actions: state === 'fresh' ? ['setup', 'quit'] : [], copyPhase: null, retainedOriginal: 'not_applicable', diagnostic: null,
  });
  const calls = [];
  const root = document.documentElement;
  root.dataset.qaCalls = '[]';
  const trace = () => { root.dataset.qaCalls = JSON.stringify(calls); };
  let startupRequested = false, initialObserved = false, c0 = false, c1 = false, handoff = false, resolveStatus;
  const statusWait = new Promise(resolve => { resolveStatus = resolve; });
  const control = () => {
    const open = mode === 'initialopen';
    return {
      schemaVersion: 'rangoon.workspace-control-state.v1', close: close(open ? 'open' : 'locked', startupAttempt, '2'),
      phase: open ? 'open' : 'locked', action: null, outcome: 'observed', diagnostic: null, reload: false,
    };
  };
  const request = () => ({ schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId });
  const native = async (command, args) => {
    calls.push(command); trace();
    if (command === 'get_workspace_close_state') {
      if (mode === 'initialopen' || mode === 'initialopenlegacy') return handoff && mode === 'initialopenlegacy' ? legacyClose() : close('open');
      if (handoff) return close('locked', startupAttempt, '2');
      if (!initialObserved) { initialObserved = true; return close('locked'); }
      if (!startupRequested || c1) return close('locked', startupAttempt, '2');
      return close('closing', startupAttempt, '2');
    }
    if (command === 'acknowledge_workspace_purge') {
      if (mode === 'badack') return {};
      const decoded = args instanceof Uint8Array ? JSON.parse(new TextDecoder().decode(args)) : null;
      if (!decoded?.attemptId) return {};
      if (decoded.attemptId === initialAttempt) { c0 = true; return close('locked'); }
      if (decoded.attemptId === startupAttempt) { c1 = true; resolveStatus?.(); return close('locked', startupAttempt, '2'); }
      return {};
    }
    if (command === 'workspace_startup_status') {
      startupRequested = true;
      if (mode === 'malformed' || mode === 'badquit') return {};
      if (mode === 'initialopen' || mode === 'initialopenlegacy') {
        handoff = true;
        return startup('handoff');
      }
      await statusWait;
      return c1 ? startup('fresh') : {};
    }
    if (command === 'workspace_startup_quit') {
      if (JSON.stringify(args) !== '{}') throw new Error('Quit requires an exact empty object.');
      if (mode === 'badquit') throw new Error('Synthetic quit refusal.');
      return null;
    }
    if (command === 'workspace_startup_action') {
      if (mode === 'badstartup') return {};
      const decoded = args instanceof Uint8Array ? JSON.parse(new TextDecoder().decode(args)) : null;
      if (!decoded || decoded.action !== 'setup') return {};
      handoff = true;
      return startup('handoff');
    }
    if (command === 'get_workspace_request_state') return request();
    if (command === 'get_workspace_control_state') return control();
    if (command === 'get_workspace_protection_state') return {
      schemaVersion: 'rangoon.workspace-protection-state.v1', close: control().close, layout: 'absent', diagnostic: null,
    };
    if (command === 'list_snapshots') return { outcome: 'listed', snapshots: [] };
    if (command === 'list_capabilities') return { outcome: 'listed', capabilities: [] };
    if (command === 'list_workflows') return { outcome: 'listed', workflows: [] };
    throw new Error(`Unexpected fixture command: ${command}`);
  };
  if (mode !== 'browser') window.__TAURI__ = { core: { invoke: native } };
  window.__desktopStartupQa = {
    calls,
    get state() { return { startupRequested, initialObserved, c0, c1, handoff }; },
  };
}

export async function createDesktopStartupQAServer() {
  const html = (await readFile(new URL('../preview/analyze.html', import.meta.url), 'utf8'))
    .replace('<script type="module" src="analyze.mjs"></script>', '<script src="desktop-startup-fixture.mjs"></script><script type="module" src="analyze.mjs"></script>');
  const script = `(${fixture.toString()})();`;
  const server = createPreviewServer();
  const original = server.listeners('request')[0];
  server.removeAllListeners('request');
  server.on('request', (request, response) => {
    const path = new URL(request.url, 'http://localhost').pathname;
    if (!['/analyze.html', '/desktop-startup-fixture.mjs'].includes(path)) return original(request, response);
    const allowed = ['GET', 'HEAD'].includes(request.method);
    response.writeHead(allowed ? 200 : 405, {
      'Content-Type': path.endsWith('.mjs') ? 'text/javascript; charset=utf-8' : 'text/html; charset=utf-8',
      'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', 'Referrer-Policy': 'no-referrer',
      'Content-Security-Policy': "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    });
    response.end(request.method === 'HEAD' ? undefined : allowed ? path.endsWith('.mjs') ? script : html : 'Method not allowed');
  });
  return server;
}

const calls = page => page.evaluate(() => JSON.parse(document.documentElement.dataset.qaCalls || '[]'));
const noContentCalls = list => assert.equal(list.some(command => ['list_snapshots', 'list_capabilities', 'list_workflows', 'select_and_analyze'].includes(command)), false);

async function waitForState(page, selector) {
  try { await page.waitForSelector(selector); }
  catch (error) {
    const proof = await page.evaluate(() => ({
      calls: (() => {
        const list = JSON.parse(document.documentElement.dataset.qaCalls || '[]');
        return { total: list.length, counts: Object.fromEntries([...new Set(list)].map(command => [command, list.filter(item => item === command).length])) };
      })(),
      fixture: window.__desktopStartupQa?.state ?? null,
      close: document.querySelector('[data-workspace-close]')?.getAttribute('data-workspace-close') ?? null,
      startup: document.querySelector('[data-desktop-startup-state]')?.getAttribute('data-desktop-startup-state') ?? null,
      text: document.querySelector('#analysis-app')?.textContent?.trim().slice(0, 320) ?? '',
    }));
    const message = `${error.message}\nFixture state: ${JSON.stringify(proof)}`;
    console.error(message);
    throw new Error(message, { cause: error });
  }
}

async function open(context, url, mode, viewport = { width: 768, height: 900 }) {
  const page = await context.newPage();
  await page.setViewportSize(viewport);
  await page.goto(`${url}/analyze.html?qa=${mode}#analysis`, { waitUntil: 'networkidle' });
  return page;
}

export async function runDesktopStartupQA({ chromium: suppliedChromium, executablePath, screenshotDir } = {}) {
  let chromium = suppliedChromium;
  try { if (!chromium) ({ chromium } = await import('playwright')); }
  catch { throw new Error('Playwright unavailable; no dependency was installed. Run this QA where existing Playwright is available.'); }
  const server = await createDesktopStartupQAServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ headless: true, ...(executablePath ? { executablePath } : {}) });
  const context = await browser.newContext();
  const screenshots = [];
  if (screenshotDir) await mkdir(screenshotDir, { recursive: true });
  await context.route('**/*', route => {
    const host = new URL(route.request().url()).hostname;
    return ['127.0.0.1', 'localhost', '::1'].includes(host) ? route.continue() : route.abort();
  });
  try {
    const fresh = await open(context, url, 'fresh');
    await fresh.waitForSelector('[data-desktop-startup-state="fresh"]');
    assert.equal(await fresh.locator('#analysis-announcer').getAttribute('aria-live'), 'off');
    noContentCalls(await calls(fresh));
    await fresh.locator('[data-desktop-startup-action="setup"]').focus();
    await fresh.evaluate(() => {
      const action = document.querySelector('[data-desktop-startup-action="setup"]');
      action?.click();
      action?.click();
    });
    await waitForState(fresh, '[data-workspace-close="protection"]');
    const freshCalls = await calls(fresh);
    assert.equal(freshCalls.filter(command => command === 'workspace_startup_action').length, 1);
    noContentCalls(freshCalls);
    await fresh.close();

    const legacyAfterHandoff = await open(context, url, 'initialopenlegacy');
    await legacyAfterHandoff.waitForSelector('[data-desktop-startup-state="unavailable"]');
    noContentCalls(await calls(legacyAfterHandoff));
    assert.equal((await calls(legacyAfterHandoff)).includes('get_workspace_request_state'), false);
    await legacyAfterHandoff.close();

    const initial = await open(context, url, 'initialopen');
    await initial.waitForSelector('#analysis-title');
    assert.ok((await calls(initial)).includes('get_workspace_request_state'));
    const cancelableBeforeUnload = await initial.evaluate(() => {
      const event = new Event('beforeunload', { cancelable: true });
      event.preventDefault();
      window.dispatchEvent(event);
      return { defaultPrevented: event.defaultPrevented, contentStillMounted: Boolean(document.querySelector('#analysis-title')) };
    });
    assert.equal(cancelableBeforeUnload.defaultPrevented, true);
    assert.equal(cancelableBeforeUnload.contentStillMounted, true);
    await initial.evaluate(() => window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true })));
    await initial.waitForSelector('[data-workspace-close="recovery_required"]');
    assert.equal(await initial.locator('#analysis-title').count(), 0);
    await initial.close();

    for (const mode of ['malformed', 'badack', 'badstartup']) {
      const page = await open(context, url, mode);
      if (mode === 'badstartup') {
        await page.waitForSelector('[data-desktop-startup-state="fresh"]');
        await page.locator('[data-desktop-startup-action="setup"]').click();
      }
      await page.waitForSelector('[data-desktop-startup-state="unavailable"]');
      noContentCalls(await calls(page));
      await page.close();
    }

    const badQuit = await open(context, url, 'badquit');
    await badQuit.waitForSelector('[data-desktop-startup-state="unavailable"]');
    await badQuit.evaluate(() => {
      const quit = document.querySelector('[data-desktop-startup-unavailable-quit]');
      quit?.click();
      quit?.click();
    });
    await badQuit.waitForFunction(() => !document.querySelector('[data-desktop-startup-unavailable-quit]')?.disabled);
    let badQuitCalls = await calls(badQuit);
    assert.equal(badQuitCalls.filter(command => command === 'workspace_startup_quit').length, 1);
    await badQuit.locator('[data-desktop-startup-unavailable-quit]').click();
    await badQuit.waitForFunction(() => JSON.parse(document.documentElement.dataset.qaCalls || '[]').filter(command => command === 'workspace_startup_quit').length === 2);
    badQuitCalls = await calls(badQuit);
    assert.equal(badQuitCalls.filter(command => command === 'workspace_startup_quit').length, 2);
    noContentCalls(badQuitCalls);
    await badQuit.close();

    const browserPreview = await open(context, url, 'browser');
    await browserPreview.waitForSelector('#analysis-title');
    assert.equal(await browserPreview.locator('[data-desktop-startup-state]').count(), 0);
    assert.deepEqual(await calls(browserPreview), []);
    await browserPreview.close();

    for (const width of [320, 768, 1440]) {
      for (const theme of ['dark', 'light']) {
        const page = await open(context, url, 'fresh', { width, height: 900 });
        const changedTheme = await page.evaluate(target => document.documentElement.dataset.theme !== target, theme);
        if (changedTheme) await page.locator('button[data-desktop-startup-theme]').click();
        await page.emulateMedia({ reducedMotion: 'reduce' });
        await page.waitForSelector('[data-desktop-startup-state="fresh"]');
        assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth), true);
        assert.equal(await page.locator(changedTheme ? 'button[data-desktop-startup-theme]' : '#desktop-startup-title').evaluate(element => document.activeElement === element), true);
        if (screenshotDir && [768, 1440].includes(width)) {
          const file = join(screenshotDir, `desktop-startup-${theme}-${width}.png`);
          await page.screenshot({ path: file });
          screenshots.push(file);
        }
        await page.close();
      }
    }
    const summary = Object.freeze({
      freshSourceFree: true, duplicateAction: true, setupHandoffProtection: true,
      initialOpenContent: true, beforeUnloadNoFence: true, pagehideFence: true,
      invalidStartupClosed: true, legacyAfterHandoffClosed: true, unavailableQuitRetries: true, browserNoStartupIpc: true,
      viewportWidths: [320, 768, 1440], lightDark: true, keyboardFocus: true,
      reducedMotion: true, screenshots: Object.freeze(screenshots),
    });
    console.log(`desktop-startup QA PASS ${JSON.stringify(summary)}`);
    return summary;
  } finally {
    await context.close();
    await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
}

if (import.meta.url === new URL(`file://${process.argv[1]}`).href) await runDesktopStartupQA();
