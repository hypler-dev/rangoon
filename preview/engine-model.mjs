export const ENGINE_OPERATIONS = Object.freeze([
  'negotiate_contract',
  'inspect_configuration',
  'read_evidence',
  'submit_operation',
  'reconcile_operation',
]);

const STATUS_SCHEMA = 'rangoon.engine-status.v0';
const UNAVAILABLE_REASON = 'adapter_not_implemented';
const STATUS_KEYS = new Set([
  'schemaVersion', 'provider', 'adapterState', 'connectionState', 'runtimeState',
  'installedVersion', 'observedContract', 'executionAuthority', 'networkAttempted',
  'reference', 'capabilities',
]);
const REFERENCE_KEYS = new Set([
  'repository', 'sourceCommit', 'productVersion', 'wireContract',
  'defaultProductSurface', 'configurationProductSurface', 'releaseQualified',
]);

const hasOnlyKeys = (value, keys) => Object.keys(value).every(key => keys.has(key));
const isObject = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);

export function validateEngineStatus(value) {
  if (!isObject(value) || !hasOnlyKeys(value, STATUS_KEYS)) return null;
  if (value.schemaVersion !== STATUS_SCHEMA || value.provider !== 'lnsat' || value.adapterState !== 'placeholder'
    || value.connectionState !== 'not_attempted' || value.runtimeState !== 'not_checked'
    || value.installedVersion !== null || value.observedContract !== null
    || value.executionAuthority !== 'none' || value.networkAttempted !== false) return null;

  const reference = value.reference;
  if (!isObject(reference) || !hasOnlyKeys(reference, REFERENCE_KEYS)
    || reference.repository !== 'https://github.com/hypler-dev/LNSAT'
    || reference.sourceCommit !== 'e09a6b02634b04a46f861ed8b092acc2c2e50fe8'
    || reference.productVersion !== '0.1.0'
    || reference.wireContract !== 'lnsat.contracts.v1_0'
    || reference.defaultProductSurface !== 'lnsat.product_surface.v1'
    || reference.configurationProductSurface !== 'lnsat.product_surface.v2'
    || reference.releaseQualified !== false) return null;

  if (!Array.isArray(value.capabilities) || value.capabilities.length !== ENGINE_OPERATIONS.length) return null;
  const operations = new Set();
  for (const capability of value.capabilities) {
    if (!isObject(capability) || !hasOnlyKeys(capability, new Set(['operation', 'state', 'reason']))
      || !ENGINE_OPERATIONS.includes(capability.operation) || operations.has(capability.operation)
      || capability.state !== 'unavailable' || capability.reason !== UNAVAILABLE_REASON) return null;
    operations.add(capability.operation);
  }
  return value;
}

export const EMPTY_ENGINE_STATE = Object.freeze({
  bridgeAvailable: false,
  status: 'idle',
  result: null,
  message: 'Check integration status to read Rangoon’s local placeholder diagnostic.',
  requestId: 0,
});

export function createEngineController({ invoke, onChange = () => {} } = {}) {
  const bridgeAvailable = typeof invoke === 'function';
  let state = bridgeAvailable ? { ...EMPTY_ENGINE_STATE, bridgeAvailable } : {
    ...EMPTY_ENGINE_STATE,
    bridgeAvailable,
    status: 'unavailable',
    message: 'Desktop bridge unavailable. Installation and runtime remain unknown.',
  };
  const publish = () => onChange({ ...state });
  const setState = changes => { state = { ...state, ...changes }; publish(); };
  const unavailable = () => !bridgeAvailable;

  return {
    getState: () => ({ ...state }),
    async check() {
      const requestId = state.requestId + 1;
      if (unavailable()) {
        setState({ status: 'unavailable', result: null, requestId, message: 'Desktop bridge unavailable. Installation and runtime remain unknown.' });
        return;
      }
      setState({ status: 'loading', result: null, requestId, message: 'Reading local integration status…' });
      let result;
      try {
        result = await invoke('get_engine_status');
      } catch {
        result = null;
      }
      if (state.requestId !== requestId) return;
      const validated = validateEngineStatus(result);
      if (validated) {
        setState({ status: 'unavailable', result: validated, message: 'Local placeholder diagnostic read. All engine capabilities remain unavailable.' });
      } else {
        setState({ status: 'failed', result: null, message: 'Integration status was not accepted. Engine capabilities remain unavailable.' });
      }
    },
  };
}
