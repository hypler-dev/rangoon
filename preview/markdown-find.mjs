import { parseMarkdownReview } from './markdown-review.mjs';

const MATCH_LIMIT = 1000;
const validQuery = value => typeof value === 'string' && value.length <= 256 && !/[\r\n\0]/.test(value);

// UI offsets only. These positions never become native byte ranges or authority.
function foldedLine(value, sensitive) {
  let text = '', offset = 0;
  const starts = [], ends = [];
  for (const point of value) {
    const folded = sensitive ? point : point.toLowerCase();
    text += folded;
    for (let i = 0; i < folded.length; i++) { starts.push(offset); ends.push(offset + point.length); }
    offset += point.length;
  }
  return { text, starts, ends };
}
function searchLines(lines, query, sensitive) {
  const needle = sensitive ? query : query.toLowerCase();
  const matches = [];
  for (const line of lines) {
    const folded = foldedLine(line.text, sensitive);
    let from = 0;
    while (from <= folded.text.length - needle.length) {
      const found = folded.text.indexOf(needle, from); if (found < 0) break;
      const last = found + needle.length - 1;
      const wholeStart = found === 0 || folded.starts[found] !== folded.starts[found - 1];
      const wholeEnd = last + 1 === folded.text.length || folded.ends[last] !== folded.ends[last + 1];
      if (wholeStart && wholeEnd) {
        if (matches.length === MATCH_LIMIT) return { matches, limited: true };
        matches.push({ line: line.number, start: folded.starts[found], end: folded.ends[last] });
        from = found + needle.length;
      } else from = found + 1;
    }
  }
  return { matches, limited: false };
}

export function createMarkdownFindModel(source) {
  let parsed = parseMarkdownReview(source); source = null;
  let disposed = false;
  let state = { open: false, query: '', caseSensitive: false, matches: [], index: -1, limited: false, status: parsed.available ? 'empty' : 'unavailable', reason: parsed.reason ?? '' };
  function recompute() {
    state = { ...state, matches: [], index: -1, limited: false, reason: '' };
    if (!parsed?.available) { state.status = 'unavailable'; state.reason = parsed?.reason ?? 'Search is closed.'; return; }
    if (!validQuery(state.query)) { state.status = 'invalid'; state.reason = 'Use one line of text, up to 256 characters, without NUL.'; return; }
    if (!state.query) { state.status = 'empty'; return; }
    const found = searchLines(parsed.lines, state.query, state.caseSensitive);
    state = { ...state, ...found, index: found.matches.length ? 0 : -1, status: found.matches.length ? 'ready' : 'no_matches' };
  }
  return {
    getState: () => ({ ...state, matches: state.matches.map(match => ({ ...match })) }),
    setOpen(open) { if (disposed) return; state.open = Boolean(open); },
    setQuery(query) {
      if (disposed) return false;
      if (typeof query !== 'string' || query.length > 256) {
        state = { ...state, query: '', matches: [], index: -1, limited: false, status: 'invalid', reason: 'Use at most 256 characters. The query was not searched.' }; return false;
      }
      state.query = query; recompute(); return state.status !== 'invalid';
    },
    setCase(sensitive) { if (!disposed && typeof sensitive === 'boolean') { state.caseSensitive = sensitive; recompute(); } },
    select(index) { if (!disposed && Number.isSafeInteger(index) && index >= 0 && index < state.matches.length) state.index = index; },
    move(direction) { if (!disposed && state.matches.length && (direction === 1 || direction === -1)) state.index = (state.index + direction + state.matches.length) % state.matches.length; },
    close() { if (disposed) return; state = { ...state, open: false, query: '', matches: [], index: -1, limited: false, status: parsed.available ? 'empty' : 'unavailable', reason: parsed.reason ?? '' }; },
    dispose() { disposed = true; parsed = null; state = { open: false, query: '', caseSensitive: false, matches: [], index: -1, limited: false, status: 'unavailable', reason: 'Search is closed.' }; },
  };
}

export function markdownFindStatus(state) {
  if (state.status === 'invalid' || state.status === 'unavailable') return state.reason;
  if (state.status === 'empty') return 'Type to search this file.';
  if (state.status === 'no_matches') return 'No matches in this file.';
  const active = state.matches[state.index];
  return `${state.index + 1} of ${state.matches.length}${state.limited ? '+' : ''} · line ${active.line}${state.limited ? ' · first 1000 matches shown' : ''}`;
}

function clearHighlights(review) {
  const source = review.querySelector('.md-source'); if (!source) return;
  for (const mark of source.querySelectorAll('mark[data-md-find-hit]')) mark.replaceWith(review.ownerDocument.createTextNode(mark.textContent));
  source.normalize();
}
function paintHighlights(review, matches) {
  clearHighlights(review);
  const grouped = new Map();
  matches.forEach((match, index) => { if (!grouped.has(match.line)) grouped.set(match.line, []); grouped.get(match.line).push({ ...match, index }); });
  for (const [number, hits] of grouped) {
    const code = review.querySelector(`[data-md-line="${number}"] code`); if (!code) continue;
    const doc = review.ownerDocument;
    const walker = doc.createTreeWalker(code, 4); // NodeFilter.SHOW_TEXT
    const nodes = []; while (walker.nextNode()) nodes.push(walker.currentNode);
    let offset = 0;
    for (const node of nodes) {
      const value = node.data, end = offset + value.length;
      const slices = hits.filter(hit => hit.start < end && hit.end > offset);
      if (slices.length) {
        const fragment = doc.createDocumentFragment(); let cursor = 0;
        for (const hit of slices) {
          const start = Math.max(0, hit.start - offset), stop = Math.min(value.length, hit.end - offset);
          if (start > cursor) fragment.append(doc.createTextNode(value.slice(cursor, start)));
          const mark = doc.createElement('mark'); mark.dataset.mdFindHit = String(hit.index); mark.textContent = value.slice(start, stop); fragment.append(mark); cursor = stop;
        }
        if (cursor < value.length) fragment.append(doc.createTextNode(value.slice(cursor)));
        node.replaceWith(fragment);
      }
      offset = end;
    }
  }
}

export function bindMarkdownFind(root, content, initial = null) {
  const review = root.querySelector('.md-review'); if (!review) return null;
  const model = createMarkdownFindModel(content); content = null;
  const query = review.querySelector('[data-md-find-query]');
  const panel = review.querySelector('[data-md-find-panel]');
  const toggle = review.querySelector('[data-md-find-toggle]');
  const caseControl = review.querySelector('[data-md-find-case]');
  let disposed = false;
  function chrome({ reveal = false } = {}) {
    const state = model.getState();
    panel.hidden = !state.open; toggle.setAttribute('aria-expanded', String(state.open)); caseControl.checked = state.caseSensitive;
    const status = review.querySelector('[data-md-find-status]'), message = markdownFindStatus(state);
    if (status.textContent !== message) status.textContent = message;
    for (const button of review.querySelectorAll('[data-md-find-move]')) button.disabled = state.index < 0;
    const marks = review.querySelectorAll('mark[data-md-find-hit]');
    let active = null;
    for (const mark of marks) {
      const selected = Number(mark.dataset.mdFindHit) === state.index;
      mark.classList.toggle('md-find-hit--active', selected); mark.setAttribute('aria-current', String(selected));
      if (selected && !active) active = mark;
    }
    if (reveal && active) {
      for (let parent = active.parentElement; parent && parent !== review; parent = parent.parentElement) if (parent.tagName === 'DETAILS') parent.open = true;
      const source = review.querySelector('.md-source'), rect = active.getBoundingClientRect(), bounds = source.getBoundingClientRect();
      source.scrollTop += rect.top - bounds.top - Math.max(0, (source.clientHeight - rect.height) / 2);
    }
  }
  function search() { const state = model.getState(); paintHighlights(review, state.matches); chrome({ reveal: true }); }
  function open() { model.setOpen(true); chrome(); query.focus({ preventScroll: true }); }
  function close() { model.close(); query.value = ''; clearHighlights(review); chrome(); toggle.focus({ preventScroll: true }); }
  function move(direction) { model.move(direction); chrome({ reveal: true }); }
  const onClick = event => {
    if (disposed) return;
    if (event.target.closest('[data-md-find-toggle]')) { event.preventDefault(); event.stopPropagation(); model.getState().open ? close() : open(); }
    else if (event.target.closest('[data-md-find-close]')) { event.preventDefault(); event.stopPropagation(); close(); }
    else { const control = event.target.closest('[data-md-find-move]'); if (control) { event.preventDefault(); event.stopPropagation(); move(Number(control.dataset.mdFindMove)); } }
  };
  const onInput = event => { if (!disposed && event.target === query) { event.stopPropagation(); model.setQuery(query.value); const state = model.getState(); if (query.value !== state.query) query.value = state.query; search(); } };
  const onChange = event => { if (!disposed && event.target === caseControl) { event.stopPropagation(); model.setCase(caseControl.checked); search(); } };
  const onKey = event => {
    if (disposed) return;
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'f') { event.preventDefault(); event.stopPropagation(); open(); return; }
    if (event.key === 'F3') { event.preventDefault(); event.stopPropagation(); model.getState().open ? move(event.shiftKey ? -1 : 1) : open(); return; }
    if (event.key === 'Escape' && model.getState().open) { event.preventDefault(); event.stopPropagation(); close(); return; }
    if (event.target === query && event.key === 'Enter') { event.preventDefault(); event.stopPropagation(); move(event.shiftKey ? -1 : 1); }
  };
  review.addEventListener('click', onClick); review.addEventListener('input', onInput); review.addEventListener('change', onChange); review.addEventListener('keydown', onKey);
  if (initial) {
    model.setCase(initial.caseSensitive === true); model.setQuery(initial.query ?? ''); model.select(initial.index); model.setOpen(initial.open === true);
    query.value = model.getState().query; paintHighlights(review, model.getState().matches);
  }
  chrome();
  if (initial?.focus && (model.getState().open || initial.focus.kind === 'toggle')) {
    if (initial.focus.kind === 'query') { query.focus({ preventScroll: true }); if (initial.focus.range) query.setSelectionRange(...initial.focus.range); }
    else {
      const selectors = { case: '[data-md-find-case]', next: '[data-md-find-move="1"]', previous: '[data-md-find-move="-1"]', toggle: '[data-md-find-toggle]', close: '[data-md-find-close]' };
      if (selectors[initial.focus.kind]) review.querySelector(selectors[initial.focus.kind])?.focus({ preventScroll: true });
    }
  }
  function dispose() { if (disposed) return; disposed = true; review.removeEventListener('click', onClick); review.removeEventListener('input', onInput); review.removeEventListener('change', onChange); review.removeEventListener('keydown', onKey); clearHighlights(review); query.value = ''; panel.hidden = true; toggle.setAttribute('aria-expanded', 'false'); model.dispose(); }
  return {
    capture() {
      const state = model.getState(), active = root.ownerDocument.activeElement;
      return { open: state.open, query: state.query, caseSensitive: state.caseSensitive, index: state.index,
        focus: active === query ? { kind: 'query', range: [query.selectionStart, query.selectionEnd] } : active === caseControl ? { kind: 'case' } : active === toggle ? { kind: 'toggle' } : active?.matches('[data-md-find-close]') ? { kind: 'close' } : active?.matches('[data-md-find-move]') ? { kind: active.dataset.mdFindMove === '1' ? 'next' : 'previous' } : null };
    },
    dispose, purgeForWorkspaceLock: dispose,
  };
}
