import test from 'node:test';
import assert from 'node:assert/strict';
import { bindMarkdownFind, createMarkdownFindModel, markdownFindStatus } from '../preview/markdown-find.mjs';

const query = (source, value, sensitive = false) => { const model = createMarkdownFindModel(source); model.setCase(sensitive); model.setQuery(value); return model; };

test('literal find distinguishes empty, no matches and case matching with original line offsets', () => {
  const model = createMarkdownFindModel('# Review\r\nreview review\r\n');
  assert.equal(model.getState().status, 'empty'); model.setOpen(true); model.setQuery('Review');
  assert.deepEqual(model.getState().matches, [{line:1,start:2,end:8},{line:2,start:0,end:6},{line:2,start:7,end:13}]);
  model.setCase(true); assert.equal(model.getState().matches.length,1);
  model.setQuery('missing'); assert.equal(model.getState().status,'no_matches'); assert.match(markdownFindStatus(model.getState()),/No matches/);
  model.setQuery(''); assert.equal(model.getState().status,'empty'); assert.match(markdownFindStatus(model.getState()),/Type to search/);
});

test('metacharacters and source HTML are literal text, not regular expressions or markup',()=>{
  const source='<img onerror=run()> .* [a] `text`';
  for(const value of ['<img','.*','[a]','`text`']){const state=query(source,value).getState();assert.equal(state.matches.length,1);assert.equal(source.slice(state.matches[0].start,state.matches[0].end),value);}
  assert.equal(query('anything','.*').getState().status,'no_matches');
});

test('Unicode lowercase expansion and astral points map to whole original UTF-16 boundaries',()=>{
  assert.deepEqual(query('İi','i').getState().matches,[{line:1,start:1,end:2}]);
  assert.deepEqual(query('İi','İ').getState().matches,[{line:1,start:0,end:1}]);
  assert.deepEqual(query('a😀😀','😀').getState().matches,[{line:1,start:1,end:3},{line:1,start:3,end:5}]);
  assert.equal(query('😀','\ud83d').getState().status,'no_matches');assert.equal(query('😀','\ude00').getState().status,'no_matches');
  assert.equal(query('café CAFÉ','café').getState().matches.length,2);
  assert.equal(query('café','cafe\u0301',true).getState().status,'no_matches');
});

test('matches are non-overlapping and capped honestly without inventing a total',()=>{
  assert.deepEqual(query('aaaa','aa').getState().matches,[{line:1,start:0,end:2},{line:1,start:2,end:4}]);
  const capped=query('x '.repeat(1001),'x').getState();assert.equal(capped.matches.length,1000);assert.equal(capped.limited,true);assert.match(markdownFindStatus(capped),/1 of 1000\+/);assert.match(markdownFindStatus(capped),/first 1000 matches shown/);
  assert.equal(query('x '.repeat(1000),'x').getState().limited,false);
});

test('previous and next wrap safely, invalid selections and directions do not alter position',()=>{
  const model=query('one one one','one');model.move(-1);assert.equal(model.getState().index,2);model.move(1);assert.equal(model.getState().index,0);
  model.select(1);for(const direction of [0,3,NaN])model.move(direction);for(const index of [-1,100,NaN])model.select(index);assert.equal(model.getState().index,1);
  const none=query('nothing','one');none.move(1);assert.equal(none.getState().index,-1);
});

test('invalid and unavailable inputs produce bounded explicit states',()=>{
  const model=query('text','text');for(const value of ['a\nb','a\rb','a\0b']){assert.equal(model.setQuery(value),false);assert.equal(model.getState().status,'invalid');assert.deepEqual(model.getState().matches,[]);}
  assert.equal(model.setQuery('x'.repeat(257)),false);assert.equal(model.getState().query,'');assert.match(markdownFindStatus(model.getState()),/256/);
  const unavailable=query('x'.repeat(262145),'x');assert.equal(unavailable.getState().status,'unavailable');assert.match(markdownFindStatus(unavailable.getState()),/256 KiB/);
});

test('close and dispose remove query/matches and late operations cannot revive state',()=>{
  const model=query('source source','source');model.setOpen(true);model.close();assert.equal(model.getState().open,false);assert.equal(model.getState().query,'');assert.equal(model.getState().matches.length,0);
  model.setQuery('source');model.dispose();model.setOpen(true);model.setQuery('source');model.setCase(true);model.move(1);model.select(1);model.close();
  const state=model.getState();assert.equal(state.open,false);assert.equal(state.query,'');assert.deepEqual(state.matches,[]);assert.equal(state.status,'unavailable');
});

test('public result copies cannot alter model offsets or active position',()=>{
  const model=query('text text','text');const state=model.getState();state.matches[0].start=99;state.matches.length=0;state.index=99;assert.equal(model.getState().matches[0].start,0);assert.equal(model.getState().matches.length,2);assert.equal(model.getState().index,0);
});


test('same-file binding restores Find trigger and Close focus without retaining source', () => {
  for (const [kind, open] of [['toggle', false], ['toggle', true], ['close', true]]) {
    let focused = null;
    const control = name => ({ value: '', checked: false, hidden: false, textContent: '', setAttribute() {}, focus() { focused = name; }, matches(selector) { return selector === '[data-md-find-close]' && name === 'close'; } });
    const controls = { '[data-md-find-query]': control('query'), '[data-md-find-panel]': control('panel'), '[data-md-find-toggle]': control('toggle'), '[data-md-find-case]': control('case'), '[data-md-find-close]': control('close'), '[data-md-find-status]': control('status') };
    const source = { querySelectorAll: () => [], normalize() {} };
    const review = { querySelector: selector => selector === '.md-source' ? source : controls[selector], querySelectorAll: () => [], addEventListener() {}, removeEventListener() {} };
    const root = { querySelector: () => review, ownerDocument: { activeElement: controls[kind === 'toggle' ? '[data-md-find-toggle]' : '[data-md-find-close]'] } };
    const binding = bindMarkdownFind(root, '# Original', { open, query: '', caseSensitive: false, index: -1, focus: { kind } });
    assert.equal(focused, kind);
    assert.equal(binding.capture().focus.kind, kind);
    binding.dispose();
    assert.equal(controls['[data-md-find-query]'].value, '');
  }
});
