import { catalogKeys, icon } from './icons.mjs';

const sizes = [16, 20, 24, 40];
const grid = document.querySelector('#gallery-grid');
const themeButton = document.querySelector('#gallery-theme');
let theme = 'dark';

function render() {
  document.documentElement.dataset.theme = theme;
  themeButton.textContent = theme === 'dark' ? 'Light mode' : 'Dark mode';
  grid.innerHTML = catalogKeys.map(name => `<article class="icon-gallery__card"><h2>${name}</h2><div class="icon-gallery__sizes">${sizes.map(size => `<span><b>${icon(name, { size })}</b><small>${size}</small></span>`).join('')}</div></article>`).join('');
}

themeButton.addEventListener('click', () => {
  theme = theme === 'dark' ? 'light' : 'dark';
  render();
  themeButton.focus({ preventScroll: true });
});

render();
