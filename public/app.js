(() => {
  const $ = (s, root = document) => root.querySelector(s);
  const $$ = (s, root = document) => [...root.querySelectorAll(s)];
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const mobileToggle = $('.mobile-toggle');
  const mobileNav = $('#mobile-nav');
  const closeMenus = () => $$('.nav-trigger').forEach(trigger => {
    trigger.setAttribute('aria-expanded', 'false');
    document.getElementById(trigger.getAttribute('aria-controls')).hidden = true;
  });
  $$('.nav-trigger').forEach(trigger => {
    const panel = document.getElementById(trigger.getAttribute('aria-controls'));
    const open = () => { closeMenus(); trigger.setAttribute('aria-expanded', 'true'); panel.hidden = false; };
    trigger.addEventListener('click', () => trigger.getAttribute('aria-expanded') === 'true' ? closeMenus() : open());
    trigger.addEventListener('keydown', event => {
      if (event.key === 'ArrowDown') { event.preventDefault(); open(); $('a', panel)?.focus(); }
    });
    trigger.parentElement.addEventListener('focusout', event => {
      if (!trigger.parentElement.contains(event.relatedTarget)) closeMenus();
    });
  });
  document.addEventListener('click', event => { if (!event.target.closest('.nav-item')) closeMenus(); });
  const closeMobile = (returnFocus = false) => {
    mobileToggle.setAttribute('aria-expanded', 'false');
    mobileToggle.setAttribute('aria-label', 'Open navigation');
    mobileNav.hidden = true;
    document.body.style.overflow = '';
    if (returnFocus) mobileToggle.focus();
  };
  mobileToggle.addEventListener('click', () => {
    if (mobileToggle.getAttribute('aria-expanded') === 'true') return closeMobile();
    closeMenus(); mobileNav.hidden = false; mobileToggle.setAttribute('aria-expanded', 'true');
    mobileToggle.setAttribute('aria-label', 'Close navigation'); document.body.style.overflow = 'hidden';
    $('summary', mobileNav)?.focus();
  });
  window.matchMedia('(min-width: 901px)').addEventListener('change', event => { if (event.matches) closeMobile(); });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      const active = $('.nav-trigger[aria-expanded="true"]'); closeMenus(); active?.focus();
      if (!mobileNav.hidden) closeMobile(true);
    }
    if (!mobileNav.hidden && event.key === 'Tab') {
      const links = [mobileToggle, ...$$('summary, a', mobileNav)].filter(el => el.getClientRects().length);
      const current = links.indexOf(document.activeElement);
      if (event.shiftKey && current === 0) { event.preventDefault(); links.at(-1).focus(); }
      if (!event.shiftKey && current === links.length - 1) { event.preventDefault(); links[0].focus(); }
    }
  });
  const tabs = $$('[data-tab]');
  function activateTab(tab) {
    tabs.forEach(item => { const selected = item === tab; item.setAttribute('aria-selected', String(selected)); item.tabIndex = selected ? 0 : -1; document.getElementById(item.getAttribute('aria-controls')).hidden = !selected; });
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => activateTab(tab));
    tab.addEventListener('keydown', event => {
      let target;
      if (event.key === 'ArrowRight') target = tabs[(index + 1) % tabs.length];
      if (event.key === 'ArrowLeft') target = tabs[(index + tabs.length - 1) % tabs.length];
      if (event.key === 'Home') target = tabs[0];
      if (event.key === 'End') target = tabs.at(-1);
      if (target) { event.preventDefault(); activateTab(target); target.focus(); }
    });
  });
  const preview = $('#preview-dialog');
  $$('.preview-open').forEach(button => button.addEventListener('click', () => {
    $('#preview-title').textContent = button.dataset.title;
    $('#preview-image').src = button.dataset.image;
    $('#preview-image').alt = `${button.dataset.title} design preview, with illustrative sample data`;
    preview.showModal();
  }));
  $$('dialog').forEach(dialog => {
    $('.dialog-close', dialog)?.addEventListener('click', () => dialog.close());
    dialog.addEventListener('click', event => {
      const box = dialog.getBoundingClientRect();
      if (event.target === dialog && (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom)) dialog.close();
    });
  });
  let searchData;
  const searchDialog = $('#search-dialog');
  const searchInput = $('#site-search');
  const results = $('#search-results');
  const renderSearch = () => {
    if (!searchData) return;
    const query = searchInput.value.trim().toLowerCase();
    const matches = searchData.filter(item => `${item.title} ${item.description} ${item.path}`.toLowerCase().includes(query)).slice(0, 9);
    results.replaceChildren();
    if (!matches.length) { const p = document.createElement('p'); p.className = 'no-results'; p.textContent = 'No matching pages. Try “skills”, “policy”, or “open source”.'; results.append(p); }
    matches.forEach(item => { const a = document.createElement('a'); a.href = item.path; const title = document.createElement('strong'); title.textContent = item.title; const desc = document.createElement('p'); desc.textContent = item.description; a.append(title, desc); results.append(a); });
  };
  async function openSearch() {
    closeMobile(); closeMenus(); if (preview.open) preview.close();
    if (!searchDialog.open) searchDialog.showModal(); searchInput.focus();
    if (!searchData) {
      results.textContent = 'Loading page index…';
      try { const response = await fetch('/search.json'); if (!response.ok) throw new Error('Search index unavailable'); searchData = await response.json(); }
      catch { results.textContent = 'Search is temporarily unavailable. Use the navigation to explore the site.'; return; }
    }
    renderSearch();
  }
  $$('.search-open').forEach(button => button.addEventListener('click', openSearch));
  searchInput.addEventListener('input', renderSearch);
  document.addEventListener('keydown', event => { if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); openSearch(); } });
  let toastTimer;
  function toast(text) { const el = $('#toast'); el.textContent = text; el.classList.add('visible'); clearTimeout(toastTimer); toastTimer = setTimeout(() => el.classList.remove('visible'), 3000); }
  $$('.copy-button').forEach(button => button.addEventListener('click', async () => {
    try { await navigator.clipboard.writeText($('code', button.closest('.code-block')).textContent); toast('Commands copied.'); }
    catch { toast('Could not copy. Select the commands to copy them manually.'); }
  }));
  $$('[data-filter]').forEach(button => button.addEventListener('click', () => {
    $$('[data-filter]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
    let count = 0;
    $$('[data-category]').forEach(card => { card.hidden = button.dataset.filter !== 'All' && card.dataset.category !== button.dataset.filter; if (!card.hidden) count++; });
    $('#filter-status').textContent = `${count} planned target${count === 1 ? '' : 's'}`;
  }));
  if ('IntersectionObserver' in window && !reducedMotion.matches) {
    document.documentElement.classList.add('js-reveals');
    const observer = new IntersectionObserver(entries => entries.forEach(entry => { if (entry.isIntersecting) { entry.target.classList.add('is-visible'); observer.unobserve(entry.target); } }), {threshold: 0.08});
    $$('.reveal').forEach(el => observer.observe(el));
    reducedMotion.addEventListener('change', event => { if (event.matches) document.documentElement.classList.remove('js-reveals'); });
  }
})();
