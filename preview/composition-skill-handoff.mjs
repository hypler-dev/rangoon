// Presentation/navigation only. Native reads resolve the saved reference.
const exactId = (value, prefix) => typeof value === 'string' && new RegExp(`^${prefix}:[a-f0-9]{64}$`).test(value);

export function compositionResultSelection(state, capabilityId, revisionId) {
  if (!state?.bridgeAvailable || state.pending || !exactId(capabilityId, 'capability') || !exactId(revisionId, 'revision')) return null;
  const result = state.results?.find(item => item.id === capabilityId && item.revision?.id === revisionId);
  return result ? { capabilityId, revisionId } : null;
}

export function compositionResultIssue({ composition, skills, capabilityId, revisionId }) {
  if (!compositionResultSelection(composition.getState(), capabilityId, revisionId)) return 'This saved result is no longer available. Open the current composition results.';
  const state = skills.getState();
  if (!state.bridgeAvailable) return 'Open Rangoon desktop to inspect this saved skill.';
  if (state.pending) return 'Wait for the current skill operation to finish.';
  if (skills.hasUnsavedDraft(capabilityId)) return 'Save or discard this skill’s unsaved edits in Skills before opening the saved result.';
  return null;
}

export async function openCompositionResultInSkills({ composition, skills, capabilityId, revisionId, isCurrent }) {
  if (!isCurrent() || compositionResultIssue({ composition, skills, capabilityId, revisionId })) return false;
  // Skills is pending during its read; only the origin and retained-draft guards apply here.
  const current = () => isCurrent() && Boolean(compositionResultSelection(composition.getState(), capabilityId, revisionId)) && !skills.hasUnsavedDraft(capabilityId);
  const opened = await skills.open(capabilityId, revisionId, { isCurrent: current });
  return Boolean(opened && current());
}

export function compositionResultFocus(state, selection) {
  if (!selection || state?.pending) return null;
  return state?.selected?.id === selection.capabilityId && state.selected.revision?.id === selection.revisionId && state.viewedRevisionId === selection.revisionId
    ? '#skills-editor-title' : '#skills-title';
}
