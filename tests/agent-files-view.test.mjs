import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { renderAgentFilesView } from '../preview/agent-files-view.mjs';

function record(overrides = {}) {
  return {
    key: 'file-1',
    name: 'AGENTS.md',
    sourceId: 'source-1',
    sha256: 'abc123',
    byteLength: 42,
    format: 'agents_markdown',
    origin: 'selected',
    savedAtMs: null,
    state: 'unchecked',
    findings: [],
    error: null,
    report: { source: { content: '# Agent instructions\nUse local checks.' } },
    ...overrides,
  };
}

function state(overrides = {}) {
  return {
    status: 'ready',
    pending: null,
    records: [record()],
    selectedKey: 'file-1',
    markedKeys: [],
    query: '',
    typeFilter: 'all',
    findingFilter: 'all',
    page: 0,
    message: 'One local file is ready.',
    error: null,
    checkCancelled: false,
    ...overrides,
  };
}

test('agent-file view escapes malformed names, source text, and advisory text', () => {
  const html = renderAgentFilesView(state({
    records: [record({
      name: '<img src=x onerror=alert(1)>',
      report: { source: { content: '<script>run()</script>' } },
      findings: [{ ruleId: '<rule>', title: '<finding>', severity: 'review', line: 3, rulePack: 'rangoon.agent-file-checks.v1', explanation: '<unsafe>' }],
    })],
  }));
  assert.doesNotMatch(html, /<script>|<img src=x/);
  assert.match(html, /&lt;script&gt;run\(\)&lt;\/script&gt;/);
  assert.match(html, /&lt;finding&gt;/);
});

test('agent-file view keeps selected and saved files visibly distinct and keeps import navigation independent of marks', () => {
  const html = renderAgentFilesView(state({
    markedKeys: ['file-2'],
    records: [record(), record({ key: 'file-2', name: 'CLAUDE.md', origin: 'saved', format: 'claude_markdown', state: 'checked' })],
  }));
  assert.match(html, /Selected files/);
  assert.match(html, /Saved files/);
  assert.match(html, /data-agent-files-action="check-selected"(?![^>]*disabled)/);
  assert.match(html, /data-agent-files-action="import"(?![^>]*disabled)/);
  assert.match(html, /<details class="agent-files-limits">/);
  assert.match(html, /Open Import &amp; Analyze/);
  assert.match(html, /Selected files remain unsaved\. Saving uses the current one-file Import &amp; Analyze path/);
  assert.match(html, /role="listitem"/);
});

test('unavailable view refuses native selection without claiming browser file access', () => {
  const html = renderAgentFilesView(state({ status: 'unavailable', records: [], selectedKey: null }));
  assert.match(html, /data-agent-files-action="choose"[^>]*disabled/);
  assert.match(html, /data-agent-files-action="import"[^>]*disabled/);
  assert.match(html, /Open this route in Rangoon desktop to choose local files/);
  assert.doesNotMatch(html, /filesystem|file path/i);
});

test('partial failed records retain row truth and safe local error text', () => {
  const html = renderAgentFilesView(state({ records: [record({ state: 'failed', sourceId: null, sha256: null, format: null, report: null, error: { code: 'source_failed', message: 'Could not read <private>' } })] }));
  assert.match(html, /Could not read/);
  assert.match(html, /Could not read &lt;private&gt;/);
  assert.match(html, /Source ID<\/dt><dd>Unavailable/);
});

test('empty and filtered no-match states remain distinct', () => {
  const empty = renderAgentFilesView(state({ records: [], selectedKey: null }));
  assert.match(empty, /Choose Markdown files or reload saved files to begin/);
  const noMatch = renderAgentFilesView(state({ query: 'not-present' }));
  assert.match(noMatch, /No files match these filters/);
  assert.match(noMatch, /No file open|AGENTS\.md/);
});

test('pending check disables mutations but keeps cancel reachable', () => {
  const html = renderAgentFilesView(state({ pending: 'check', markedKeys: ['file-1'] }));
  assert.match(html, /data-agent-files-action="choose"[^>]*disabled/);
  assert.match(html, /data-agent-files-action="check-selected"[^>]*disabled/);
  assert.match(html, /data-agent-files-action="cancel-check"/);
});

test('compact bulk controls avoid inactive actions and zero unavailable count', () => {
  const html = renderAgentFilesView(state());
  assert.match(html, /Checks advisory; original files unchanged/);
  assert.match(html, /data-agent-files-action="select-page"/);
  assert.doesNotMatch(html, /data-agent-files-action="clear-selection"/);
  assert.doesNotMatch(html, /data-agent-files-action="check-selected"/);
  assert.doesNotMatch(html, /<strong>0<\/strong> unavailable/);
});

test('pagination renders fifty rows per page and retains an inspector selection', () => {
  const records = Array.from({ length: 51 }, (_, index) => record({ key: `file-${index}`, name: `agent-${index}.md`, origin: index % 2 ? 'saved' : 'selected' }));
  const html = renderAgentFilesView(state({ records, page: 1, selectedKey: 'file-0' }));
  assert.equal((html.match(/data-agent-files-open=/g) ?? []).length, 1);
  assert.match(html, /Page 2 of 2/);
  assert.match(html, /Open file remains in inspector on another page/);
});

test('findings show source line and rule-pack provenance without compatibility claims', () => {
  const html = renderAgentFilesView(state({ records: [record({ state: 'checked', findings: [{ ruleId: 'risky-curl', title: 'Network instruction', severity: 'review', line: 9, rulePack: 'rangoon.agent-file-checks.v1', explanation: 'Manual review needed.' }] })] }));
  assert.match(html, /line 9/);
  assert.match(html, /rangoon\.agent-file-checks\.v1 · risky-curl/);
  assert.match(html, /Compatibility not assessed/);
  assert.doesNotMatch(html, /outdated|safe|green/i);
});

test('technical details preserve hidden row metadata and selected item order without paths', () => {
  const html = renderAgentFilesView(state({ records: [record({ key: 'selected:8:2', byteLength: 4096, savedAtMs: null })], selectedKey: 'selected:8:2' }));
  assert.match(html, /Selected file · item 3/);
  assert.match(html, /Bytes<\/dt><dd>4096/);
  assert.match(html, /Saved at<\/dt><dd>Not saved locally/);
  assert.doesNotMatch(html, /\/Users\/|C:\\\\/);
});

test('agent-file styles keep controls accessible and stack the inspector', () => {
  const css = readFileSync(new URL('../preview/agent-files.css', import.meta.url), 'utf8');
  assert.match(css, /grid-template-columns:minmax\(0,1fr\) minmax\(280px,320px\)/);
  assert.match(css, /@media\(max-width:1240px\)\{\.agent-files-workbench\{grid-template-columns:1fr/);
  assert.match(css, /\.agent-files-row\{display:grid;grid-template-columns:44px minmax\(0,1fr\)/);
  assert.match(css, /\.agent-files-mark\{display:grid;place-items:center;min-height:44px/);
  assert.match(css, /\.agent-files-table\{min-width:0;max-height:min\(48vh,560px\);overflow:auto/);
  assert.match(css, /\.agent-files-filters\{grid-template-columns:repeat\(2,minmax\(0,1fr\)\)/);
  assert.match(css, /\.agent-files-filters label:first-child\{grid-column:1\/-1/);
  assert.match(css, /min-height:44px/);
  assert.match(css, /prefers-reduced-motion:reduce/);
  assert.match(css, /forced-colors:active/);
});

test('library, inspector and paging targets support deterministic keyboard return', () => {
 const html=renderAgentFilesView({status:'ready',pending:null,records:[],selectedKey:null,markedKeys:[],query:'',typeFilter:'all',findingFilter:'all',page:0,message:'Ready',error:null});
 assert.match(html, /<h2 tabindex="-1">0 matches/);assert.match(html, /data-agent-files-page-status tabindex="-1"/);
});

test('activity shows exact timestamps and explicitly separates snapshots from revision history',()=>{
 const html=renderAgentFilesView(state({records:[record({origin:'saved',savedAtMs:1700000000123,selectedAtMs:null,checkedAtMs:1800000000456})]}));
 assert.match(html,/datetime="2023-11-14T22:13:20.123Z"/);
 assert.match(html,/datetime="2027-01-15T08:00:00.456Z"/);
 assert.match(html,/1700000000123 ms since Unix epoch/);
 assert.match(html,/Saved UTC<\/dt><dd>2023-11-14T22:13:20.123Z · 1700000000123 ms since Unix epoch/);
 assert.match(html,/<span class="agent-files-row__name">AGENTS.md<small class="agent-files-row__time">Saved <time /);
 assert.doesNotMatch(html,/<span class="agent-files-sr">[^<]*<small/);
 assert.match(html,/Revision history/);assert.match(html,/not an ordered revision history/);
 assert.match(html,/File modified<\/dt><dd>Not collected/);
 assert.match(html,/not trusted audit time/);assert.match(html,/last only this session/);
 const invalid=renderAgentFilesView(state({records:[record({savedAtMs:Infinity,checkedAtMs:NaN,selectedAtMs:-1})]}));
 assert.doesNotMatch(invalid,/<time /);
});


test('opened source has a central IDE review pane with safe outline and grouped code',()=>{
 const html=renderAgentFilesView(state({records:[record({report:{source:{content:'# Review\n```json\n{"allow": false}\n```\n'}}})]}));
 assert.match(html,/agent-files-workbench--review/);assert.match(html,/aria-label="Source editor"/);
 assert.match(html,/aria-label="Markdown outline"/);assert.match(html,/data-md-jump="1"/);assert.match(html,/Code · json/);
 assert.match(html,/md-token--keyword/);assert.equal((html.match(/data-agent-files-action="back-to-list"/g)||[]).length,1);
});

test('advisory line jump keeps source evidence separate from markup interpretation',()=>{
 const html=renderAgentFilesView(state({records:[record({findings:[{line:2,severity:'review',title:'Check',explanation:'Local hint'}]})]}));
 assert.match(html,/class="agent-files-finding-line"/);assert.match(html,/data-md-jump="2"/);
 assert.match(html,/Checks advisory; original files unchanged/);
});

test('compact header keeps limits and import reachable without duplicate summary bands',()=>{
 const html=renderAgentFilesView(state());
 const header=html.match(/<header class="agent-files-header">[\s\S]*?<\/header>/)?.[0];
 assert.match(header,/data-agent-files-action="choose"/);assert.match(header,/data-agent-files-action="refresh"/);
 assert.match(header,/<details class="agent-files-limits"><summary>Inspection limits/);assert.match(header,/data-agent-files-action="import"/);
 assert.equal((html.match(/class="agent-files-toolbar"/g)||[]).length,1);assert.equal((html.match(/class="agent-files-summary"/g)||[]).length,1);
 assert.match(html,/Checks advisory; original files unchanged/);
});

test('compact row date retains exact time in title and detailed inspector',()=>{
 const html=renderAgentFilesView(state({records:[record({origin:'saved',savedAtMs:1700000000123})]}));
 const row=html.match(/<small class="agent-files-row__time">([\s\S]*?)<\/small>/)?.[1];
 assert.match(row,/datetime="2023-11-14T22:13:20.123Z"/);assert.match(row,/1700000000123 ms since Unix epoch/);
 assert.match(html,/Saved UTC<\/dt><dd>2023-11-14T22:13:20.123Z/);assert.match(html,/Record activity/);
});


test('saved handoff is explicit and disabled for unsaved, pending and missing bridge',()=>{
 const saved=record({origin:'saved'});
 const enabled=renderAgentFilesView(state({sourceHandoffAvailable:true,records:[saved]}));assert.match(enabled,/data-agent-files-action="open-source"[^>]*aria-describedby="agent-files-handoff-help"\s*>/);
 for(const data of [state(),state({sourceHandoffAvailable:true,pending:'handoff',records:[saved]}),state({sourceHandoffAvailable:false,records:[saved]})])assert.match(renderAgentFilesView(data),/data-agent-files-action="open-source"[^>]*disabled/);
 assert.match(renderAgentFilesView(state({sourceHandoffAvailable:true})),/Selected-file results are temporary/);
 assert.match(renderAgentFilesView(state({sourceHandoffAvailable:true,pending:'handoff',records:[saved]})),/Opening source…/);
});

test('folder rows show relative metadata and a separate native file selection action',()=>{
 const folder=record({key:'folder:1:0',name:'AGENTS.md',relativePath:'project/<b>.md',rootLabel:'work<space>',modifiedAtMs:1700000000000,listedAtMs:1800000000000,origin:'folder',state:'metadata',sourceId:null,sha256:null,report:null});
 const html=renderAgentFilesView(state({records:[folder],selectedKey:folder.key,folder:{rootLabel:'work<space>',visitedEntries:4,excludedEntries:1,partialReasons:['depth_limit']}}));
 assert.match(html,/Choose folder/);assert.match(html,/Folder listing/);assert.match(html,/Metadata only/);assert.match(html,/Contents have not been read/);assert.match(html,/Choose files to inspect/);assert.match(html,/whole-second precision/);assert.match(html,/Folder depth limit reached/);
 assert.match(html,/project\/&lt;b&gt;\.md/);assert.doesNotMatch(html,/<b>\.md|work<space>|data-agent-files-action="open-source"/);assert.match(html,/data-agent-files-mark="folder:1:0"[^>]*disabled/);
});
