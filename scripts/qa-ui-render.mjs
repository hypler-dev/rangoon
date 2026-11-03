// Synthetic UI inputs only; host Chrome is not an OS filesystem sandbox.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createSkillsInteractionQAServer } from './qa-skills-interaction.mjs';
import { catalogKeys } from '../preview/icons.mjs';
import { classifyUiQaResult } from './ui-qa-result.mjs';
import { uiQaLaunch } from './ui-qa-launch.mjs';
import { compositionInspectorCases } from './ui-composition-fixtures.mjs';

const chromePath = process.env.RANGOON_QA_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
let output = null, profile = null, server = null, origin = null, chrome = null, browserExit = null;
let browserExited = false, ownedTarget = null, browserDiagnostic = '';
async function bounded(promise, milliseconds, label) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(Error(`${label} timeout`)), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
const evidence = { kind: 'synthetic_browser_only', output, origin, viewports: [], surfaces: [], interactions: [], exceptions: [], consoleErrors: [], blockedOrigins: [], nativeWorkflow: 'not_run', status: 'running' };
let socket, session, sequence = 0, stopping = false, expectedDialog = null;
const requests = new Map();
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function call(method, params = {}, sessionId) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { requests.delete(id); reject(Error(`CDP timeout: ${method}`)); }, 15000);
    requests.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
}
const page = (method, params) => call(method, params, session);
async function evaluate(expression) {
  const result = await page('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw Error(`Page evaluation failed: ${result.exceptionDetails.text}`);
  return result.result.value;
}
async function until(expression) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (await evaluate(expression)) return;
    await delay(75);
  }
  throw Error(`UI state timeout: ${expression}`);
}
async function click(selector) {
  evidence.lastControl=selector;
  await evaluate(`(() => { document.querySelector(${JSON.stringify(selector)})?.scrollIntoView({block:'center',behavior:'instant'}); return new Promise(resolve=>setTimeout(resolve,0)); })()`);
  const position = await evaluate(`(() => { const n=document.querySelector(${JSON.stringify(selector)}); if(!n || n.disabled) return null; const r=n.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2; return r.width && r.height && n.contains(document.elementFromPoint(x,y)) ? {x,y} : null; })()`);
  assert.ok(position, `Clickable control missing: ${selector}`);
  await page('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...position });
  await page('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...position });
}
async function key(name) {
  const codes = { Escape: 27, Tab: 9, ArrowDown: 40, Home: 36, End: 35, Enter: 13 };
  const nativeCodes = { Escape: 53, Tab: 48, ArrowDown: 125, Home: 115, End: 119, Enter: 36 };
  const text = name === 'Enter' ? { text: '\r', unmodifiedText: '\r' } : {};
  await page('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code: name, windowsVirtualKeyCode: codes[name], nativeVirtualKeyCode: nativeCodes[name], ...text });
  await page('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code: name, windowsVirtualKeyCode: codes[name], nativeVirtualKeyCode: nativeCodes[name] });
}
async function screenshot(name) {
  const result = await page('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile(join(output, name), Buffer.from(result.data, 'base64'));
}
async function theme(name) {
  if (await evaluate('document.documentElement.dataset.theme') !== name) await click('#analysis-theme');
  await until(`document.documentElement.dataset.theme === ${JSON.stringify(name)}`);
}
async function route(name) {
  await evaluate(`location.hash=${JSON.stringify('#' + name)}`);
  await until(`document.querySelector('.analysis-shell')?.dataset.area === ${JSON.stringify(name)}`);
}
try {
  await access(chromePath); // Never download a browser or dependency.
  output = await mkdtemp(join(tmpdir(), 'rangoon-ui-render-')); evidence.output = output;
  profile = await mkdtemp(join(output, 'browser-'));
  server = await createSkillsInteractionQAServer();
  await new Promise((resolve, reject) => {
    const failed = error => { server.off('listening', listening); reject(error); };
    const listening = () => { server.off('error', failed); resolve(); };
    server.once('error', failed); server.once('listening', listening); server.listen(0, '127.0.0.1');
  });
  origin = `http://127.0.0.1:${server.address().port}`; evidence.origin = origin;
  const hostArm64 = process.platform === 'darwin'
    ? execFileSync('/usr/sbin/sysctl', ['-n', 'hw.optional.arm64'], { timeout: 3000, encoding: 'utf8', maxBuffer: 1024 }).trim()
    : null;
  if (process.platform === 'darwin' && !['0', '1'].includes(hostArm64)) throw Error('Unrecognized macOS ARM capability.');
  const launch = uiQaLaunch({ platform: process.platform, hostArm64: hostArm64 === '1', browserPath: chromePath, explicitOverride: Boolean(process.env.RANGOON_QA_CHROME), args: [
    '--disable-gpu', '--headless=new', `--user-data-dir=${profile}`, '--remote-debugging-port=0', '--remote-debugging-address=127.0.0.1',
    '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
    '--disable-sync', '--disable-default-apps', '--disable-extensions', '--password-store=basic', '--use-mock-keychain',
    '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1', 'about:blank',
  ] });
  evidence.browserLaunch = { platform: process.platform, nodeArch: process.arch, hostArm64: hostArm64 === '1', selection: launch.selection, renderer: 'software_gpu_disabled' };
  chrome = spawn(launch.command, launch.args, { stdio: ['ignore', 'ignore', 'pipe'], env: { PATH: process.env.PATH, HOME: process.env.HOME, TMPDIR: tmpdir(), LANG: 'en_US.UTF-8' } });
  browserExit = new Promise(resolve => {
    chrome.once('exit', (code, signal) => { browserExited = true; resolve({ code, signal }); });
    chrome.once('error', error => { if (!chrome.pid) { browserExited = true; resolve({ code: null, signal: null, notStarted: true, diagnostic: error.message }); } });
  });
  const endpoint = await new Promise((resolve, reject) => {
    let tail = '';
    const timer = setTimeout(() => reject(Error('Chrome startup timeout')), 20000);
    chrome.once('error', error => { clearTimeout(timer); reject(error); });
    chrome.once('exit', code => { clearTimeout(timer); reject(Error(`Chrome exited ${code}`)); });
    chrome.stderr.on('data', chunk => {
      tail = (tail + chunk).slice(-8192); browserDiagnostic = tail;
      const match = tail.match(/DevTools listening on (ws:\/\/127\.0\.0\.1:[^\s]+)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
  });
  socket = new WebSocket(endpoint);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  socket.onmessage = event => {
    const message = JSON.parse(String(event.data));
    if (message.id) {
      const request = requests.get(message.id);
      if (!request) return;
      clearTimeout(request.timer); requests.delete(message.id);
      message.error ? request.reject(Error(JSON.stringify(message.error))) : request.resolve(message.result);
    } else if (message.method === 'Page.javascriptDialogOpening') {
      const dialog = message.params;
      const expected = expectedDialog;
      const matches = expected && !expected.seen && dialog.type === 'confirm' && dialog.message === expected.message && dialog.url === `${origin}/analyze.html?qa=open#merge`;
      if (matches) expected.seen = true;
      else evidence.exceptions.push('Unexpected browser dialog; dismissed without approval.');
      page('Page.handleJavaScriptDialog', { accept: matches ? expected.accept : false }).catch(error => evidence.consoleErrors.push(error.message));
    } else if (message.method === 'Runtime.exceptionThrown') {
      evidence.exceptions.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
    } else if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
      evidence.consoleErrors.push(message.params.args.map(arg => String(arg.value ?? arg.description ?? '')).join(' ').slice(0,800));
    } else if (message.method === 'Fetch.requestPaused') {
      const { requestId, request } = message.params;
      const url = new URL(request.url);
      const allowed = url.origin === origin || ['data:', 'blob:'].includes(url.protocol);
      if (!allowed) evidence.blockedOrigins.push(url.origin);
      void call(allowed ? 'Fetch.continueRequest' : 'Fetch.failRequest', { requestId, ...(allowed ? {} : { errorReason: 'BlockedByClient' }) }, message.sessionId).catch(error => { if (!stopping) evidence.exceptions.push(error.message); });
    }
  };
  socket.onclose = () => { for (const request of requests.values()) { clearTimeout(request.timer); request.reject(Error('CDP connection closed')); } requests.clear(); };
  evidence.browser = await call('Browser.getVersion');
  const target = await call('Target.createTarget', { url: 'about:blank' }); ownedTarget = target.targetId;
  ({ sessionId: session } = await call('Target.attachToTarget', { targetId: target.targetId, flatten: true }));
  await page('Page.enable'); await page('Runtime.enable'); await page('Fetch.enable', { patterns: [{ urlPattern: '*' }] });
  await page('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  evidence.phase='inspector_stress_fixtures';
  // Separate page and inert controller: display fixtures never enter the app transport.
  await page('Page.navigate', {url:`${origin}/icon-gallery.html`});
  await until(`document.querySelectorAll(".icon-gallery__card").length === ${catalogKeys.length}`);
  await evaluate(`(async()=>{
    for(const n of document.querySelectorAll('link[rel=stylesheet]')) n.remove();
    await Promise.all(['app.css','analyze.css','composition.css','visual-system.css','workbench-refinement.css','workbench-shell.css'].map(href=>new Promise((resolve,reject)=>{const n=document.createElement('link');n.rel='stylesheet';n.href=href;n.onload=resolve;n.onerror=reject;document.head.append(n)})));
    document.body.innerHTML='<div class="analysis-shell" data-area="split"><aside class="analysis-rail"><strong>Synthetic inspector fixture</strong><p>No native data, provider or persistence.</p></aside><main id="qa-inspector-root"></main></div>';
    window.__qaInspectorModule=await import('./composition-view.mjs');
  })()`);
  for (const fixture of compositionInspectorCases()) {
    await evaluate(`(()=>{
      window.__qaInspectorBinding?.dispose({preserveCache:false});
      window.__qaInspectorState=${JSON.stringify(fixture.state)};
      const controller=new Proxy({getState:()=>window.__qaInspectorState},{get(target,key){if(key==='getState')return target[key];throw Error('Inert inspector fixture cannot call '+String(key));}});
      window.__qaInspectorBinding=window.__qaInspectorModule.bindCompositionView(document.querySelector('#qa-inspector-root'),controller);
    })()`);
    if(fixture.name==='many-outputs') {
      await click('[data-composition-inspect-output="15"]');
      assert.equal(await evaluate('document.activeElement?.getAttribute("data-composition-output-title")'),'15');
      await click('[data-composition-add-authored="15"]');await page('Input.insertText',{text:'Retained unapplied inspector text.'});
      await click('[data-composition-tab=graph]');
      await click('[data-composition-inspect-output="0"]');
      await click('[data-composition-inspect-output="15"]');
      assert.equal(await evaluate(`document.querySelector('[data-composition-add-authored="15"]').value`),'Retained unapplied inspector text.');
      assert.equal(await evaluate(`document.querySelector('[data-composition-inspect-output="15"]').getAttribute("aria-pressed")`),'true');
      const navigator=await evaluate(`(()=>{const list=document.querySelector('.composition-output-navigator'),n=list.querySelector('[aria-pressed=true]');const a=list.getBoundingClientRect(),b=n.getBoundingClientRect();return {scroll:list.scrollTop,visible:b.top>=a.top-1&&b.bottom<=a.bottom+1}})()`);
      assert.ok(navigator.scroll>0&&navigator.visible,'Selected output must stay visible in its scrollable inspector list');
      await click('[data-composition-tab=source]');
      // Controller path only; actual native select input remains an explicit open gate.
      await evaluate(`(()=>{const select=document.querySelector('[data-composition-range-output]');for(const value of ['0','15']){select.value=value;select.dispatchEvent(new Event('change',{bubbles:true}));}})()`);
      const sourceSelection=await evaluate(`(()=>{const list=document.querySelector('.composition-output-navigator'),n=list.querySelector('[aria-pressed=true]');const a=list.getBoundingClientRect(),b=n.getBoundingClientRect();return {index:n.dataset.compositionInspectOutput,visible:b.top>=a.top-1&&b.bottom<=a.bottom+1}})()`);
      assert.equal(sourceSelection.index,'15');assert.equal(sourceSelection.visible,true);
      evidence.interactions.push('source_output_change_controller_path_reveals_selected_inspector_row');
      evidence.interactions.push('inspector_navigates_sixteen_outputs_without_losing_unapplied_text');
    }
    for(const view of ['recipe','graph']) {
      await click(`[data-composition-tab="${view}"]`);
      for(const [width,height] of [[320,900],[768,1024],[1440,1000]]) for(const mode of ['dark','light']) {
        await page('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
        await evaluate(`document.documentElement.dataset.theme=${JSON.stringify(mode)};window.scrollTo(0,0)`);
        const geometry=await evaluate(`({overflow:Math.max(0,document.documentElement.scrollWidth-innerWidth),outputs:document.querySelectorAll('[data-composition-inspect-output]').length,targets:[...document.querySelectorAll('[data-composition-inspect-output]')].map(n=>({height:n.getBoundingClientRect().height,right:n.getBoundingClientRect().right})),saveDisabled:document.querySelector('#composition-save').disabled,ackDisabled:document.querySelector('[data-composition-ack]').disabled})`);
        if(geometry.overflow) evidence.inspectorOverflow=await evaluate(`([...document.querySelectorAll('#qa-inspector-root *')].filter(n=>n.getBoundingClientRect().width&&n.getBoundingClientRect().right>innerWidth).map(n=>({tag:n.tagName,cls:n.className,right:n.getBoundingClientRect().right})).slice(0,20))`);
        assert.equal(geometry.overflow,0,`${fixture.name}/${view} overflow ${width}/${mode}`);
        assert.equal(geometry.outputs,16);assert.ok(geometry.targets.every(n=>n.height>=44&&n.right<=width));
        assert.equal(geometry.saveDisabled,true);assert.equal(geometry.ackDisabled,true);
        evidence.surfaces.push({area:'inspector-fixture',fixture:fixture.name,view,width,height,mode,...geometry});
        await screenshot(`inspector-${fixture.name}-${view}-${width}-${height}-${mode}.png`);
        if(fixture.name==='many-outputs') {
          await evaluate('document.querySelector(".composition-inspector").scrollIntoView({block:"start",behavior:"instant"})');
          await screenshot(`inspector-detail-${view}-${width}-${height}-${mode}.png`);
        }
      }
    }
  }
  await evaluate('window.__qaInspectorBinding.dispose({preserveCache:false});delete window.__qaInspectorBinding;delete window.__qaInspectorState;delete window.__qaInspectorModule');
  evidence.interactions.push('inspector_long_title_recovery_and_busy_display_cases_keep_save_closed');
  evidence.phase='icon_gallery';
  await page('Page.navigate', {url:`${origin}/icon-gallery.html`});
  await until(`document.querySelectorAll(".icon-gallery__card").length === ${catalogKeys.length}`);
  for (const [width,height] of [[320,900],[768,1024],[1440,1000]]) for (const mode of ['dark','light']) {
    await page('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
    if(await evaluate('document.documentElement.dataset.theme')!==mode) await click('#gallery-theme');
    await evaluate('window.scrollTo(0,0);document.fonts.ready');
    await until('[...document.querySelectorAll("#gallery-software img")].every(n=>n.complete&&n.naturalWidth>0)');
    const geometry=await evaluate(`({overflow:Math.max(0,document.documentElement.scrollWidth-innerWidth),software:[...document.querySelectorAll('#gallery-software img')].map(n=>({loaded:n.complete&&n.naturalWidth>0,src:n.getAttribute('src')})),icons:document.querySelectorAll('.icon-gallery__card svg').length})`);
    await until('[...document.querySelectorAll("#gallery-software img")].every(n=>n.complete&&n.naturalWidth>0)');
    assert.equal(geometry.overflow,0,`Gallery overflow ${width}/${mode}`);
    assert.equal(geometry.icons,catalogKeys.length*4);assert.equal(geometry.software.length,8);
    evidence.surfaces.push({area:'icon-gallery',width,height,mode,...geometry});
    await screenshot(`icon-gallery-${width}-${height}-${mode}.png`);
  }
  await page('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  await evaluate('[...document.querySelectorAll(".icon-gallery__group")].find(n=>n.querySelector("h2").textContent==="Storage and state").scrollIntoView()');
  await screenshot('icon-gallery-storage-light.png');


  evidence.phase='skills_profiles';
  await page('Page.navigate', { url: `${origin}/analyze.html?qa=open#skills` });
  await until('Boolean(document.querySelector("[data-capability]"))');
  // Fixture switches are test signals, not product controls. Keep them out of hit testing.
  await evaluate(`(() => { for(const n of document.querySelectorAll('body>[id^=qa-]:not(output),body>#workspace-lock-qa')) n.style.display='none'; const badge=document.createElement('style');badge.id='qa-render-label';badge.textContent='#analysis-main::before{content:"Synthetic browser review · no native data";display:block;pointer-events:none;padding:4px 0;margin-bottom:8px;font:10px system-ui;color:CanvasText}';document.head.append(badge); })()`);
  await click('[data-capability]'); await until('Boolean(document.querySelector("#skills-open-compose"))');
  const painted=await evaluate(`({color:getComputedStyle(document.querySelector('#skills-open-compose .foldline-color')).display,mono:getComputedStyle(document.querySelector('#skills-open-compose .foldline-monochrome')).display,fills:[...document.querySelectorAll('#skills-open-compose .foldline-color [fill]')].map(n=>n.getAttribute('fill'))})`);
  assert.notEqual(painted.color,'none');assert.equal(painted.mono,'none');assert.ok(new Set(painted.fills).size>=2);

  const profiles = [
    [320, 900, 1], [768, 1024, 1], [1366, 768, 1], [1440, 1000, 1],
    [1920, 1080, 1], [1920, 1080, 2], [3840, 2160, 1],
  ];
  for (const [width, height, dpr] of profiles) for (const mode of ['dark', 'light']) {
    await page('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dpr, mobile: false });
    await theme(mode); await evaluate('window.scrollTo(0,0); document.querySelector("#analysis-main")?.scrollTo(0,0)');
    await evaluate('document.fonts.ready');
    const geometry = await evaluate(`(() => { const rect=s=>{const n=document.querySelector(s),r=n?.getBoundingClientRect();return r?{x:r.x,y:r.y,width:r.width,height:r.height}:null};return {width:innerWidth,height:innerHeight,dpr:devicePixelRatio,overflow:Math.max(0,document.documentElement.scrollWidth-innerWidth),editor:rect('.skills-editor'),content:rect('#skills-content-input'),inspector:rect('.skills-inspection'),provenance:rect('.skills-provenance'),actions:[...document.querySelectorAll('.skills-actions button')].map(n=>({id:n.id,x:n.getBoundingClientRect().x,y:n.getBoundingClientRect().y,width:n.getBoundingClientRect().width,height:n.getBoundingClientRect().height,right:n.getBoundingClientRect().right,bottom:n.getBoundingClientRect().bottom})),titleFont:getComputedStyle(document.querySelector('#skills-title-input')).fontFamily};})()`);
    assert.deepEqual(geometry.actions.map(action => action.id).sort(), ['skills-save','skills-review','skills-discard','skills-open-compile','skills-open-compose'].sort(), `Missing or duplicate revision action at ${width}/${mode}`);
    assert.ok(geometry.editor && geometry.editor.width > 0 && geometry.content && geometry.content.height >= 180, `Editor/content missing at ${width}/${mode}`);
    assert.ok(geometry.actions.every(action => [action.x,action.y,action.width,action.height,action.right,action.bottom].every(Number.isFinite) && action.x >= 0 && action.y >= 0 && action.right <= geometry.width), `Invalid action geometry at ${width}/${mode}`);
    assert.equal(geometry.overflow, 0, `Horizontal overflow at ${width}/${mode}`);
    assert.ok(geometry.inspector && geometry.provenance && geometry.provenance.width >= geometry.inspector.width - 2, `Inspector implicit-track squeeze at ${width}/${mode}`);
    assert.ok(geometry.actions.every(action => action.height >= 44 && action.width >= 44), `Action target below 44px at ${width}/${mode}`);
    if (width >= 1240) assert.ok(geometry.actions.every(action => action.bottom > 0 && action.bottom <= height), `Actions below fold at ${width}/${mode}`);
    if (width > 800) {
      const controls=await evaluate(`['#analysis-theme','#workspace-lock-open'].map(s=>{const n=document.querySelector(s),r=n?.getBoundingClientRect();return r?{selector:s,top:r.top,bottom:r.bottom,height:r.height}:null})`);
      assert.ok(controls.every(n=>n&&n.top>=0&&n.bottom<=height&&n.height>=44),`Global controls unreachable at ${width}/${mode}: ${JSON.stringify(controls)}`);
    }
    evidence.viewports.push({ mode, ...geometry });
    if ([320, 768, 1366, 1440].includes(width)) await screenshot(`skills-${width}-${height}-${mode}.png`);
  }
  await page('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await theme('dark');
  evidence.phase='task_interactions';
  // Exercise discovery controls with real input, not direct model mutation.
  const contentCalls = 'JSON.parse(document.querySelector("#qa-skills-calls").textContent).filter(name => !["get_workspace_control_state","get_workspace_close_state","get_workspace_request_state"].includes(name))';
  const callsBeforeFilters = await evaluate(contentCalls);
  await click('#skills-filter-summary');
  await until('document.querySelector(".skills-filter-options").open');
  await click('#skills-library-query');
  await page('Input.insertText', { text: 'Review guidelines' });
  await until('document.querySelectorAll("[data-capability]").length === 1');
  assert.ok(await evaluate('document.querySelector(".skills-library__hidden") && document.querySelector("#skills-title-input").value === "Project instructions"'));
  await key('Escape'); await until('document.querySelectorAll("[data-capability]").length === 2');
  // Headless macOS native select menus require a display. Exercise the real
  // controller change path explicitly; do not count this as native menu input.
  await evaluate('(() => {const n=document.querySelector("#skills-review-filter");n.value="reviewed";n.dispatchEvent(new Event("change",{bubbles:true}));})()');
  await until('document.querySelector("#skills-review-filter").value === "reviewed"');
  assert.equal(await evaluate('document.querySelectorAll("[data-capability]").length'), 0);
  assert.ok(await evaluate('document.querySelector("#skills-filter-summary").textContent.includes("Reviewed")'));
  assert.ok(await evaluate('document.body.innerText.includes("No saved skills match these filters.")'));
  await click('#skills-clear-library-filters');
  await until('document.querySelectorAll("[data-capability]").length === 2');
  assert.deepEqual(await evaluate(contentCalls), callsBeforeFilters);
  evidence.interactions.push('pointer_search_escape_clear_and_hidden_selection_without_content_calls');
  evidence.interactions.push('facets_programmatic_change_no_match_controller_path');
  evidence.nativeSelectInput = 'unqualified_headless_macos';
  await route('merge'); await route('skills');
  assert.ok(await evaluate('document.querySelector(".skills-filter-options").open'));
  await click('#skills-filter-summary');
  await click('#skills-library-query'); await page('Input.insertText', { text: 'Project' });
  await until('document.querySelectorAll("[data-capability]").length === 1');
  assert.equal(await evaluate('document.querySelector(".skills-filter-options").open'), false);
  await key('Escape'); await until('document.querySelectorAll("[data-capability]").length === 2');
  evidence.interactions.push('filter_disclosure_survives_rerender_and_navigation');
  await click('#skills-review');
  await until('document.body.innerText.includes("synthetic_mutation_refused")');
  assert.equal(await evaluate('document.querySelector("#skills-review").disabled'), false);
  assert.ok(await evaluate('document.querySelector(".skills-record-summary").textContent.includes("Not locally reviewed")'));
  evidence.interactions.push('refused_review_never_marks_revision_reviewed');
  await page('Emulation.setDeviceMetricsOverride', { width: 320, height: 900, deviceScaleFactor: 1, mobile: false });
  // Wait for the responsive media-query handler before toggling its drawer.
  await until('innerWidth === 320 && document.querySelector("[data-workbench-nav-drawer]").open === false');
  await click('[data-workbench-nav-drawer]>summary');
  await until('document.querySelector("[data-workbench-nav-drawer]").open === true');
  await click('[data-workbench-nav] a[href="#merge"]');
  await until('location.hash === "#merge" && document.querySelector(".analysis-shell")?.dataset.area === "merge"');
  await click('[data-workbench-nav-drawer]>summary');
  await click('[data-workbench-nav] a[href="#skills"]');
  await until('location.hash === "#skills" && document.querySelector(".analysis-shell")?.dataset.area === "skills" && document.querySelector("[data-workbench-nav-drawer]").open === false');
  assert.equal(await evaluate('document.querySelector("[data-workbench-nav-drawer]").open'), false);
  evidence.interactions.push('compact_navigation_pointer_roundtrip');
  await page('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await click('[data-revision]'); await until('document.querySelector("#skills-content-input")?.readOnly === true');
  await click('#skills-open-compose'); await until('location.hash === "#merge" && Boolean(document.querySelector("[data-composition-replace-revision]"))');
  assert.equal(await evaluate('document.querySelector("[data-composition-replace-revision]").value'), `revision:${'4'.repeat(64)}`);
  assert.equal(await evaluate('document.querySelector("#composition-selection-help").textContent'), '1 revision selected. Add 1 more saved revision to begin.');
  assert.equal(await evaluate('document.querySelector("#composition-start").disabled'), true);
  assert.equal(await evaluate('document.querySelector("#composition-start").getAttribute("aria-describedby")'), 'composition-selection-help');
  evidence.interactions.push('compose_explains_missing_input_without_starting_draft');
  evidence.interactions.push('historical_revision_pointer_handoff'); await screenshot('compose-historical-dark.png');
  evidence.phase='compose_draft';
  await click('.composition-capability:last-of-type>summary');
  await click('[data-composition-add-revision="capability:' + '6'.repeat(64) + '"]');
  await until('document.querySelector("#composition-start")?.disabled === false');
  await click('#composition-start');
  await until('Boolean(document.querySelector("[data-composition-tab=recipe]"))');
  for (const view of ['recipe','graph','source']) {
    await click(`[data-composition-tab="${view}"]`);
    assert.equal(await evaluate(`document.querySelector('[data-composition-panel="${view}"]').hidden`),false);
    assert.equal(await evaluate(`document.querySelector('[data-composition-tab="${view}"]').getAttribute('aria-pressed')`),'true');
  }
  await click('#composition-request-preview');
  await until('document.querySelector(".composition-page").getAttribute("aria-busy") === "false"');
  assert.equal(await evaluate('document.querySelector("#composition-save").disabled'),true);
  assert.equal(await evaluate('document.querySelector("[data-composition-ack]").disabled'),true);
  for (const [width,height] of [[320,900],[768,1024],[1440,1000]]) for (const mode of ['dark','light']) {
    await page('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
    await theme(mode);await evaluate('window.scrollTo(0,0)');
    const geometry=await evaluate(`({overflow:Math.max(0,document.documentElement.scrollWidth-innerWidth),guide:document.querySelectorAll('.composition-guide li').length,revisionSelectors:[...document.querySelectorAll('.composition-revision-choice select')].map(n=>({right:n.getBoundingClientRect().right,container:n.closest('.composition-inputs').getBoundingClientRect().right})),tools:[...document.querySelectorAll('.composition-tabs button')].map(n=>({height:n.getBoundingClientRect().height,width:n.getBoundingClientRect().width}))})`);
    if (geometry.overflow) evidence.composeOverflow = await evaluate(`([...document.querySelectorAll('.composition-page *')].filter(n=>{const r=n.getBoundingClientRect();return r.width&&r.right>innerWidth}).map(n=>({tag:n.tagName,cls:n.className,id:n.id,width:n.getBoundingClientRect().width,right:n.getBoundingClientRect().right,scroll:n.scrollWidth})).slice(0,30))`);
    assert.equal(geometry.overflow,0,`Compose draft overflow ${width}/${mode}`);
    assert.equal(geometry.guide,4);assert.ok(geometry.tools.every(n=>n.height>=44&&n.width>0));
    assert.ok(geometry.revisionSelectors.every(n=>n.right<=n.container));
    evidence.surfaces.push({area:'compose-draft',width,height,mode,...geometry});
    await screenshot(`compose-draft-${width}-${height}-${mode}.png`);
  }
  await page('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  await theme('dark');
  await evaluate('document.querySelector("[data-composition-tab=graph]").focus()');await key('Enter');
  assert.equal(await evaluate('document.querySelector("[data-composition-panel=graph]").hidden'),false);
  await evaluate('document.querySelector("[data-composition-tab=source]").focus()');await key('Enter');
  assert.equal(await evaluate('document.querySelector("[data-composition-panel=source]").hidden'),false);
  assert.notEqual(await evaluate('getComputedStyle(document.querySelector("[data-composition-tab=source]")).outlineStyle'),'none');
  evidence.interactions.push('compose_view_keyboard_activation_and_focus');
  for (const [name,value] of [['prefers-reduced-motion','reduce'],['forced-colors','active']]) {
    await page('Emulation.setEmulatedMedia',{features:[{name,value}]});
    assert.ok(await evaluate('document.querySelector("[data-composition-tab=source]").getBoundingClientRect().height>=44'));
    if(name==='prefers-reduced-motion') assert.equal(await evaluate('getComputedStyle(document.querySelector("[data-composition-tab=source]")).transitionDuration'),'0s');
    else assert.notEqual(await evaluate('getComputedStyle(document.querySelector("[data-composition-tab=source]")).borderTopStyle'),'none');
    await screenshot(`compose-${name}.png`);
  }
  await page('Emulation.setEmulatedMedia',{features:[]});
  evidence.interactions.push('compose_reduced_motion_and_forced_colors');
  for (const accept of [false,true]) {
    expectedDialog = { message: 'Discard this in-memory composition draft? Selected inputs remain.', accept, seen: false };
    await click('#composition-reset');
    assert.equal(expectedDialog.seen,true);
    await until(accept ? 'Boolean(document.querySelector("#composition-start"))' : 'Boolean(document.querySelector("[data-composition-tab=source]"))');
    expectedDialog = null;
  }
  evidence.interactions.push('compose_discard_pointer_cancel_then_confirm');
  evidence.interactions.push('compose_local_draft_views_and_refused_preview_keep_save_closed');

  await route('skills'); await click('#skills-return-current'); await until('document.querySelector("#skills-content-input")?.readOnly === false');
  await evaluate('document.querySelector("#skills-content-input").focus();document.querySelector("#skills-content-input").select()');
  await page('Input.insertText', { text: 'UI QA retained draft\n' });
  assert.equal(await evaluate('document.querySelector("#skills-open-compose").disabled'), true);
  await click('#skills-save');
  await until('document.body.innerText.includes("synthetic_mutation_refused")');
  assert.equal(await evaluate('document.querySelector("#skills-content-input").value'), 'UI QA retained draft\n');
  assert.equal(await evaluate('document.querySelector("#skills-save").disabled'), false);
  evidence.interactions.push('refused_save_preserves_editable_unsaved_content');
  await route('merge'); await route('skills');
  assert.equal(await evaluate('document.querySelector("#skills-content-input").value'), 'UI QA retained draft\n');
  await click('#skills-discard'); await click('#skills-discard-cancel');
  assert.equal(await evaluate('document.querySelector("#skills-content-input").value'), 'UI QA retained draft\n');
  await click('#skills-discard'); await click('#skills-discard-confirm');
  evidence.interactions.push('dirty_draft_navigation_and_explicit_discard');
  await evaluate('document.querySelector("#skills-open-compile").focus()');
  await page('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 36, text: '\r', unmodifiedText: '\r' });
  await page('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await until('location.hash === "#compile" && Boolean(document.querySelector("#compile-title"))');
  evidence.interactions.push('keyboard_compile_handoff'); await screenshot('compile-dark.png');
  await route('skills'); await evaluate('document.querySelector("#qa-skills-controls").open=true');
  await evaluate('document.querySelector("#qa-skills-fail-open").click()'); await click('[data-capability="capability:' + '6'.repeat(64) + '"]');
  await until('document.body.innerText.includes("Synthetic open failure.")');
  evidence.interactions.push('failed_read_keeps_current_editor');
  await evaluate('document.querySelector("#qa-skills-fail-list").click()'); await click('#skills-reload-list');
  await until('document.body.innerText.includes("Library unavailable. Prior results may be stale.")');
  assert.ok(await evaluate('Boolean(document.querySelector("[data-capability]"))'));
  evidence.interactions.push('failed_refresh_retains_loaded_library');
  await click('[data-capability]'); await until('!document.querySelector("#skills-open-compile").disabled');
  await evaluate('document.querySelector("#skills-content-input").focus();document.querySelector("#skills-content-input").select()');
  await page('Input.insertText', { text: 'Conflict draft stays here\n' });
  await evaluate('document.querySelector("#qa-skills-conflict").click()'); await click('#skills-save');
  await until('Boolean(document.querySelector("#skills-reload-current"))');
  assert.equal(await evaluate('document.querySelector("#skills-content-input").value'), 'Conflict draft stays here\n');
  assert.equal(await evaluate('document.querySelector("#skills-open-compose").disabled'), true);
  await screenshot('skills-conflict-dark.png');
  await click('#skills-reload-current'); await until('!document.querySelector("#skills-reload-current")');
  await click('#skills-discard'); await click('#skills-discard-confirm');
  evidence.interactions.push('conflict_preserves_draft_and_requires_explicit_reload');
  await evaluate('document.querySelector("#qa-skills-hold").click()'); await click('#skills-open-compile');
  await until('document.querySelector("#skills-open-compile")?.disabled === true');
  await route('merge'); await evaluate('document.querySelector("#qa-skills-release").click()'); await delay(200);
  assert.equal(await evaluate('location.hash'), '#merge'); evidence.interactions.push('late_compile_reply_does_not_navigate');
  for (const setting of [{ name: 'prefers-reduced-motion', value: 'reduce' }, { name: 'forced-colors', value: 'active' }]) {
    await page('Emulation.setEmulatedMedia', { features: [setting] }); await route('skills');
    assert.equal(await evaluate('Math.max(0,document.documentElement.scrollWidth-innerWidth)'), 0);
    await screenshot(`skills-${setting.name}.png`);
    if (setting.name === 'prefers-reduced-motion') assert.ok(['0s', 'none'].includes(await evaluate('getComputedStyle(document.querySelector("#skills-open-compose")).transitionDuration')));
    if(setting.name==='forced-colors') {
      const fallback=await evaluate(`({color:getComputedStyle(document.querySelector('#skills-open-compose .foldline-color')).display,mono:getComputedStyle(document.querySelector('#skills-open-compose .foldline-monochrome')).display})`);
      assert.equal(fallback.color,'none');assert.notEqual(fallback.mono,'none');
    }
    evidence.interactions.push(setting.name);
  }
  await page('Emulation.setEmulatedMedia', { features: [] });
  evidence.phase='route_profiles';
  // Inspect affected real route renderers with the same synthetic transport.
  for (const area of ['merge', 'compile', 'workspace', 'authority-engines']) {
    await route(area);
    await until('document.querySelector("[aria-busy=true]") === null');
    for (const [width, height] of [[320,900],[768,1024],[1440,1000]]) for (const mode of ['dark','light']) {
      await page('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await theme(mode); await evaluate('window.scrollTo(0,0)');
      const geometry = await evaluate(`(() => {
        const main=document.querySelector('#analysis-main');
        const icons=[...main.querySelectorAll('svg.foldline-icon')].filter(n=>n.getBoundingClientRect().width);
        return {overflow:Math.max(0,document.documentElement.scrollWidth-innerWidth),icons:icons.length,softwareMarks:[...main.querySelectorAll('img')].filter(n=>n.complete&&n.naturalWidth>0&&n.getBoundingClientRect().width>0).length,badIcons:icons.filter(n=>n.getAttribute('aria-hidden')!=='true'||n.getAttribute('focusable')!=='false').length};
      })()`);
      assert.equal(geometry.overflow,0,`${area} overflow at ${width}/${mode}`);
      assert.ok(geometry.icons+geometry.softwareMarks>0,`${area} missing icons or software marks`); assert.equal(geometry.badIcons,0);
      evidence.surfaces.push({area,width,height,mode,...geometry});
      await screenshot(`${area}-${width}-${height}-${mode}.png`);
    }
  }
  await route('skills');
  await page('Emulation.setDeviceMetricsOverride', { width:1440,height:1000,deviceScaleFactor:1,mobile:false });
  await theme('dark');
  evidence.phase='lock_cancel';
  evidence.pending=['native_select_pointer_keyboard','lock_pointer_confirmation'];
  await evaluate('document.querySelector("#workspace-lock-open").focus()');await key('Enter');
  await until('Boolean(document.querySelector("#workspace-lock-confirm"))');
  await page('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 53 });
  await page('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 53 });
  await until('!document.querySelector("#workspace-lock-confirm")');
  assert.ok(await evaluate('Boolean(document.querySelector("#skills-content-input"))'));
  evidence.interactions.push('lock_confirmation_escape_keeps_editor');
  evidence.phase='lock_purge';
  evidence.pending=['native_select_pointer_keyboard','lock_pointer_confirmation'];
  await evaluate('document.querySelector("#workspace-lock-open").focus()');await key('Enter');
  await until('Boolean(document.querySelector("#workspace-lock-confirm"))');
  // Headless macOS pointer dispatch stalled on this browser dialog in retained runs.
  // Exercise real keyboard activation; retain pointer confirmation as an open gate.
  evidence.pending=['native_select_pointer_keyboard','lock_pointer_confirmation'];
  await evaluate('document.querySelector("#workspace-lock-confirm").focus()');
  await key('Enter'); await until('!document.querySelector("#skills-content-input")');
  evidence.interactions.push('synthetic_lock_purges_editor_keyboard');
  evidence.phase='saved_result_handoff';
  await page('Page.navigate',{url:`${origin}/analyze.html?qa=open&handoff=1#skills`});
  await until('Boolean(document.querySelector("[data-capability]"))');
  await evaluate(`(()=>{for(const n of document.querySelectorAll('body>[id^=qa-]:not(output),body>#workspace-lock-qa'))n.style.display='none';const label=document.createElement('div');label.id='qa-memory-only-badge';label.textContent='Synthetic memory-only save · no native persistence';label.style.cssText='position:fixed;bottom:8px;right:12px;z-index:20000;pointer-events:none;max-width:calc(100vw - 24px);padding:5px 8px;background:Canvas;color:CanvasText;border:1px solid GrayText;font:11px system-ui';document.body.append(label);})()`);
  await page('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  await click('[data-capability]');await until('Boolean(document.querySelector("#skills-open-compose"))');
  await click('#skills-open-compose');await until('location.hash === "#merge" && Boolean(document.querySelector("#composition-start"))');
  await click('.composition-capability:last-of-type>summary');
  await click('[data-composition-add-revision="capability:'+'6'.repeat(64)+'"]');
  await until('document.querySelector("#composition-start").disabled === false');
  await click('#composition-start');await until('Boolean(document.querySelector("#composition-request-preview"))');
  await click('#composition-request-preview');await until('document.querySelector("[data-composition-ack]").disabled === false');
  await click('[data-composition-ack]');await until('document.querySelector("#composition-save").disabled === false');
  await click('#composition-save');await until('Boolean(document.querySelector("[data-composition-open-skill]"))');
  const resultSelector='[data-composition-open-skill="capability:'+'9'.repeat(64)+'"]';
  const resultRevision=`revision:${'9'.repeat(64)}`;
  assert.equal(await evaluate(`document.querySelector('${resultSelector}').dataset.compositionResultRevision`),resultRevision);
  assert.ok(await evaluate(`(()=>{const n=document.querySelector('#qa-memory-only-badge'),r=n.getBoundingClientRect();return n.textContent==='Synthetic memory-only save · no native persistence'&&r.width>0&&r.height>0&&r.top>=0&&r.bottom<=innerHeight&&getComputedStyle(n).pointerEvents==='none'})()`));
  await screenshot('saved-result-composition.png');
  await click(resultSelector);
  await until('location.hash === "#skills" && document.activeElement?.id === "skills-editor-title"');
  assert.equal(await evaluate('document.querySelector(".skills-history [aria-pressed=true]").dataset.revision'),resultRevision);
  assert.equal(await evaluate('document.querySelector("#skills-content-input").value'),'Keep changes bounded.\nReview before publishing.\n\n\nReview the exact diff.\n');
  assert.ok(await evaluate('document.querySelector(".skills-record-summary").textContent.includes("historical")'));
  assert.equal(await evaluate('document.querySelectorAll("[data-capability]").length'),3);
  assert.ok(await evaluate(`(()=>{const n=document.querySelector('#qa-memory-only-badge'),r=n.getBoundingClientRect();return n.textContent==='Synthetic memory-only save · no native persistence'&&r.width>0&&r.height>0&&r.top>=0&&r.bottom<=innerHeight&&getComputedStyle(n).pointerEvents==='none'})()`));
  await screenshot('saved-result-skills-focus.png');
  evidence.interactions.push('saved_result_exact_historical_revision_routes_to_skills_with_focus');
  const resultReads=`JSON.parse(document.querySelector('#qa-composition-requests').textContent).filter(n=>n.command==='open_capability'&&n.capabilityId==='capability:${'9'.repeat(64)}')`;
  assert.equal((await evaluate(resultReads)).at(-1).revisionId,resultRevision);
  await click('[data-revision="revision:'+'a'.repeat(64)+'"]');
  await until('document.querySelector("#skills-content-input")?.readOnly === false');
  await click('#skills-content-input');await page('Input.insertText',{text:'Retained result draft.'});
  await route('merge');const dirtyReads=(await evaluate(resultReads)).length;
  await click(resultSelector);await until('document.body.innerText.includes("Save or discard this skill")');
  assert.equal((await evaluate(resultReads)).length,dirtyReads);assert.equal(await evaluate('location.hash'),'#merge');
  await route('skills');assert.ok(await evaluate('document.querySelector("#skills-content-input").value.includes("Retained result draft.")'));
  await click('#skills-discard');await click('#skills-discard-confirm');
  evidence.interactions.push('saved_result_dirty_draft_blocks_read_and_preserves_edits');
  await evaluate('document.querySelector("#qa-skills-hold").click()');
  await click('[data-capability="capability:'+'3'.repeat(64)+'"]');
  await until('document.querySelector(".skills-page").getAttribute("aria-busy") === "true"');
  await route('merge');const busyReads=(await evaluate(resultReads)).length;
  await click(resultSelector);await delay(100);assert.equal(await evaluate('location.hash'),'#merge');
  assert.equal((await evaluate(resultReads)).length,busyReads);
  await evaluate('document.querySelector("#qa-skills-release").click()');
  await until(`JSON.parse(document.querySelector('#qa-composition-requests').textContent).filter(n=>n.command==='open_capability'&&n.capabilityId==='capability:${'3'.repeat(64)}').at(-1).completed === true`);
  evidence.interactions.push('saved_result_busy_skill_operation_blocks_extra_read');
  await evaluate('document.querySelector("#qa-composition-hold").click()');
  await click(resultSelector);await until('document.body.innerText.includes("Opening saved revision")');
  await route('compile');await evaluate('document.querySelector("#qa-composition-release").click()');
  await until(`(${resultReads}).at(-1).completed === true`);
  await delay(150);assert.equal(await evaluate('location.hash'),'#compile');
  evidence.interactions.push('saved_result_late_read_after_navigation_does_not_redirect');
  await route('merge');
  await evaluate('document.querySelector("#qa-composition-hold").click()');
  await click(resultSelector);await until('document.body.innerText.includes("Opening saved revision")');
  await evaluate('document.querySelector("#workspace-lock-open").focus()');await key('Enter');
  await until('Boolean(document.querySelector("#workspace-lock-confirm"))');
  await evaluate('document.querySelector("#workspace-lock-confirm").focus()');await key('Enter');
  await until('!document.querySelector("#skills-content-input") && !document.querySelector("[data-composition-open-skill]")');
  await evaluate('document.querySelector("#qa-composition-release").click()');
  await until(`(${resultReads}).at(-1).completed === true`);await delay(150);
  assert.equal(await evaluate('Boolean(document.querySelector("#skills-content-input"))'),false);
  assert.equal(await evaluate('location.hash'),'#merge');
  await until(`Boolean(document.querySelector('[data-workspace-close="protection"][data-workspace-layout="locked"][data-workspace-pending="false"]'))`);
  evidence.interactions.push('saved_result_late_read_after_lock_does_not_restore_content');
  assert.deepEqual(evidence.exceptions, []); assert.deepEqual(evidence.consoleErrors, []); assert.deepEqual(evidence.blockedOrigins, []);
  evidence.status = 'checks_passed';
} catch (error) {
  evidence.status = 'FAIL'; evidence.failure = error.message; process.exitCode = 1;
  try { evidence.pageState = await evaluate('({hash:location.hash,active:document.activeElement?.id,requests:document.querySelector("#qa-skills-requests")?.textContent,body:document.body.innerText.slice(-1600)})'); } catch { /* No page during startup failures. */ }
  try { await screenshot('failure.png'); } catch { /* Startup failures may have no page. */ }
} finally {
  stopping = true;
  const cleanup = { browserExited: false, profileRemoved: false, serverClosed: false, forcedTermination: false, errors: [] };
  // Stop interception and close only our tab before asking the owned browser to exit.
  if (socket?.readyState === WebSocket.OPEN && session) {
    try {
      await bounded(page('Fetch.disable'), 3000, 'Interception drain');
      await bounded(page('Runtime.evaluate', {expression:'document.querySelector("#qa-memory-only-badge")?.remove()',returnByValue:true}),3000,'Synthetic badge dispose');
      await bounded(page('Page.stopLoading'), 3000, 'Page stop');
      await bounded(page('Page.navigate',{url:'about:blank'}),3000,'Owned page unload');
      cleanup.pageUnloaded = true;
      if (ownedTarget) {
        const result = await bounded(call('Target.closeTarget', { targetId: ownedTarget }), 3000, 'Owned tab close');
        if (!result.success) throw Error('Owned tab did not close.');
      }
      cleanup.targetClosed = true;
    } catch (error) { cleanup.errors.push(error.message); }
  }
  try { if (socket?.readyState === WebSocket.OPEN) await bounded(call('Browser.close'), 3000, 'Browser close request'); }
  catch { /* Exit proof below is authoritative when Chrome closes without an acknowledgement. */ }
  try { cleanup.exit = browserExit ? await bounded(browserExit, 10000, 'Browser exit') : { notStarted: true }; }
  catch (error) {
    cleanup.errors.push(error.message); cleanup.forcedTermination = true;
    chrome?.kill('SIGTERM');
    try { cleanup.exit = await bounded(browserExit, 3000, 'Terminated browser exit'); }
    catch {
      chrome?.kill('SIGKILL');
      try { cleanup.exit = await bounded(browserExit, 3000, 'Killed browser exit'); }
      catch (error) { cleanup.errors.push(error.message); }
    }
  }
  cleanup.browserExited = browserExited || !chrome;
  if (cleanup.exit && !cleanup.exit.notStarted && (cleanup.exit.code !== 0 || cleanup.exit.signal)) cleanup.errors.push('Browser did not exit normally.');
  socket?.close();
  for (const request of requests.values()) { clearTimeout(request.timer); request.reject(Error('QA browser closed')); }
  requests.clear();
  try {
    if (server?.listening) {
      server.closeAllConnections();
      await bounded(new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())), 3000, 'Fixture server close');
    }
    cleanup.serverClosed = true;
  } catch (error) { cleanup.errors.push(error.message); }
  if (cleanup.browserExited) {
    try { if (profile) await rm(profile, { recursive: true, force: true }); cleanup.profileRemoved = true; }
    catch (error) { cleanup.errors.push(error.message); }
  } else cleanup.errors.push('Profile retained because owned browser exit is unconfirmed.');
  if (cleanup.errors.length) cleanup.browserDiagnostic = browserDiagnostic;
  evidence.cleanup = cleanup;
  const verdict=classifyUiQaResult({
    checksPassed:evidence.status==='checks_passed',
    cleanupComplete:cleanup.errors.length===0&&cleanup.browserExited&&cleanup.profileRemoved&&cleanup.serverClosed,
    pending:evidence.pending??(evidence.nativeSelectInput==='unqualified_headless_macos'?['native_select_pointer_keyboard']:undefined),
    nativeSelectInput:evidence.nativeSelectInput,
  });
  evidence.status=verdict;
  if(verdict==='FAIL'){evidence.failure||='QA checks or teardown did not complete cleanly.';process.exitCode=1;}
  else if(verdict==='PARTIAL'){evidence.pending??=['native_select_pointer_keyboard'];process.exitCode=2;}
  // Completed checks are durable only after the owned browser/server stop and profile removal.
  // An explicit interaction gap remains PARTIAL even when every executed assertion passed.
  if (output) await writeFile(join(output, 'receipt.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log(JSON.stringify({ status: evidence.status, output, viewports: evidence.viewports.length, surfaces:evidence.surfaces.length, interactions: evidence.interactions.length, cleanup, failure: evidence.failure }));
}
