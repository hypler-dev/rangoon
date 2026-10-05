export const EMPTY_STATE = Object.freeze({
  status: 'idle',
  report: null,
  selectedFragmentId: null,
  errorCode: null,
  message: 'Choose one Markdown file from the desktop picker. Rangoon analyzes it in memory.',
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

export function createAnalysisController({ invoke, onChange = () => {} } = {}) {
  let state = { ...EMPTY_STATE, status: typeof invoke === 'function' ? 'idle' : 'unavailable' };

  const publish = () => onChange({ ...state });
  const setState = changes => {
    state = { ...state, ...changes };
    publish();
  };

  return {
    getState: () => ({ ...state }),
    clear() {
      setState({ ...EMPTY_STATE, status: typeof invoke === 'function' ? 'idle' : 'unavailable', requestId: state.requestId + 1 });
    },
    selectFragment(id) {
      if (state.report?.fragments?.some(fragment => fragment.id === id)) setState({ selectedFragmentId: id });
    },
    async choose() {
      if (typeof invoke !== 'function' || state.status === 'pending') return;
      const requestId = state.requestId + 1;
      setState({ status: 'pending', requestId, errorCode: null, message: 'Waiting for selection and analysis…' });
      let result;
      try {
        result = await invoke('select_and_analyze');
      } catch {
        result = { outcome: 'failed', error: { message: 'The desktop analysis could not finish.' } };
      }
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'analyzed' && result.report) {
        setState({
          status: 'analyzed',
          report: result.report,
          selectedFragmentId: result.report.fragments?.[0]?.id ?? null,
          errorCode: null,
          message: 'Analysis ready. Source remains in this window only.',
        });
      } else if (result?.outcome === 'cancelled') {
        setState({ status: 'cancelled', errorCode: null, message: state.report ? 'No file selected. Current analysis remains available.' : 'No file selected.' });
      } else if (result?.outcome === 'rejected') {
        setState({ status: 'rejected', errorCode: resultCode(result), message: resultMessage(result) });
      } else {
        setState({ status: 'failed', errorCode: resultCode(result), message: resultMessage(result) });
      }
    },
  };
}
