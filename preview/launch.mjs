const splashMarkup = ({ status = 'Opening local workspace…', preview = false } = {}) => `
  <section class="splash-card" aria-labelledby="splash-title">
    <svg class="splash-network" viewBox="0 0 820 560" aria-hidden="true" focusable="false">
      <path d="M-20 420C170 292 206 521 377 374S600 170 855 241" />
      <line x1="56" y1="425" x2="252" y2="348" /><line x1="252" y1="348" x2="412" y2="406" /><line x1="412" y1="406" x2="636" y2="249" /><line x1="636" y1="249" x2="787" y2="310" />
      <circle cx="56" cy="425" r="3" /><circle cx="252" cy="348" r="3" /><circle cx="412" cy="406" r="3" /><circle cx="636" cy="249" r="3" /><circle cx="787" cy="310" r="3" />
    </svg>
    <div class="splash-content">
      <img class="splash-mark" src="assets/brand-symbol.png" alt="">
      <h1 id="splash-title" class="splash-wordmark">Rangoon<span>.ai</span></h1>
      <p class="splash-purpose">Build capable agents. Keep them in bounds.</p>
      <div class="splash-rule" aria-hidden="true"></div>
      <p class="splash-status" data-splash-status role="status" aria-live="polite">${status}</p>
      <p class="splash-boundary">Local workspace only. No upload or provider connection.</p>
      <div class="splash-actions" data-splash-actions>${preview ? '<a class="splash-link" href="analyze.html">Open workbench</a><a class="splash-link" href="index.html">Sample design preview</a>' : ''}</div>
      <p class="splash-footer">Development preview</p>
    </div>
  </section>`;

function mount(container, options) {
  container.innerHTML = splashMarkup(options);
  return {
    status: container.querySelector('[data-splash-status]'),
    actions: container.querySelector('[data-splash-actions]'),
  };
}

export function createStartupSplash({ container, app }) {
  const ui = mount(container, {});
  const skip = document.querySelector('#analysis-skip');
  let completed = false;
  let userInteracted = false;
  let retry = null;
  let timeout;
  const armTimeout = () => window.setTimeout(() => {
    if (completed) return;
    ui.status.textContent = 'Workspace is taking longer than expected.';
    ui.actions.innerHTML = '<button class="splash-link" type="button" data-splash-retry>Retry workspace</button><button class="splash-link" type="button" data-splash-open>Open workbench</button>';
    if (userInteracted) ui.actions.querySelector('[data-splash-retry]')?.focus();
  }, 8000);
  timeout = armTimeout();

  const openWorkbench = () => {
    if (completed) return;
    completed = true;
    window.clearTimeout(timeout);
    container.hidden = true;
    app.inert = false;
    app.removeAttribute('aria-hidden');
    skip?.removeAttribute('inert');
    document.removeEventListener('keydown', handleKeydown);
    if (container.contains(document.activeElement)) (app.querySelector('#analysis-choose') ?? app.querySelector('#analysis-main'))?.focus({ preventScroll: true });
  };
  const sync = state => {
    if (completed) return;
    if (['ready', 'failed', 'unavailable'].includes(state.snapshotsStatus)) openWorkbench();
  };
  const interacted = () => { userInteracted = true; };
  container.addEventListener('pointerdown', interacted, { once: true });
  const handleKeydown = event => {
    interacted();
    if (event.key === 'Escape') openWorkbench();
  };
  document.addEventListener('keydown', handleKeydown);
  container.addEventListener('click', event => {
    if (event.target.closest('[data-splash-open]')) openWorkbench();
    if (event.target.closest('[data-splash-retry]')) {
      ui.status.textContent = 'Opening local workspace…';
      ui.actions.textContent = '';
      window.clearTimeout(timeout);
      timeout = armTimeout();
      retry?.();
    }
  });
  return { sync, openWorkbench, setRetry: handler => { retry = handler; } };
}

const preview = document.querySelector('[data-splash-preview]');
if (preview) mount(preview, { status: 'Launch preview', preview: true });
