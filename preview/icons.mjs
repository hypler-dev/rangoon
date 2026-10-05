const folded = 'M4 3.75h10.2l5.8 5.8v10.7H4zM14.2 3.75v5.8H20';

const drawings = Object.freeze({
  command: 'M12 3.75 20.25 12 12 20.25 3.75 12zM12 8.25v7.5M8.25 12h7.5M12 3.75v-1M20.25 12h1M12 20.25v1M3.75 12h-1',
  import: `${folded}M12 7.5v8M8.75 12.25 12 15.5l3.25-3.25M7 19.25h10`,
  decompose: 'M10 4.5h4v4h-4zM12 8.5v2.5M6 12.5h4v4H6zM14 12.5h4v4h-4zM12 11H8M12 11h4',
  merge: 'M7 6.5h4v4H7zM7 13.5h4v4H7zM15 10h4v4h-4zM11 8.5h2c1.1 0 2 .9 2 2M11 15.5h2c1.1 0 2-.9 2-2',
  skill: 'M6 6.5h10l2 2v9H8l-2-2zM8 4.5h10l2 2M8 8.5h7M8 12h7M8 15.5h4',
  workflow: 'M4.5 4.5h4v4h-4zM10 10h4v4h-4zM15.5 15.5h4v4h-4zM8.5 6.5H10v3.5M14 12h1.5v3.5',
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
});

export const catalogKeys = Object.freeze(Object.keys(drawings));
const supportedSizes = new Set([16, 20, 24, 40]);

/** Return one decorative Foldline SVG from the closed local catalog. */
export function icon(name, { size = 20 } = {}) {
  if (typeof name !== 'string' || !Object.hasOwn(drawings, name) || !supportedSizes.has(size)) return '';
  return `<svg class="foldline-icon foldline-icon--${name}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="${drawings[name]}"/></svg>`;
}
