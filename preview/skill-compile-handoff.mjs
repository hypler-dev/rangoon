// Navigation eligibility only. The native host resolves and validates saved records.
const id = (value, prefix) => typeof value === 'string' && new RegExp(`^${prefix}:[a-f0-9]{64}$`).test(value);

export function skillCompileSelection(state) {
  const selected = state?.selected;
  if (!state?.bridgeAvailable || state.pending || state.needsReload || !selected
    || !id(selected.id, 'capability') || !id(selected.revision?.id, 'revision')
    || state.viewedRevisionId !== selected.revision.id) return null;
  if (state.draft?.capabilityId === selected.id && state.draft.dirty) return null;
  const current = selected.revision.id === selected.latestRevisionId;
  if (current && (state.draft?.capabilityId !== selected.id
    || state.draft?.baseRevisionId !== selected.revision.id)) return null;
  return { capabilityId: selected.id, revisionId: selected.revision.id };
}

export function skillCompileHint(state) {
  if (!state?.bridgeAvailable) return 'Open Rangoon desktop to prepare a saved revision.';
  if (state.pending) return 'Wait for the current skill operation to finish.';
  if (state.needsReload) return 'Reload the current skill to resolve its conflict first.';
  if (state.draft?.capabilityId === state.selected?.id && state.draft?.dirty) {
    return state.selected.revision.id === state.selected.latestRevisionId
      ? 'Save or discard local edits before preparing this revision.'
      : 'Return to current, then save or discard local edits before preparing a saved revision.';
  }
  return skillCompileSelection(state) ? 'Uses this saved revision. Choose a format and compile on the next screen.' : 'Choose a saved revision first.';
}

export async function openSkillInCompilation({ skills, compilation, isCurrent }) {
  const selection = skillCompileSelection(skills.getState());
  if (!selection || !isCurrent()) return false;
  const current = () => {
    const next = skillCompileSelection(skills.getState());
    return isCurrent() && next?.capabilityId === selection.capabilityId && next.revisionId === selection.revisionId;
  };
  const opened = await compilation.openPinnedRevision(selection.capabilityId, selection.revisionId, { isCurrent: current });
  return Boolean(opened && current());
}
