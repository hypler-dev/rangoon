import { escapeText } from './analysis-model.mjs';

export const CAPABILITY_SCHEMA = 'rangoon.capability.v0';
const MAX_TITLE_BYTES = 160;
const MAX_CONTENT_BYTES = 256 * 1024;
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const utf8 = value => new TextEncoder().encode(value).length;
const hasOnly = (value, keys) => object(value) && Object.keys(value).every(key => keys.has(key));
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const id = (value, kind) => typeof value === 'string' && value.startsWith(kind) && hex(value.slice(kind.length));
const time = value => Number.isSafeInteger(value) && value >= 0 && value <= 8_640_000_000_000_000;
const failed = (message = 'The local capability action could not finish.') => ({ outcome: 'failed', error: { message } });
const failure = result => ({ code: typeof result?.error?.code === 'string' ? result.error.code : null, message: typeof result?.error?.message === 'string' ? result.error.message : 'The local capability action could not finish.' });

export function validCapabilityTitle(value) {
  return typeof value === 'string' && value.length > 0 && utf8(value) <= MAX_TITLE_BYTES
    && value.trim() === value && !/[\p{Cc}\u2028\u2029]/u.test(value);
}

export function validCapabilityContent(value) {
  return typeof value === 'string' && value.trim().length > 0 && utf8(value) <= MAX_CONTENT_BYTES && !value.includes('\0');
}

function review(value) {
  return value === null || (hasOnly(value, new Set(['reviewer', 'reviewedAtMs']))
    && value.reviewer === 'local_operator' && time(value.reviewedAtMs));
}

function revision(value, content = true) {
  const keys = new Set(['id', 'parentRevisionId', 'title', 'sha256', 'createdAtMs', 'review']);
  if (content) keys.add('content');
  return hasOnly(value, keys) && id(value.id, 'revision:') && (value.parentRevisionId === null || id(value.parentRevisionId, 'revision:'))
    && validCapabilityTitle(value.title) && hex(value.sha256) && time(value.createdAtMs) && review(value.review)
    && (!content || validCapabilityContent(value.content));
}

export function validateCapabilitySummary(value) {
  return hasOnly(value, new Set(['id', 'sourceId', 'fragmentId', 'latestRevisionId', 'title', 'reviewed', 'revisionCount']))
    && id(value.id, 'capability:') && id(value.sourceId, 'source:') && id(value.fragmentId, 'fragment:') && id(value.latestRevisionId, 'revision:')
    && validCapabilityTitle(value.title) && typeof value.reviewed === 'boolean'
    && Number.isInteger(value.revisionCount) && value.revisionCount > 0 && value.revisionCount <= 32;
}

export function validateCapabilityDetail(value) {
  if (!hasOnly(value, new Set(['schemaVersion', 'id', 'sourceId', 'fragmentId', 'sourceName', 'span', 'originalText', 'latestRevisionId', 'revision', 'history', 'authority']))
    || value.schemaVersion !== CAPABILITY_SCHEMA || !id(value.id, 'capability:') || !id(value.sourceId, 'source:') || !id(value.fragmentId, 'fragment:')
    || typeof value.sourceName !== 'string' || typeof value.originalText !== 'string' || !id(value.latestRevisionId, 'revision:')
    || value.authority !== 'none' || !object(value.span) || !Array.isArray(value.history) || !revision(value.revision)) return null;
  if (!hasOnly(value.span, new Set(['startByte', 'endByte', 'startLine', 'endLine']))
    || !['startByte', 'endByte', 'startLine', 'endLine'].every(key => Number.isSafeInteger(value.span[key])) || value.span.startByte < 0 || value.span.endByte < value.span.startByte
    || value.span.endByte > MAX_CONTENT_BYTES || value.span.endLine > 10_000 || value.span.startLine < 1 || value.span.endLine < value.span.startLine || utf8(value.originalText) !== value.span.endByte - value.span.startByte) return null;
  const seen = new Set();
  if (!value.history.length || value.history.length > 32 || !value.history.every(item => revision(item, false) && !seen.has(item.id) && (seen.add(item.id), true))) return null;
  if (!seen.has(value.revision.id) || value.history.at(-1)?.id !== value.latestRevisionId) return null;
  if (!value.history.every((item, index) => item.parentRevisionId === (index ? value.history[index - 1].id : null))) return null;
  const summary = value.history.find(item => item.id === value.revision.id);
  if (!summary || ['id', 'parentRevisionId', 'title', 'sha256', 'createdAtMs'].some(key => summary[key] !== value.revision[key]) || (summary.review?.reviewer !== value.revision.review?.reviewer || summary.review?.reviewedAtMs !== value.revision.review?.reviewedAtMs)) return null;
  return value;
}

export const EMPTY_SKILLS_STATE = Object.freeze({
  bridgeAvailable: false, listStatus: 'idle', capabilities: [], selected: null, viewedRevisionId: null,
  draft: null, pending: null, error: null, needsReload: false, source: null, createTitle: '', requestId: 0,
  message: 'Open the desktop app to use local skills.',
});

export function createSkillsController({ invoke, onChange = () => {} } = {}) {
  const bridgeAvailable = typeof invoke === 'function';
  let state = { ...EMPTY_SKILLS_STATE, bridgeAvailable, listStatus: bridgeAvailable ? 'idle' : 'unavailable', message: bridgeAvailable ? 'Create a skill from a saved section, or open one from your library.' : EMPTY_SKILLS_STATE.message };
  let actionEpoch = 0;
  let openingCapabilityId = null;
  let listEpoch = 0;
  const drafts = new Map();
  const creationTitles = new Map();
  const sourceKey = source => source ? `${source.sourceId}/${source.fragmentId}` : null;
  const publish = () => onChange({ ...state, capabilities: [...state.capabilities], selected: state.selected ? structuredClone(state.selected) : null, draft: state.draft ? { ...state.draft } : null });
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const unavailable = () => !bridgeAvailable;
  const canMutate = () => !unavailable() && !state.pending && !state.needsReload;
  const selectDraft = detail => {
    const prior = drafts.get(detail.id);
    state.draft = prior ? { ...prior } : { capabilityId: detail.id, baseRevisionId: detail.latestRevisionId, title: detail.revision.title, content: detail.revision.content, dirty: false };
  };
  async function call(command, args) {
    try { return await invoke(command, args); } catch { return failed(); }
  }
  function acceptOpened(result, action) {
    if (result?.outcome !== 'opened') return null;
    const detail = validateCapabilityDetail(result.capability);
    if (!detail || typeof result.alreadyApplied !== 'boolean') return null;
    return { detail, alreadyApplied: result.alreadyApplied, action };
  }
  const controller = {
    hasUnsavedDraft: capabilityId => Boolean(drafts.get(capabilityId)?.dirty || (state.draft?.capabilityId === capabilityId && state.draft.dirty)),
    forgetDeleted(capabilityId) {
      drafts.delete(capabilityId);
      ++listEpoch;
      const changes = { capabilities: state.capabilities.filter(item => item.id !== capabilityId) };
      if (openingCapabilityId === capabilityId) {
        ++actionEpoch;
        openingCapabilityId = null;
        Object.assign(changes, { pending: null, error: null, message: 'The skill being opened was removed.' });
      }
      if (state.selected?.id === capabilityId) {
        ++actionEpoch;
        openingCapabilityId = null;
        Object.assign(changes, { selected: null, viewedRevisionId: null, draft: null, pending: null, error: null, needsReload: false, message: 'Saved skill and its local revision history were removed.' });
      }
      set(changes);
    },
    getState: () => ({ ...state, capabilities: [...state.capabilities], selected: state.selected ? structuredClone(state.selected) : null, draft: state.draft ? { ...state.draft } : null }),
    setSourceContext({ report = null, snapshots = [], snapshotsStatus = 'idle' } = {}) {
      const sourceId = typeof report?.source?.id === 'string' ? report.source.id : null;
      const fragment = report?.fragments?.find(item => item.id === report?.selectedFragmentId) ?? report?.fragments?.[0] ?? null;
      const saved = sourceId && snapshotsStatus === 'ready' && snapshots.some(item => item?.sourceId === sourceId);
      const source = id(sourceId, 'source:') && id(fragment?.id, 'fragment:') ? { sourceId, fragmentId: fragment.id, saved: Boolean(saved), title: fragment.heading?.title ?? 'Preamble', span: fragment.span ?? null } : null;
      set({ source, createTitle: creationTitles.get(sourceKey(source)) ?? source?.title ?? '' });
    },
    updateCreateTitle(title) {
      if (!state.source || state.pending || typeof title !== 'string') return;
      creationTitles.set(sourceKey(state.source), title);
      state = { ...state, createTitle: title };
    },
    async list() {
      if (unavailable()) { set({ listStatus: 'unavailable', capabilities: [], error: null }); return; }
      const localRequest = ++listEpoch;
      set({ listStatus: 'loading' });
      const result = await call('list_capabilities');
      if (localRequest !== listEpoch) return;
      const unique = new Set(result?.capabilities?.map?.(item => item?.id));
      if (result?.outcome === 'listed' && Array.isArray(result.capabilities) && result.capabilities.length <= 128 && unique.size === result.capabilities.length && result.capabilities.every(validateCapabilitySummary)) {
        set({ listStatus: 'ready', capabilities: result.capabilities });
      } else {
        set({ listStatus: 'failed' });
      }
    },
    updateDraft(changes) {
      if (!state.draft || state.pending || state.viewedRevisionId !== state.selected?.latestRevisionId) return;
      const draft = { ...state.draft, ...changes };
      draft.dirty = draft.title !== state.selected.revision.title || draft.content !== state.selected.revision.content;
      drafts.set(draft.capabilityId, draft);
      state = { ...state, draft, error: null };
    },
    async open(capabilityId, revisionId = null, { preserveDraft = null } = {}) {
      if (unavailable() || state.pending || !id(capabilityId, 'capability:') || (revisionId !== null && !id(revisionId, 'revision:'))) return;
      const localRequest = ++actionEpoch;
      openingCapabilityId = capabilityId;
      set({ pending: 'open', error: null, message: 'Opening local capability…' });
      const result = await call('open_capability', { capabilityId, revisionId });
      if (localRequest !== actionEpoch) return;
      openingCapabilityId = null;
      const opened = acceptOpened(result, 'open');
      if (!opened || opened.detail.id !== capabilityId || opened.detail.revision.id !== (revisionId ?? opened.detail.latestRevisionId)) { set({ pending: null, error: failure(result), message: failure(result).message }); return; }
      const latest = opened.detail.latestRevisionId;
      const historical = opened.detail.revision.id !== latest;
      if (!historical && preserveDraft) {
        const draft = { ...preserveDraft, baseRevisionId: opened.detail.latestRevisionId };
        draft.dirty = draft.title !== opened.detail.revision.title || draft.content !== opened.detail.revision.content;
        drafts.set(opened.detail.id, draft);
        state.draft = draft;
      } else if (!historical) selectDraft(opened.detail);
      const staleDraft = !historical && state.draft?.baseRevisionId !== latest;
      set({ selected: opened.detail, viewedRevisionId: opened.detail.revision.id, pending: null, needsReload: staleDraft, error: null, message: staleDraft ? 'A newer revision is available. Reload current to compare your retained draft before saving.' : historical ? 'Historical revision open. Return to current before saving or reviewing.' : preserveDraft ? 'Current revision reloaded. Draft retained; compare the saved revision before saving.' : 'Current skill open.' });
    },
    async reloadCurrent() {
      if (unavailable() || state.pending || !state.selected) return;
      const preserveDraft = state.draft ? { ...state.draft } : null;
      return controller.open(state.selected.id, null, { preserveDraft });
    },
    async create(title) {
      if (!canMutate() || !state.source?.saved || !validCapabilityTitle(title)) return;
      const source = state.source;
      const localRequest = ++actionEpoch;
      set({ pending: 'create', error: null, message: 'Creating local capability draft…' });
      const result = await call('create_capability', { sourceId: source.sourceId, fragmentId: source.fragmentId, title });
      if (localRequest !== actionEpoch) return;
      const opened = acceptOpened(result, 'create');
      if (!opened || opened.detail.sourceId !== source.sourceId || opened.detail.fragmentId !== source.fragmentId) { set({ pending: null, error: failure(result), message: failure(result).message }); return; }
      selectDraft(opened.detail);
      set({ selected: opened.detail, viewedRevisionId: opened.detail.latestRevisionId, pending: null, needsReload: false, error: null, message: opened.alreadyApplied ? 'Existing matching skill opened.' : 'Draft capability created from the saved source section.' });
      void controller.list();
    },
    async save() {
      const draft = state.draft;
      if (!canMutate() || !state.selected || !draft || !draft.dirty || state.viewedRevisionId !== state.selected.latestRevisionId || !validCapabilityTitle(draft.title) || !validCapabilityContent(draft.content)) return;
      const localRequest = ++actionEpoch;
      set({ pending: 'save', error: null, message: 'Saving local revision…' });
      const result = await call('revise_capability', { capabilityId: state.selected.id, expectedRevisionId: draft.baseRevisionId, title: draft.title, content: draft.content });
      if (localRequest !== actionEpoch) return;
      const opened = acceptOpened(result, 'save');
      if (!opened || opened.detail.id !== state.selected.id || opened.detail.revision.id !== opened.detail.latestRevisionId || opened.detail.revision.title !== draft.title || opened.detail.revision.content !== draft.content) {
        const issue = failure(result);
        set({ pending: null, error: issue, needsReload: issue.code === 'capability_conflict', message: issue.code === 'capability_conflict' ? 'Revision conflict. Reload current capability before another save or review.' : issue.message });
        return;
      }
      state.draft = { capabilityId: opened.detail.id, baseRevisionId: opened.detail.latestRevisionId, title: opened.detail.revision.title, content: opened.detail.revision.content, dirty: false };
      drafts.set(opened.detail.id, state.draft);
      set({ selected: opened.detail, viewedRevisionId: opened.detail.latestRevisionId, pending: null, error: null, needsReload: false, message: opened.alreadyApplied ? 'Matching revision already saved.' : 'Revision saved. Local review is still required.' });
      void controller.list();
    },
    async review() {
      const draft = state.draft;
      if (!canMutate() || !state.selected || !draft || draft.dirty || state.viewedRevisionId !== state.selected.latestRevisionId || state.selected.revision.review || !validCapabilityTitle(draft.title) || !validCapabilityContent(draft.content)) return;
      const localRequest = ++actionEpoch;
      set({ pending: 'review', error: null, message: 'Recording local content review…' });
      const result = await call('review_capability', { capabilityId: state.selected.id, expectedRevisionId: state.selected.latestRevisionId });
      if (localRequest !== actionEpoch) return;
      const opened = acceptOpened(result, 'review');
      if (!opened || opened.detail.id !== state.selected.id || opened.detail.latestRevisionId !== state.selected.latestRevisionId || opened.detail.revision.id !== state.selected.latestRevisionId || !opened.detail.revision.review) {
        const issue = failure(result);
        set({ pending: null, error: issue, needsReload: issue.code === 'capability_conflict', message: issue.code === 'capability_conflict' ? 'Revision conflict. Reload current capability before another save or review.' : issue.message });
        return;
      }
      selectDraft(opened.detail);
      set({ selected: opened.detail, viewedRevisionId: opened.detail.latestRevisionId, pending: null, error: null, needsReload: false, message: opened.alreadyApplied ? 'This revision was already locally reviewed.' : 'Local content review recorded. Authority remains none.' });
      void controller.list();
    },
  };
  // The host starts list loading after controller bindings exist.

  return controller;
}

export const safeText = escapeText;
