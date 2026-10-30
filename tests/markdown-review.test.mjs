import test from 'node:test';
import assert from 'node:assert/strict';
import { parseMarkdownReview, renderMarkdownReview, renderHighlightedLine, renderImportMarkdownLines, jumpToMarkdownLine, captureMarkdownReview, restoreMarkdownReview } from '../preview/markdown-review.mjs';

test('lexical outline respects hierarchy and keeps fenced/frontmatter headings inert', () => {
  const content = '\uFEFF---\r\nname: test\r\n# not a section\r\n---\r\n# Main\r\ntext\r\n## Child\r\n~~~~python\r\n# code comment\r\n~~~\r\n~~~~\r\n## Next\r\n';
  const parsed = parseMarkdownReview(content);
  assert.equal(parsed.available, true);
  assert.deepEqual(parsed.headings.map(h => [h.title, h.line, h.end]), [['Main',5,12],['Child',7,11],['Next',12,12]]);
  assert.deepEqual(parsed.blocks.map(b => [b.kind,b.start,b.end,b.closed]), [['frontmatter',1,4,true],['code',8,11,true]]);
  assert.equal(parsed.lines[8].kind, 'code'); assert.equal(parsed.lines[9].kind, 'code'); assert.equal(content.includes('\r\n'), true);
});

test('unclosed blocks are grouped with notes, never inferred headings', () => {
  for (const content of ['```js\n# hidden\n', '---\n# hidden\n']) {
    const parsed = parseMarkdownReview(content); assert.equal(parsed.headings.length,0); assert.equal(parsed.blocks[0].closed,false); assert.equal(parsed.notes.length,1);
    assert.match(renderMarkdownReview(content),/unclosed/);
  }
  assert.equal(parseMarkdownReview('````js\n```\n# hidden\n````\n').headings.length,0);
  assert.equal(parseMarkdownReview('    ```\n# visible\n').headings.length,1);
});

test('source HTML, URLs, labels and unknown language remain inert escaped text', () => {
  const html = renderMarkdownReview('# <img src=x>\n[link](https://example.invalid)\n```<script>\n<script>run()</script>\n```\n');
  assert.doesNotMatch(html, /<img|<script|<a |<iframe|<svg|src=\"/);
  assert.match(html,/&lt;script&gt;run\(\)&lt;\/script&gt;/); assert.match(html,/data-md-jump="1"/);
  assert.match(html,/Code · &lt;script&gt;/);
  assert.equal(renderHighlightedLine({kind:'code',language:'unknown',text:'return "<img>"'}),'return &quot;&lt;img&gt;&quot;');
});

test('known code gets bounded hints and Markdown gets inline highlighting', () => {
  assert.match(renderHighlightedLine({kind:'code',language:'python',text:'return "safe" # note'}),/md-token--keyword/);
  assert.match(renderHighlightedLine({kind:'code',language:'json',text:'{"count": 3, "allow": false}'}),/md-token--number/);
  assert.match(renderHighlightedLine({kind:'text',text:'Use `local` and **review**.'}),/md-token--inline/);
  const long = '<'.repeat(3000); assert.equal(renderHighlightedLine({kind:'text',text:long}), '&lt;'.repeat(3000));
});

test('limits refuse whole review without truncating or manufacturing sections', () => {
  for (const content of ['x'.repeat(262145), '😀'.repeat(65537), '\n'.repeat(10001), 'x'.repeat(16385), '# h\n'.repeat(257)]) assert.equal(parseMarkdownReview(content).available, false);
  assert.match(renderMarkdownReview('x'.repeat(262145)), /Original source remains unchanged/);
  assert.match(renderMarkdownReview(''), /source is empty/);
  assert.equal(parseMarkdownReview('##\n').headings[0].title, 'Untitled section');
});

test('line jumps open every enclosing group and refuse absent or invalid targets', () => {
  const outer={tagName:'DETAILS',open:false,parentElement:null};const inner={tagName:'DETAILS',open:false,parentElement:outer};let focused=false,scrolled=false;
  const target={parentElement:inner,focus(){focused=true;},scrollIntoView(){scrolled=true;}};
  const root={querySelector: selector => selector==='[data-md-line="8"]'?target:null};outer.parentElement=root;
  assert.equal(jumpToMarkdownLine(root,8),true);assert.equal(outer.open,true);assert.equal(inner.open,true);assert.equal(focused&&scrolled,true);
  for(const value of [0,NaN,Infinity,10001,'8',3])assert.equal(jumpToMarkdownLine(root,value),false);
});

test('matching rerender captures folds, scroll and keyboard focus without retaining source text', () => {
  const fold={open:false,dataset:{mdFold:'block-8'}};const outline={open:false};const source={scrollTop:42,scrollLeft:5};let focused=false;
  const active={matches: selector=>selector==='summary',parentElement:{dataset:{mdFold:'block-8'}}};
  const summary={focus(){focused=true;}};
  const review={contains: node=>node===active,querySelectorAll:()=>[fold],querySelector: selector=>selector==='.md-outline'?outline:selector==='.md-source'?source:selector==='[data-md-fold="block-8"] > summary'?summary:null};
  const root={ownerDocument:{activeElement:active},querySelector:()=>review};
  const saved=captureMarkdownReview(root);assert.deepEqual(saved.focus,{kind:'fold',key:'block-8'});assert.doesNotMatch(JSON.stringify(saved),/content|source text/);
  fold.open=true;outline.open=true;source.scrollTop=0;restoreMarkdownReview(root,saved);assert.equal(fold.open,false);assert.equal(outline.open,false);assert.equal(source.scrollTop,42);assert.equal(focused,true);
});

test('nested fold markup contains every original display line once and only numeric targets', () => {
  const html=renderMarkdownReview('# Parent\n## Child\n```json\n{"x":true}\n```\n# Sibling\n');
  assert.equal((html.match(/data-md-line=/g)||[]).length,6);assert.equal((html.match(/class="md-section"/g)||[]).length,3);
  assert.match(html,/Lines 1–5/);assert.match(html,/Lines 2–5/);assert.match(html,/data-md-fold="block-3"/);
});


test('syntax-hint fallback is explicit in review and Import without losing line content',()=>{
 const dense='['.repeat(3000);
 for(const html of [renderMarkdownReview(dense),renderImportMarkdownLines(dense,null)]){assert.match(html,/1 line uses plain text because syntax hint limits were reached/);assert.ok(html.includes(dense));}
 const tooLarge='x'.repeat(262145);assert.match(renderImportMarkdownLines(tooLarge,null),/Syntax view unavailable.*256 KiB/);assert.doesNotMatch(renderImportMarkdownLines(tooLarge,null),/class="analysis-line/);
 const selected=renderImportMarkdownLines('# Root\ntext\n',{startLine:2,endLine:2});assert.match(selected,/analysis-line--selected"><b>2/);assert.match(selected,/md-token--heading/);
});


test('token caps fall back explicitly below the line-length cap for code and Markdown',()=>{
 const code='1 '.repeat(513), markdown='`a`'.repeat(513);assert.ok(code.length<2048&&markdown.length<2048);
 for(const [text,kind,language] of [[code,'code','json'],[markdown,'text','']]){const highlighted=renderHighlightedLine({text,kind,language});assert.doesNotMatch(highlighted,/md-token/);assert.equal(highlighted,text);}
 for(const content of ['```json\n'+code+'\n```',markdown]){const html=renderMarkdownReview(content);assert.match(html,/syntax hint limits were reached/);assert.ok(html.includes(content.startsWith('```')?code:markdown));assert.match(renderImportMarkdownLines(content,null),/syntax hint limits were reached/);}
});

test('outline starts secondary without removing section navigation or original source lines',()=>{
 const html=renderMarkdownReview('# Heading\n```sh\necho local\n```\n');
 assert.match(html,/<details class="md-outline"><summary>Outline/);assert.match(html,/data-md-jump="1"/);
 assert.equal((html.match(/data-md-line=/g)||[]).length,4);assert.match(html,/<details open class="md-code-block"/);
});

test('find is a secondary labeled disclosure with keyboard-ready bounded literal controls',()=>{
 const html=renderMarkdownReview('# Source\nText\n');
 assert.match(html,/data-md-find-toggle aria-expanded="false"/);assert.match(html,/data-md-find-panel hidden role="search"/);
 assert.match(html,/data-md-find-query maxlength="256" autocomplete="off" spellcheck="false"/);
 assert.match(html,/aria-label="Previous match" disabled/);assert.match(html,/aria-label="Next match" disabled/);assert.match(html,/data-md-find-case/);assert.match(html,/aria-label="Close find"/);
 assert.equal((html.match(/data-md-line=/g)||[]).length,2);
});
