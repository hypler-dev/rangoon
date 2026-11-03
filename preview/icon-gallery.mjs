import { catalogKeys, icon } from './icons.mjs';
import { vendorMark } from './vendor-marks.mjs';

const sizes = [16, 20, 24, 40];
const groups = [
  ['Actions', ['edit','save','check','undo','redo','compile','export','import','merge','decompose','restore','copy','clear','search','filter']],
  ['Records and systems', ['source','skill','workflow','folder','database','bundle','model','agent','api','terminal','cloud','network','connector','gate']],
  ['Storage and state', ['lock','unlock','encrypted','plaintext','history','warning','unknown','unavailable','evidence','approval','outcome']],
];
const grouped = new Set(groups.flatMap(([, names]) => names));
groups.push(['Navigation and graph', catalogKeys.filter(name => !grouped.has(name))]);
const grid = document.querySelector('#gallery-grid');
const themeButton = document.querySelector('#gallery-theme');
let theme = 'dark';

function render() {
  document.documentElement.dataset.theme = theme;
  themeButton.textContent = theme === 'dark' ? 'Light mode' : 'Dark mode';
  const card = name => `<article class="icon-gallery__card"><h3>${name}</h3><div class="icon-gallery__sizes">${sizes.map(size => `<span><b>${icon(name, { size })}</b><small>${size}</small></span>`).join('')}</div></article>`;
  grid.innerHTML = groups.map(([label, names]) => `<section class="icon-gallery__group"><h2>${label}</h2><div class="icon-gallery__grid">${names.map(card).join('')}</div></section>`).join('');
  document.querySelector('#gallery-software').innerHTML = `<article><img src="assets/brand-symbol.png" width="28" height="28" alt=""><strong>Rangoon</strong><small>Application identity</small></article><article>${vendorMark('claude', 24)}<strong>Claude</strong><small>CLAUDE.md format identity</small></article><article><img src="assets/authority/lnsat-mark.webp" width="28" height="28" alt=""><strong>LNSAT</strong><small>Independent authority engine</small></article><article><img src="assets/foundations/vusl-logo.png" width="28" height="28" alt=""><strong>VUSL</strong><small>Independent semantic layer</small></article>` + [['agentgateway','agentgateway','Gateway catalog identity'],['cerbos','Cerbos','Authorization catalog identity'],['permit','Permit','Authorization / middleware identity'],['invariant','Invariant','Guardrail catalog identity']].map(([id,name,detail])=>`<article>${vendorMark(id,24)}<strong>${name}</strong><small>${detail}</small></article>`).join('');
}

themeButton.addEventListener('click', () => {
  theme = theme === 'dark' ? 'light' : 'dark';
  render();
  themeButton.focus({ preventScroll: true });
});

render();
