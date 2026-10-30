import { escapeText } from './analysis-model.mjs';

// Presentation only: callers must pass escaped, renderer-owned markup, never credentials.
export function renderTechnicalDetails(id, markup, label = 'Technical details') {
  return `<details id="${escapeText(id)}" class="workbench-technical"><summary>${escapeText(label)}</summary><div class="workbench-technical__body">${markup}</div></details>`;
}
