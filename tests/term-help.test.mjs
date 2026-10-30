import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { bindTermHelp, renderTermHelp } from '../preview/term-help.mjs';
import { createSkillsController } from '../preview/skills-model.mjs';
import { renderSkillsView } from '../preview/skills-view.mjs';

function eventTarget(group = null) {
  return { closest: selector => {
    if (selector === '[data-term-help-group]') return group;
    if (selector === 'details[data-term-help-group]' || selector === 'details[data-term-help-group][open]') return group?.open ? group : null;
    return null;
  } };
}

function fixture() {
  const rootListeners = new Map();
  const documentListeners = new Map();
  const document = {
    addEventListener(type, listener) { documentListeners.set(type, listener); },
    removeEventListener(type) { documentListeners.delete(type); },
  };
  const makeGroup = () => {
    const summary = { focused: false, focus() { this.focused = true; } };
    const group = { open: false, querySelector: selector => selector === 'summary' ? summary : null, summary };
    group.closest = selector => selector === '[data-term-help-group]' || selector === 'details[data-term-help-group]' || selector === 'details[data-term-help-group][open]' ? group : null;
    return group;
  };
  const first = makeGroup();
  const second = makeGroup();
  const other = makeGroup();
  const root = {
    ownerDocument: document,
    contains: node => [first, second, other].includes(node),
    querySelectorAll: selector => selector === 'details[data-term-help-group][open]' ? [first, second, other].filter(group => group.open) : [],
    addEventListener(type, listener) { rootListeners.set(type, listener); },
    removeEventListener(type) { rootListeners.delete(type); },
  };
  return {
    root, first, second, other,
    fireRoot(type, event) { rootListeners.get(type)?.(event); },
    fireDocument(type, event) { documentListeners.get(type)?.(event); },
    listenerCount: () => rootListeners.size + documentListeners.size,
  };
}

test('renders one closed semantic glossary with fixed known terms and hostile input inert', () => {
  const html = renderTermHelp('compile', ['compile', 'export', 'compile', '<img src=x onerror=1>']);
  assert.match(html, /<details class="term-help" data-term-help-group="compile">/);
  assert.match(html, /<summary><span class="term-help__label">Help with terms<\/span><span class="term-help__topics">Compile · Export<\/span><\/summary>/);
  assert.match(html, /<dl class="term-help__definitions">/);
  assert.match(html, /<div data-term-help="compile"><dt>Compile<\/dt><dd>/);
  assert.match(html, /Generates instruction text from one saved version/);
  assert.match(html, /Writes an unencrypted \(plaintext\) file/);
  assert.equal((html.match(/<details/g) ?? []).length, 1);
  assert.equal((html.match(/data-term-help="compile"/g) ?? []).length, 1);
  assert.doesNotMatch(html, /<img src=x/);
  assert.equal(renderTermHelp('__proto__', ['compile']), '');
  assert.equal(renderTermHelp('constructor', ['compile']), '');
  assert.equal(renderTermHelp('compile', ['toString']), '');
});

test('puts closed help in normal route flow and keeps action markup outside it', () => {
  const skills = renderSkillsView(createSkillsController().getState());
  assert.match(skills, /data-term-help-group="skills"/);
  assert.match(skills, /data-term-help="review"/);
  const help = skills.match(/<details[^>]*data-term-help-group="skills"[^>]*>[\s\S]*?<\/details>/g) ?? [];
  assert.equal(help.length, 1);
  assert.doesNotMatch(help[0], /<details[^>]* open|id="skills-create"/);
  assert.match(skills, /data-skills-disclosure="create"/);
  assert.match(skills, /id="skills-create"/);
  const analysis = readFileSync(new URL('../preview/analyze.mjs', import.meta.url), 'utf8');
  assert.match(analysis, /renderTermHelp\('analysis', \['source', 'saved-source', 'sections'\]\)/);
  const css = readFileSync(new URL('../preview/visual-system.css', import.meta.url), 'utf8');
  assert.match(css, /\.term-help summary \{[\s\S]*min-height: 44px/);
  assert.match(css, /\.term-help__definitions \{[\s\S]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/);
  assert.match(css, /\.term-help summary::after \{[\s\S]*content: ''[\s\S]*transform: rotate\(-45deg\)/);
  assert.match(css, /\.term-help\[open\] summary::after \{ transform: rotate\(45deg\); \}/);
  assert.match(css, /@media \(forced-colors: active\)[\s\S]*\.term-help/);
});

test('keeps one open help panel in its bound root', () => {
  const view = fixture();
  const binding = bindTermHelp(view.root);
  view.first.open = true;
  view.second.open = true;
  view.fireRoot('toggle', { target: view.second });
  assert.equal(view.first.open, false);
  assert.equal(view.second.open, true);
  binding.dispose();
});

test('escape closes only open local help, focuses its summary, and stops controller escape', () => {
  const view = fixture();
  const binding = bindTermHelp(view.root);
  view.first.open = true;
  let prevented = false;
  let stopped = false;
  view.fireRoot('keydown', { key: 'Escape', target: eventTarget(view.first), preventDefault() { prevented = true; }, stopPropagation() { stopped = true; } });
  assert.equal(view.first.open, false);
  assert.equal(view.first.summary.focused, true);
  assert.equal(prevented, true);
  assert.equal(stopped, true);
  prevented = false; stopped = false;
  view.fireRoot('keydown', { key: 'Escape', target: eventTarget(), preventDefault() { prevented = true; }, stopPropagation() { stopped = true; } });
  assert.equal(prevented, false);
  assert.equal(stopped, false);
  binding.dispose();
});

test('outside help click closes other local groups without changing focus or blocking the click', () => {
  const view = fixture();
  const binding = bindTermHelp(view.root);
  view.first.open = true;
  view.other.open = true;
  view.fireDocument('click', { target: eventTarget(view.first) });
  assert.equal(view.first.open, true);
  assert.equal(view.other.open, false);
  view.fireDocument('click', { target: eventTarget() });
  assert.equal(view.first.open, false);
  assert.equal(view.first.summary.focused, false);
  binding.dispose();
});

test('dispose removes the root and document listeners', () => {
  const view = fixture();
  const binding = bindTermHelp(view.root);
  assert.equal(view.listenerCount(), 3);
  binding.dispose();
  assert.equal(view.listenerCount(), 0);
});
