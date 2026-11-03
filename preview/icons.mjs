import { colorIconArtwork } from './color-icon-artwork.mjs';

const folded = 'M4 3.75h10.2l5.8 5.8v10.7H4zM14.2 3.75v5.8H20';

const drawings = Object.freeze({
  command: 'M12 3.75 20.25 12 12 20.25 3.75 12zM12 8.25v7.5M8.25 12h7.5M12 3.75v-1M20.25 12h1M12 20.25v1M3.75 12h-1',
  import: `${folded}M12 7.5v8M8.75 12.25 12 15.5l3.25-3.25M7 19.25h10`,
  decompose: 'M10 4.5h4v4h-4zM12 8.5v2.5M6 12.5h4v4H6zM14 12.5h4v4h-4zM12 11H8M12 11h4',
  merge: 'M7 6.5h4v4H7zM7 13.5h4v4H7zM15 10h4v4h-4zM11 8.5h2c1.1 0 2 .9 2 2M11 15.5h2c1.1 0 2-.9 2-2',
  skill: 'M6 6.5h10l2 2v9H8l-2-2zM8 4.5h10l2 2M8 8.5h7M8 12h7M8 15.5h4',
  workflow: 'M4.5 4.5h4v4h-4zM10 10h4v4h-4zM15.5 15.5h4v4h-4zM8.5 6.5H10v3.5M14 12h1.5v3.5',
  check: 'M5.5 4.5h13v15h-13zM9 8h6M9 11.5h3M9 15l2 2 4-4',
  branch: 'M12 3.5 16 7.5 12 11.5 8 7.5zM12 11.5v2M5 16h4v4H5zM15 16h4v4h-4zM7 16v-2.5h10V16',
  connector: 'M8.5 7.25h5v5h2.25a2.25 2.25 0 0 1 2.25 2.25v2M10.5 12.25v2M13.5 12.25v2M7.5 16.5h10v2h-10zM6.5 10.25h2M6.5 14.25h2',
  evidence: `${folded}M8 9h8M8 12.5h5M8 16h3M15.5 15.5l1 1 2-2.25`,
  roadmap: `${folded}M7.5 16.5 10.5 13l2.5 2.25 4-5M7.5 19.25h9M8 8.5h.01M12 8.5h.01M16 8.5h.01`,
  source: `${folded}M8 9h8M8 12.5h8M8 16h5M6.5 9h.01M6.5 12.5h.01M6.5 16h.01`,
  bundle: 'M12 3.75 18.25 7.25 12 10.75 5.75 7.25zM5.75 7.25v6.5L12 17.25v-6.5M18.25 7.25v6.5L12 17.25M8.75 15.5v3.25L15 22v-3.25',
  gate: 'M12 4.25 19.75 12 12 19.75 4.25 12zM12 4.25v15.5M12 9h3.25M12 15h3.25',
  outcome: 'M6 6.5h8.5l3.5 3.5v7.5H9.5L6 14.5zM14.5 6.5V10H18M9.5 13h5M9.5 16h2.5M18 18.5l2 2M20 18.5l-2 2',
  search: `M10.5 5.5a5 5 0 1 0 0 10 5 5 0 0 0 0-10ZM14.25 14.25 19 19`,
  density: `M6 7h12M6 12h12M6 17h12M8 5.75v2.5M12 10.75v2.5M16 15.75v2.5`,
  focus: `M9 5H5v4M15 5h4v4M19 15v4h-4M5 15v4h4`,
  theme: `M12 4.5v2M12 17.5v2M19.5 12h-2M6.5 12h-2M17.3 6.7l-1.4 1.4M8.1 15.9l-1.4 1.4M17.3 17.3l-1.4-1.4M8.1 8.1 6.7 6.7M12 8.25a3.75 3.75 0 1 0 0 7.5 3.75 3.75 0 0 0 0-7.5Z`,
  chevron: 'm8.5 10 3.5 3.5 3.5-3.5',
  unavailable: `M7 7l10 10M17 7 7 17M5.25 12a6.75 6.75 0 1 0 13.5 0 6.75 6.75 0 0 0-13.5 0Z`,
  unknown: `M9.75 9.5a2.45 2.45 0 0 1 4.65 1.1c0 1.75-2.4 2.05-2.4 3.8M12 17.25h.01M5.25 12a6.75 6.75 0 1 0 13.5 0 6.75 6.75 0 0 0-13.5 0Z`,
  research: `${folded}M9 8.5h6M12 8.5v7M9.5 15.5h5M8 18.5h8`,
  approval: 'M12 4.5a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5ZM5.5 20v-2.5a6.5 6.5 0 0 1 13 0V20M16.5 13.5l3 3-3 3',
  arrow: 'M5 12h13M13 7l5 5-5 5',
  plus: 'M12 5v14M5 12h14',
  save: `${folded}M8 4v5h7V4M8 19v-5h8v5`,
  clear: 'M6.5 7.5h11M9.25 7.5v-2h5.5v2M9.5 10.5v5M14.5 10.5v5M8 7.5l.7 11h6.6l.7-11',
  more: 'M5 11.5h1v1H5zM11.5 11.5h1v1h-1zM18 11.5h1v1h-1z',
  zoomin: 'M10 4.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11ZM14 14l5.5 5.5M7.5 10h5M10 7.5v5',
  zoomout: 'M10 4.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11ZM14 14l5.5 5.5M7.5 10h5',
  panleft: 'M19 12H5M10 7l-5 5 5 5',
  panright: 'M5 12h14M14 7l5 5-5 5',
  reset: 'M5 9a7 7 0 1 1 .5 7M5 4.5V9h4.5',
  undo: 'M9 5 4.5 9.5 9 14M5 9.5h9a5 5 0 0 1 0 10H12',
  redo: 'M15 5 19.5 9.5 15 14M19 9.5h-9a5 5 0 0 0 0 10h2',
  edit: 'M4.5 19.5 5.5 15 16 4.5 19.5 8 9 18.5zM13.5 7 17 10.5M4.5 19.5 9 18.5',
  compile: 'M4 4.5h9l5 5V13M13 4.5v5h5M7 10 5 12l2 2M11 10l2 2-2 2M8.5 15.5h2M15 17h6M18 14l3 3-3 3',
  export: 'M12 3.5v11M8.5 7 12 3.5 15.5 7M5 12v7.5h14V12',
  lock: 'M7.5 10V7.5a4.5 4.5 0 0 1 9 0V10M5.5 10h13v10h-13zM12 14v2.5',
  unlock: 'M8 10V7.5a4.5 4.5 0 0 1 8.5-2M5.5 10h13v10h-13zM12 14v2.5',
  encrypted: 'M14 4.75c0 1.25-2.25 2.25-5 2.25S4 6 4 4.75 6.25 2.5 9 2.5s5 1 5 2.25ZM4 4.75v12.5c0 1.25 2.25 2.25 5 2.25M4 10c0 1.25 2.25 2.25 5 2.25M14.5 13V11a2.5 2.5 0 0 1 5 0v2M12.5 13h9v7.5h-9zM17 16v1.5',
  plaintext: 'M5 3.5h9l5 5v12H5zM14 3.5v5h5M8 11.5h8M8 15h8M8 18.5h4',
  database: 'M19 5c0 1.65-3.15 3-7 3S5 6.65 5 5s3.15-3 7-3 7 1.35 7 3ZM5 5v13c0 1.65 3.15 3 7 3s7-1.35 7-3V5M5 11.5c0 1.65 3.15 3 7 3s7-1.35 7-3',
  history: 'M5 5v5h5M5.5 9a7.5 7.5 0 1 1-.25 6M12 7.5v5l3 2',
  restore: 'M8 5 4 9l4 4M4.5 9H13a6 6 0 0 1 0 12H9M12 5h7v7',
  copy: 'M9 8h11v13H9zM15 8V3H4v13h5',
  filter: 'M3.5 5h17l-6.5 8v6l-4-2v-4z',
  folder: 'M3.5 6h6l2 2h9v11h-17zM3.5 8V4.5H10l2 2',
  terminal: 'M3 4.5h18v15H3zM6.5 8.5 10 12l-3.5 3.5M13 15.5h4.5',
  model: 'M8 8h8v8H8zM9.5 3v5M14.5 3v5M9.5 16v5M14.5 16v5M3 9.5h5M3 14.5h5M16 9.5h5M16 14.5h5',
  agent: 'M12 2.5 20 7v10l-8 4.5L4 17V7zM8 9h8v6H8zM12 6v3M12 15v3',
  api: 'M8 6 2.5 12 8 18M16 6l5.5 6-5.5 6M14 4l-4 16',
  cloud: 'M7 18.5h10.5a4 4 0 0 0 .5-7.95 6 6 0 0 0-11.65-1.7A4.85 4.85 0 0 0 7 18.5Z',
  network: 'M9 3h6v5H9zM3 16h6v5H3zM15 16h6v5h-6zM12 8v4M6 16v-4h12v4',
  warning: 'M12 3 22 20H2zM12 8.5v5.5M12 17h.01',
  viewcube: 'M12 3.5 20 8v8l-8 4.5L4 16V8zM4 8l8 4.5L20 8M12 12.5v8',
});

export const catalogKeys = Object.freeze(Object.keys(drawings));
const supportedSizes = new Set([16, 20, 24, 40]);

/** Return one decorative Foldline SVG from the closed local catalog. */
export function icon(name, { size = 20 } = {}) {
  if (typeof name !== 'string' || !Object.hasOwn(drawings, name) || !supportedSizes.has(size)) return '';
  return `<svg class="foldline-icon foldline-icon--${name}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><g class="foldline-color" stroke="none">${colorIconArtwork(name, { size })}</g><path class="foldline-monochrome" d="${drawings[name]}"/></svg>`;
}
