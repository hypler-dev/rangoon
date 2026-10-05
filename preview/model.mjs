export const routes = [
  ['command', 'Command Center'], ['import', 'Import & Analyze'], ['decompose', 'Decompose'],
  ['merge', 'Merge & Split'], ['skills', 'Skills'], ['workflows', 'Workflows'],
  ['connectors', 'Connectors'], ['evidence', 'Evidence'], ['release', 'Release Plan'],
];

export function filterItems(items, query, keys = ['name', 'title', 'detail', 'family']) {
  const needle = query.trim().toLowerCase();
  if (!needle) return items;
  return items.filter((item) => keys.some((key) => String(item[key] || '').toLowerCase().includes(needle)));
}

export function canCompile({ conflicts = 0, semanticLoss = false }) {
  return conflicts === 0 && semanticLoss === false;
}

export function workflowOutcome(outcome) {
  if (outcome === 'unknown') return { retry: false, label: 'Unknown outcome. Do not retry.' };
  if (outcome === 'denied') return { retry: false, label: 'Denied by local policy fixture.' };
  return { retry: false, label: 'Demo only. No execution performed.' };
}
