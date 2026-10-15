const splashMarkup = ({ status = 'Opening local workspace…', preview = false } = {}) => `
  <section class="splash-card" aria-labelledby="splash-title">
    <div class="splash-content">
      <img class="splash-mark" src="assets/brand-symbol.png" alt="">
      <h1 id="splash-title" class="splash-wordmark">Rangoon<span>.si</span></h1>
      <p class="splash-purpose">Build capable agents. Keep them in bounds.</p>
      <div class="splash-rule" aria-hidden="true"></div>
      <p class="splash-status" data-splash-status role="status" aria-live="polite">${status}</p>
      <p class="splash-boundary">Local workspace only. No upload or provider connection.</p>
      <div class="splash-actions" data-splash-actions>${preview ? '<button class="splash-link" type="button" data-splash-theme>Light theme</button><a class="splash-link" href="analyze.html">Open workbench</a><a class="splash-link" href="index.html">Sample design preview</a>' : ''}</div>
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
  let disposed = false;
  let userInteracted = false;
  let retry = null;
  let timeout;
  const armTimeout = () => window.setTimeout(() => {
    if (completed || disposed) return;
    ui.status.textContent = 'Workspace is taking longer than expected.';
    ui.actions.innerHTML = '<button class="splash-link" type="button" data-splash-retry>Retry workspace</button><button class="splash-link" type="button" data-splash-open>Open workbench</button>';
    if (userInteracted) ui.actions.querySelector('[data-splash-retry]')?.focus();
  }, 8000);
  timeout = armTimeout();

  const openWorkbench = () => {
    if (completed || disposed) return;
    completed = true;
    window.clearTimeout(timeout);
    container.hidden = true;
    app.inert = false;
    app.removeAttribute('aria-hidden');
    skip?.removeAttribute('inert');
    if (skip) skip.hidden = false;
    document.removeEventListener('keydown', handleKeydown);
    container.removeEventListener('pointerdown', interacted);
    container.removeEventListener('click', clicked);
    retry = null;
    if (container.contains(document.activeElement)) (app.querySelector('#analysis-choose') ?? app.querySelector('#analysis-main'))?.focus({ preventScroll: true });
  };
  const sync = state => {
    if (completed || disposed) return;
    if (['ready', 'failed', 'unavailable'].includes(state.snapshotsStatus)) openWorkbench();
  };
  const interacted = () => { if (!disposed) userInteracted = true; };
  container.addEventListener('pointerdown', interacted, { once: true });
  const handleKeydown = event => {
    interacted();
    if (event.key === 'Escape') openWorkbench();
  };
  document.addEventListener('keydown', handleKeydown);
  const clicked = event => {
    if (completed || disposed) return;
    if (event.target.closest('[data-splash-open]')) openWorkbench();
    if (event.target.closest('[data-splash-retry]')) {
      ui.status.textContent = 'Opening local workspace…';
      ui.actions.textContent = '';
      window.clearTimeout(timeout);
      timeout = armTimeout();
      retry?.();
    }
  };
  container.addEventListener('click', clicked);
  return {
    sync, openWorkbench,
    setRetry: handler => { if (!disposed && !completed) retry = handler; },
    purgeForWorkspaceLock() {
      if (disposed) return;
      disposed = true;
      completed = true;
      retry = null;
      window.clearTimeout(timeout);
      document.removeEventListener('keydown', handleKeydown);
      container.removeEventListener('pointerdown', interacted);
      container.removeEventListener('click', clicked);
      ui.status.textContent = '';
      ui.actions.replaceChildren();
      container.hidden = true;
      app.inert = false;
      app.removeAttribute('aria-hidden');
      skip?.setAttribute('inert', '');
      if (skip) skip.hidden = true;
    },
  };
}

const preview = document.querySelector('[data-splash-preview]');
if (preview) {
  let theme = 'dark';
  try {
    const saved = localStorage.getItem('rangoon-theme');
    if (saved === 'light' || saved === 'dark') theme = saved;
  } catch {}
  mount(preview, { status: 'Launch preview', preview: true });
  const applyTheme = () => {
    document.documentElement.dataset.theme = theme;
    const control = preview.querySelector('[data-splash-theme]');
    if (control) control.textContent = theme === 'light' ? 'Dark theme' : 'Light theme';
  };
  applyTheme();
  preview.addEventListener('click', event => {
    if (!event.target.closest('[data-splash-theme]')) return;
    theme = theme === 'light' ? 'dark' : 'light';
    try { localStorage.setItem('rangoon-theme', theme); } catch {}
    applyTheme();
  });
}
