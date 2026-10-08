import { icon } from './icons.mjs';
import { createWorkflowController } from './workflow-model.mjs';
import { bindWorkflowView } from './workflow-view.mjs';

const app = document.querySelector('#workflow-playground-app');
const announcer = document.querySelector('#workflow-playground-announcer');
let theme = 'dark';
let view = null;

document.documentElement.dataset.theme = theme;

const groups = [
  ['Discover', [['analysis', 'import', 'Import & Analyze'], ['decompose', 'decompose', 'Decompose'], ['merge', 'merge', 'Merge'], ['split', 'decompose', 'Split']]],
  ['Build', [['skills', 'skill', 'Skills'], ['workflows', 'workflow', 'Native workflows'], ['compile', 'bundle', 'Compile']]],
  ['Connect', [['model-assistance', 'research', 'Model assistance'], ['engine', 'gate', 'Engine integration']]],
  ['Workspace', [['workspace', 'bundle', 'Data']]],
];

function rail() {
  const link = ([name, symbol, label]) => `<a href="analyze.html#${name}">${icon(symbol, { size: 16 })}<span>${label}</span></a>`;
  return `<aside class="analysis-rail"><a class="analysis-brand" href="index.html"><img src="assets/brand-symbol.png" alt=""><span>Rangoon</span></a><nav class="analysis-nav" aria-label="Desktop areas"><details class="analysis-nav-drawer" open><summary>Navigate areas</summary><div class="analysis-nav-groups">${groups.map(([label, links]) => `<section class="analysis-nav-group" aria-label="${label}"><p>${label}</p>${links.map(link).join('')}</section>`).join('')}<section class="analysis-nav-group analysis-nav-group--sample" aria-label="Preview"><p>Preview</p><a href="index.html">${icon('command', { size: 16 })}<span>Sample design preview</span></a><a href="workflow-playground.html" aria-current="page">${icon('workflow', { size: 16 })}<span>Synthetic playground</span></a></section></div></details></nav><div class="analysis-companion"><img src="assets/mascot.webp" width="90" height="60" alt=""><p>Small pieces.<br>Greater possibilities.</p></div><button id="playground-theme" class="analysis-theme" type="button">${icon('theme', { size: 16 })}Light mode</button></aside>`;
}

const controller = createWorkflowController({
  playground: true,
  onChange: state => {
    view?.render();
    announcer.textContent = state.message;
  },
});

function render() {
  if (!document.querySelector('#workflow-playground-root')) {
    app.innerHTML = `<div class="analysis-shell" data-area="workflows">${rail()}<main id="analysis-main" tabindex="-1"><div id="workflow-playground-root"></div></main></div>`;
    view = bindWorkflowView(document.querySelector('#workflow-playground-root'), controller);
  } else view.render();
  const button = document.querySelector('#playground-theme');
  if (button) button.innerHTML = `${icon('theme', { size: 16 })}${theme === 'light' ? 'Dark mode' : 'Light mode'}`;
}

document.addEventListener('click', event => {
  if (!event.target.closest('#playground-theme')) return;
  theme = theme === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = theme;
  render();
});

async function seed() {
  await controller.beginNew();
  controller.addNode('input', { x: 56, y: 90 });
  controller.addNode('output', { x: 360, y: 90 });
  controller.connectControl(0, 'next', 1);
  controller.connectData(0, 'text', 1, 'text');
}

render();
await seed();

window.addEventListener('beforeunload', () => { void controller.clear(true); });
