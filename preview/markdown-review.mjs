import { escapeText } from './analysis-model.mjs';

const escape = escapeText;
const limits = { bytes: 262144, lines: 10000, groups: 256, lineLength: 16384, tokens: 512 };
const known = new Set(['json', 'jsonc', 'yaml', 'yml', 'sh', 'bash', 'shell', 'python', 'py', 'javascript', 'js', 'typescript', 'ts']);

// Presentation-only lexical structure. Native source spans remain authoritative.
export function parseMarkdownReview(content) {
  if (typeof content !== 'string' || content.length > limits.bytes || new TextEncoder().encode(content).length > limits.bytes) return { available: false, reason: 'Source exceeds the 256 KiB review limit.' };
  const raw = content.split('\n'); if (raw.at(-1) === '') raw.pop();
  if (raw.length > limits.lines || raw.some(line => line.length > limits.lineLength)) return { available: false, reason: 'Source exceeds the line review limit.' };
  const lines = raw.map((value, index) => ({ number: index + 1, text: value.endsWith('\r') ? value.slice(0, -1) : value, kind: 'text', language: '' }));
  const headings = [], blocks = [], notes = [];
  let fence = null, frontmatter = null;
  for (const line of lines) {
    const value = line.number === 1 ? line.text.replace(/^\uFEFF/, '') : line.text;
    if (line.number === 1 && value === '---') { frontmatter = { start: 1, end: lines.length, kind: 'frontmatter', language: 'yaml', closed: false }; blocks.push(frontmatter); line.kind = 'fence'; continue; }
    if (frontmatter) {
      line.kind = 'code'; line.language = 'yaml';
      if (/^(---|\.\.\.)\s*$/.test(value)) { frontmatter.end = line.number; frontmatter.closed = true; line.kind = 'fence'; frontmatter = null; }
      continue;
    }
    if (fence) {
      line.kind = 'code'; line.language = fence.language;
      const close = /^ {0,3}(`{3,}|~{3,})[\t ]*$/.exec(value);
      if (close && close[1][0] === fence.marker && close[1].length >= fence.length) { line.kind = 'fence'; fence.end = line.number; fence.closed = true; fence = null; }
      continue;
    }
    const open = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(value);
    if (open && !(open[1][0] === '`' && open[2].includes('`'))) {
      fence = { start: line.number, end: lines.length, kind: 'code', language: open[2].trim(), marker: open[1][0], length: open[1].length, closed: false };
      blocks.push(fence); line.kind = 'fence'; continue;
    }
    const heading = /^ {0,3}(#{1,6})(?:[\t ]+(.+?)|[\t ]*)$/.exec(value);
    if (heading) { line.kind = 'heading'; headings.push({ line: line.number, level: heading[1].length, title: (heading[2] ?? '').replace(/[\t ]+#+[\t ]*$/, '') || 'Untitled section', end: lines.length }); }
    else if (/^\s*(?:[-*+] |\d+[.)] )/.test(value)) line.kind = 'list';
    else if (/^\s*>/.test(value)) line.kind = 'quote';
  }
  if (headings.length + blocks.length > limits.groups) return { available: false, reason: 'Source exceeds the 256-group review limit.' };
  for (const block of blocks) if (!block.closed) notes.push(`${block.kind === 'frontmatter' ? 'Frontmatter' : 'Code fence'} at line ${block.start} is not closed.`);
  const stack = [];
  for (const heading of headings) {
    while (stack.length && stack.at(-1).level >= heading.level) stack.pop().end = heading.line - 1;
    stack.push(heading);
  }
  return { available: true, lines, headings, blocks, notes };
}

function token(kind, value) { return `<span class="md-token md-token--${kind}">${escape(value)}</span>`; }
export function highlightReviewLine(line) {
  const value = line.text ?? '';
  if (value.length > 2048) return { html: escape(value), limited: true };
  if (line.kind === 'fence') return { html: token('fence', value), limited: false };
  if (line.kind === 'heading') return { html: token('heading', value), limited: false };
  if (line.kind === 'code') {
    const language = line.language?.split(/\s/, 1)[0]?.toLowerCase();
    if (!known.has(language)) return { html: escape(value), limited: false };
    // Bounded single-line hints, not language validation or executable HTML.
    const pattern = /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|#[^\r\n]*|\/\/[^\r\n]*|\b(?:true|false|null|None|True|False|const|let|var|function|return|if|else|for|while|import|from|def|class|async|await|export|echo|exit|then|fi)\b|\b\d+(?:\.\d+)?\b/g;
    let output = '', offset = 0, count = 0;
    for (const match of value.matchAll(pattern)) {
      if (++count > limits.tokens) return { html: escape(value), limited: true };
      output += escape(value.slice(offset, match.index));
      output += token(/^['"]/.test(match[0]) ? 'string' : /^(#|\/\/)/.test(match[0]) ? 'comment' : /^\d/.test(match[0]) ? 'number' : 'keyword', match[0]);
      offset = match.index + match[0].length;
    }
    return { html: output + escape(value.slice(offset)), limited: false };
  }
  let output = '', offset = 0, count = 0;
  for (const match of value.matchAll(/`[^`\r\n]+`|\*\*[^*\r\n]+\*\*|\[[^\]\r\n]+\]\([^\)\r\n]*\)|(?:^|\s)[-*+] (?=\S)|(?:^|\s)\[[ xX]\]/g)) {
    if (++count > limits.tokens) return { html: escape(value), limited: true };
    output += escape(value.slice(offset, match.index)); output += token(match[0].startsWith('`') ? 'inline' : 'markup', match[0]); offset = match.index + match[0].length;
  }
  return { html: output + escape(value.slice(offset)), limited: false };
}

export function renderHighlightedLine(line) { return highlightReviewLine(line).html; }

export function renderImportMarkdownLines(content, activeSpan) {
  const parsed = parseMarkdownReview(content);
  if (!parsed.available) return `<p class="analysis-note">Syntax view unavailable: ${escape(parsed.reason)} Original source remains unchanged.</p>`;
  if (!parsed.lines.length) return '<p class="analysis-note">The returned source is empty.</p>';
  const hints = parsed.lines.map(highlightReviewLine);
  const count = hints.filter(item => item.limited).length;
  return `${count ? `<p class="analysis-note">${count} line${count === 1 ? '' : 's'} ${count === 1 ? 'uses' : 'use'} plain text because syntax hint limits were reached.</p>` : ''}${parsed.lines.map((line, index) => {
    const selected = activeSpan && line.number >= activeSpan.startLine && line.number <= activeSpan.endLine;
    return `<span class="analysis-line${selected ? ' analysis-line--selected' : ''}"><b>${line.number}</b><code>${hints[index].html || ' '}</code></span>`;
  }).join('')}`;
}

export function renderMarkdownReview(content) {
  const parsed = parseMarkdownReview(content);
  if (!parsed.available) return `<p class="md-review-unavailable">${escape(parsed.reason)} Original source remains unchanged.</p>`;
  if (!parsed.lines.length) return '<p class="md-review-unavailable">This source is empty.</p>';
  const hints = parsed.lines.map(highlightReviewLine);
  const limited = hints.filter(item => item.limited).length;
  if (limited) parsed.notes.push(`${limited} line${limited === 1 ? '' : 's'} ${limited === 1 ? 'uses' : 'use'} plain text because syntax hint limits were reached.`);
  const headingAt = new Map(parsed.headings.map(h => [h.line, h]));
  const blockAt = new Map(parsed.blocks.map(b => [b.start, b]));
  const lineMarkup = line => `<div class="md-line" data-md-line="${line.number}" tabindex="-1"><span class="md-line-number" aria-hidden="true">${line.number}</span><code>${hints[line.number - 1].html || ' '}</code></div>`;
  function range(start, end, skipHeading = false) {
    let output = '';
    for (let number = start; number <= end; number++) {
      const heading = headingAt.get(number), block = blockAt.get(number);
      if (heading && !(skipHeading && number === start)) {
        output += `<details open class="md-section" data-md-fold="section-${number}"><summary>${escape(heading.title)}<small>Lines ${number}–${heading.end}</small></summary>${range(number, heading.end, true)}</details>`; number = heading.end;
      } else if (block) {
        output += `<details open class="md-code-block" data-md-fold="block-${number}"><summary>${block.kind === 'frontmatter' ? 'Frontmatter · YAML' : `Code · ${escape(block.language || 'plain text')}`}<small>Lines ${number}–${block.end}${block.closed ? '' : ' · unclosed'}</small></summary>${parsed.lines.slice(number - 1, block.end).map(lineMarkup).join('')}</details>`; number = block.end;
      } else output += lineMarkup(parsed.lines[number - 1]);
    }
    return output;
  }
  return `<section class="md-review" aria-label="Markdown source review"><div class="md-review-help"><span>Read-only · source unchanged</span><button type="button" data-md-find-toggle aria-expanded="false">Find in file</button></div><section class="md-find-panel" data-md-find-panel hidden role="search" aria-label="Find in open file"><div class="md-find-controls"><label><span class="md-find-label">Find in file</span><input type="search" data-md-find-query maxlength="256" autocomplete="off" spellcheck="false" placeholder="Find text…"></label><button type="button" data-md-find-move="-1" aria-label="Previous match" disabled>Previous</button><button type="button" data-md-find-move="1" aria-label="Next match" disabled>Next</button><label class="md-find-case"><input type="checkbox" data-md-find-case>Match case</label><button type="button" data-md-find-close aria-label="Close find">Close</button></div><div class="md-find-meta"><span data-md-find-status role="status" aria-live="polite" aria-atomic="true">Type to search this file.</span><small>Enter: next · Shift+Enter: previous</small></div></section>${parsed.notes.length ? `<p class="md-review-warning" role="note">${parsed.notes.map(escape).join(' ')}</p>` : ''}<details class="md-outline"><summary>Outline <small>${parsed.headings.length} sections · ${parsed.blocks.length} blocks</small></summary><nav aria-label="Markdown outline">${parsed.headings.length ? parsed.headings.map(h => `<button type="button" data-md-jump="${h.line}" style="--md-level:${h.level}">${escape(h.title)}<small>Line ${h.line}</small></button>`).join('') : '<p>No ATX headings outside code or frontmatter.</p>'}</nav></details><div class="md-source" tabindex="0" aria-label="Read-only highlighted source">${range(1, parsed.lines.length)}</div></section>`;
}

export function jumpToMarkdownLine(root, number) {
  if (!Number.isSafeInteger(number) || number < 1 || number > limits.lines) return false;
  const target = root.querySelector(`[data-md-line="${number}"]`); if (!target) return false;
  for (let parent = target.parentElement; parent && parent !== root; parent = parent.parentElement) if (parent.tagName === 'DETAILS') parent.open = true;
  target.focus({ preventScroll: true }); target.scrollIntoView({ block: 'center', behavior: 'instant' }); return true;
}
export function captureMarkdownReview(root) {
  const review = root.querySelector('.md-review'); if (!review) return null;
  const active = root.ownerDocument?.activeElement;
  const focus = !review.contains(active) ? null : active?.matches('[data-md-line]') ? { kind: 'line', key: active.dataset.mdLine }
    : active?.matches('[data-md-jump]') ? { kind: 'jump', key: active.dataset.mdJump }
      : active?.matches('.md-source') ? { kind: 'source' }
        : active?.matches('.md-outline > summary') ? { kind: 'outline' }
          : active?.matches('summary') && active.parentElement.dataset.mdFold ? { kind: 'fold', key: active.parentElement.dataset.mdFold } : null;
  return { folds: [...review.querySelectorAll('[data-md-fold]')].filter(node => !node.open).map(node => node.dataset.mdFold), outline: review.querySelector('.md-outline')?.open,
    top: review.querySelector('.md-source')?.scrollTop ?? 0, left: review.querySelector('.md-source')?.scrollLeft ?? 0, focus };
}
export function restoreMarkdownReview(root, state) {
  if (!state) return;
  const review = root.querySelector('.md-review'); if (!review) return;
  const folds = new Set(state.folds);
  for (const node of review.querySelectorAll('[data-md-fold]')) node.open = !folds.has(node.dataset.mdFold);
  const outline = review.querySelector('.md-outline'); if (outline) outline.open = state.outline;
  const source = review.querySelector('.md-source'); if (source) { source.scrollTop = state.top; source.scrollLeft = state.left; }
  const focus = state.focus;
  const selector = focus?.kind === 'line' ? `[data-md-line="${Number(focus.key)}"]` : focus?.kind === 'jump' ? `[data-md-jump="${Number(focus.key)}"]`
    : focus?.kind === 'source' ? '.md-source' : focus?.kind === 'outline' ? '.md-outline > summary'
      : focus?.kind === 'fold' && /^(section|block)-\d+$/.test(focus.key) ? `[data-md-fold="${focus.key}"] > summary` : null;
  if (selector) review.querySelector(selector)?.focus({ preventScroll: true });
}
