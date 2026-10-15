import { icon } from './icons.mjs';
import { createWorkflowController } from './workflow-model.mjs';
import { bindWorkflowView } from './workflow-view.mjs';
import { bindWorkbenchNavigation, renderWorkbenchNavigation } from './workbench-navigation.mjs';

const app = document.querySelector('#workflow-playground-app');
const announcer = document.querySelector('#workflow-playground-announcer');
let theme = 'dark';
let view = null;
const navigation = bindWorkbenchNavigation(app, { window, document });

document.documentElement.dataset.theme = theme;

const controller = createWorkflowController({
  playground: true,
  onChange: state => {
    view?.render();
    announcer.textContent = state.message;
  },
});

function render() {
  if (!document.querySelector('#workflow-playground-root')) {
    const state = navigation.capture();
    app.innerHTML = `<div class="analysis-shell" data-area="workflows">${renderWorkbenchNavigation({ area: 'workflows', surface: 'workflow-playground', theme, open: navigation.compact() ? state.open : true })}<main id="analysis-main" tabindex="-1"><div id="workflow-playground-root"></div></main></div>`;
    queueMicrotask(() => navigation.restore(state));
    view = bindWorkflowView(document.querySelector('#workflow-playground-root'), controller, { initialMode: 'flat', initialLibraryCollapsed: true });
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
  for (const [kind, position] of [['input', { x: 40, y: 40 }], ['capability', { x: 285, y: 40 }], ['check', { x: 530, y: 40 }], ['branch', { x: 530, y: 260 }], ['checkpoint', { x: 285, y: 260 }], ['output', { x: 40, y: 260 }]]) controller.addNode(kind, position);
  controller.connectControl(0, 'next', 1);
  controller.connectControl(1, 'next', 2);
  controller.connectControl(2, 'next', 3);
  controller.connectControl(3, 'true', 4);
  controller.connectControl(3, 'false', 5);
  controller.connectControl(4, 'next', 5);
  controller.connectData(0, 'text', 1, 'text');
  controller.connectData(1, 'text', 2, 'value');
  controller.connectData(2, 'passed', 3, 'condition');
  controller.select({ kind: 'node', index: 0 });
}

render();
await seed();

window.addEventListener('beforeunload', () => { navigation.dispose(); void controller.clear(true); });
