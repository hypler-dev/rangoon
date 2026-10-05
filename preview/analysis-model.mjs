export const EMPTY_STATE = Object.freeze({
  status: 'idle',
  report: null,
  selectedFragmentId: null,
  errorCode: null,
  message: 'Choose one Markdown file from the desktop picker. Rangoon analyzes it in memory.',
  busyAction: null,
  snapshots: [],
  snapshotsStatus: 'idle',
  snapshotsError: null,
  requestId: 0,
});

export function escapeText(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function resultMessage(result) {
  const message = typeof result?.error?.message === 'string' ? result.error.message : 'The desktop analysis could not finish.';
  return message;
}

function resultCode(result) {
  return typeof result?.error?.code === 'string' ? result.error.code : null;
}

function boundedSnapshots(snapshots) {
  const sourceIds = new Set();
  return snapshots.filter(snapshot => {
    const sourceId = typeof snapshot?.sourceId === 'string' ? snapshot.sourceId : null;
    if (!sourceId || sourceIds.has(sourceId)) return false;
    sourceIds.add(sourceId);
    return true;
  }).slice(0, 128);
}

export function createAnalysisController({ invoke, onChange = () => {} } = {}) {
  let state = { ...EMPTY_STATE, status: typeof invoke === 'function' ? 'idle' : 'unavailable' };
  let snapshotRequestId = 0;

  const publish = () => onChange({ ...state });
  const setState = changes => {
    state = { ...state, ...changes };
    publish();
  };

  const unavailable = () => typeof invoke !== 'function';
  const failedResult = () => ({ outcome: 'failed', error: { message: 'The desktop analysis could not finish.' } });
  const canStart = action => !unavailable() && !state.busyAction && action;

  const controller = {
    getState: () => ({ ...state }),
    async listSnapshots() {
      if (unavailable()) {
        setState({ snapshotsStatus: 'unavailable', snapshots: [], snapshotsError: null });
        return;
      }
      const requestId = ++snapshotRequestId;
      setState({ snapshotsStatus: 'loading', snapshotsError: null });
      let result;
      try {
        result = await invoke('list_snapshots');
      } catch {
        result = failedResult();
      }
      if (requestId !== snapshotRequestId) return;
      if (result?.outcome === 'listed' && Array.isArray(result.snapshots)) {
        setState({ snapshotsStatus: 'ready', snapshots: boundedSnapshots(result.snapshots), snapshotsError: null });
      } else {
        setState({
          snapshotsStatus: 'failed',
          snapshotsError: { code: resultCode(result), message: resultMessage(result) },
        });
      }
    },
    selectFragment(id) {
      if (state.report?.fragments?.some(fragment => fragment.id === id)) setState({ selectedFragmentId: id });
    },
    async choose() {
      if (!canStart('choose')) return;
      const requestId = state.requestId + 1;
      setState({ status: 'pending', busyAction: 'choose', requestId, errorCode: null, message: 'Waiting for selection and analysis…' });
      let result;
      try {
        result = await invoke('select_and_analyze');
      } catch {
        result = failedResult();
      }
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'analyzed' && result.report) {
        setState({
          status: 'analyzed',
          busyAction: null,
          report: result.report,
          selectedFragmentId: result.report.fragments?.[0]?.id ?? null,
          errorCode: null,
          message: 'Analysis ready. Save locally to keep this source after restart.',
        });
      } else if (result?.outcome === 'cancelled') {
        setState({ status: 'cancelled', busyAction: null, errorCode: null, message: state.report ? 'No file selected. Current analysis remains available.' : 'No file selected.' });
      } else if (result?.outcome === 'rejected') {
        setState({ status: 'rejected', busyAction: null, errorCode: resultCode(result), message: resultMessage(result) });
      } else {
        setState({ status: 'failed', busyAction: null, errorCode: resultCode(result), message: resultMessage(result) });
      }
    },
    async save() {
      const sourceId = state.report?.source?.id;
      if (!sourceId || !canStart('save')) return;
      const requestId = state.requestId + 1;
      setState({ status: 'pending', busyAction: 'save', requestId, errorCode: null, message: 'Saving this source on this computer…' });
      let result;
      try {
        result = await invoke('save_analysis', { sourceId });
      } catch {
        result = failedResult();
      }
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'saved' && result.snapshot) {
        setState({
          status: 'analyzed',
          busyAction: null,
          errorCode: null,
          message: result.alreadySaved ? 'Already saved locally.' : 'Saved locally on this computer.',
        });
        await controller.listSnapshots();
      } else {
        setState({ status: 'failed', busyAction: null, errorCode: resultCode(result), message: resultMessage(result) });
      }
    },
    async openSnapshot(sourceId) {
      if (!sourceId || !canStart('open')) return;
      const requestId = state.requestId + 1;
      setState({ status: 'pending', busyAction: 'open', requestId, errorCode: null, message: 'Opening saved source…' });
      let result;
      try {
        result = await invoke('open_snapshot', { sourceId });
      } catch {
        result = failedResult();
      }
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'analyzed' && result.report) {
        setState({
          status: 'analyzed',
          busyAction: null,
          report: result.report,
          selectedFragmentId: result.report.fragments?.[0]?.id ?? null,
          errorCode: null,
          message: 'Saved source opened. All sections remain unreviewed.',
        });
      } else {
        setState({ status: 'failed', busyAction: null, errorCode: resultCode(result), message: resultMessage(result) });
      }
    },
    async clear() {
      if (unavailable() || state.busyAction === 'save' || state.busyAction === 'open' || state.busyAction === 'clear') return;
      const requestId = state.requestId + 1;
      setState({ status: 'pending', busyAction: 'clear', requestId, errorCode: null, message: 'Clearing current analysis…' });
      let result;
      try {
        result = await invoke('clear_analysis');
      } catch {
        result = failedResult();
      }
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'cleared') {
        setState({
          ...EMPTY_STATE,
          status: 'idle',
          snapshots: state.snapshots,
          snapshotsStatus: state.snapshotsStatus,
          snapshotsError: state.snapshotsError,
          requestId,
          message: 'Current analysis cleared. Saved sources remain available.',
        });
      } else {
        setState({ status: 'failed', busyAction: null, errorCode: resultCode(result), message: resultMessage(result) });
      }
    },
  };

  if (unavailable()) controller.listSnapshots();
  else void controller.listSnapshots();
  return controller;
}
