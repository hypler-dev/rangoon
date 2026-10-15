import { escapeText } from './analysis-model.mjs';

const catalog = Object.freeze({
  analysis: Object.freeze({
    source: Object.freeze({ summary: 'Source', copy: 'Original imported text. Reading it does not save it.' }),
    'saved-source': Object.freeze({ summary: 'Saved source', copy: 'A local copy saved explicitly. It can be reopened after restart.' }),
    sections: Object.freeze({ summary: 'Sections', copy: 'Structural groups in source text. Detection does not establish meaning, correctness, or reusable behavior.' }),
  }),
  skills: Object.freeze({
    skill: Object.freeze({ summary: 'Skill', copy: 'A reusable block of saved instructions. Its internal capability record holds revisions; saving does not make it executable.' }),
    revision: Object.freeze({ summary: 'Revision', copy: 'One specific saved version. A pinned revision stays exact; a newer version is never substituted silently.' }),
    provenance: Object.freeze({ summary: 'Provenance', copy: 'Recorded source and editing history. It does not prove a source is trustworthy or authentic.' }),
    review: Object.freeze({ summary: 'Local content review', copy: 'A recorded review of one exact saved revision. It grants no approval, authorization, or execution permission.' }),
  }),
  composition: Object.freeze({
    recipe: Object.freeze({ summary: 'Recipe', copy: 'How output is assembled from saved content.' }),
    coverage: Object.freeze({ summary: 'Coverage', copy: 'A record of source ranges copied, repeated, replaced, left out, or still unassigned.' }),
    preview: Object.freeze({ summary: 'Preview', copy: 'A calculated proposed local result before saving. It is not execution or approval.' }),
    revision: Object.freeze({ summary: 'Pinned revision', copy: 'The exact saved version selected for this operation. A newer version is never substituted silently.' }),
  }),
  compile: Object.freeze({
    compile: Object.freeze({ summary: 'Compile', copy: 'Generates instruction text from one saved version. A text profile is a fixed formatting recipe for a target file. This does not install or run instructions, or prove that another application will load or enforce them.' }),
    export: Object.freeze({ summary: 'Export', copy: 'Writes an unencrypted (plaintext) file. Export does not install, authenticate, or run instructions.' }),
    review: Object.freeze({ summary: 'Local content review', copy: 'A recorded review of one exact saved revision. It grants no approval, authorization, or execution permission.' }),
    provenance: Object.freeze({ summary: 'Provenance', copy: 'Recorded source and editing history. It does not prove a source is trustworthy or authentic.' }),
  }),
});

const validTopics = (scope, topics) => {
  if (typeof scope !== 'string' || !Object.hasOwn(catalog, scope) || !Array.isArray(topics)) return [];
  const group = catalog[scope];
  const seen = new Set();
  return topics.filter(topic => {
    if (typeof topic !== 'string' || !Object.hasOwn(group, topic) || seen.has(topic)) return false;
    seen.add(topic);
    return true;
  });
};

export function renderTermHelp(scope, topics) {
  const entries = validTopics(scope, topics);
  if (!entries.length) return '';
  const labels = entries.map(topic => escapeText(catalog[scope][topic].summary)).join(' · ');
  return `<details class="term-help" data-term-help-group="${scope}"><summary><span class="term-help__label">Help with terms</span><span class="term-help__topics">${labels}</span></summary><dl class="term-help__definitions">${entries.map(topic => {
    const entry = catalog[scope][topic];
    return `<div data-term-help="${topic}"><dt>${escapeText(entry.summary)}</dt><dd>${escapeText(entry.copy)}</dd></div>`;
  }).join('')}</dl></details>`;
}

const openHelp = root => [...root.querySelectorAll?.('details[data-term-help-group][open]') ?? []];
const closeHelp = group => { if (group?.open) group.open = false; };

export function bindTermHelp(root) {
  const document = root?.ownerDocument;
  if (!root?.addEventListener || !document?.addEventListener) return { dispose() {} };
  const toggled = event => {
    const group = event.target?.closest?.('details[data-term-help-group]');
    if (!group?.open || !root.contains(group)) return;
    for (const other of openHelp(root)) if (other !== group) closeHelp(other);
  };
  const keydown = event => {
    if (event.key !== 'Escape') return;
    const group = event.target?.closest?.('details[data-term-help-group][open]');
    if (!group || !root.contains(group)) return;
    event.preventDefault();
    event.stopPropagation();
    closeHelp(group);
    group.querySelector?.('summary')?.focus?.({ preventScroll: true });
  };
  const outside = event => {
    const containingGroup = event.target?.closest?.('[data-term-help-group]');
    for (const group of openHelp(root)) if (group !== containingGroup) closeHelp(group);
  };
  root.addEventListener('toggle', toggled, true);
  root.addEventListener('keydown', keydown, true);
  document.addEventListener('click', outside);
  return {
    dispose() {
      root.removeEventListener('toggle', toggled, true);
      root.removeEventListener('keydown', keydown, true);
      document.removeEventListener('click', outside);
    },
  };
}
