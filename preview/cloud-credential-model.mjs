const SCHEMA = 'rangoon.cloud-custody.v1';
const OUTCOMES = new Set(['stored', 'missing', 'saved', 'removed', 'cancelled', 'failed']);
const BACKENDS = new Set(['macos_keychain', 'windows_credential_manager', 'linux_secret_service']);
const CODES = new Set(['invalid_request', 'busy', 'unavailable', 'invalid_stored_credential', 'changed', 'write_uncertain', 'confirmation_unavailable']);
const DIGEST = /^[0-9a-f]{64}$/;
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));

export function validateCloudCustody(value) {
  const keys = ['schemaVersion', 'provider', 'authority', 'backend', 'outcome'];
  if (!object(value) || !keys.every(key => Object.hasOwn(value, key))) return null;
  if (value.outcome === 'stored' || value.outcome === 'saved') keys.push('revision');
  if (value.outcome === 'failed') keys.push('code');
  if (!only(value, keys) || value.schemaVersion !== SCHEMA || value.provider !== 'openai' || value.authority !== 'none'
    || !BACKENDS.has(value.backend) || !OUTCOMES.has(value.outcome)) return null;
  if ((value.outcome === 'stored' || value.outcome === 'saved') && !DIGEST.test(value.revision)) return null;
  if (value.outcome === 'failed' && !CODES.has(value.code)) return null;
  return structuredClone(value);
}

const initial = available => ({
  bridgeAvailable: available,
  status: 'unknown',
  needsInspection: false,
  backend: null,
  revision: null,
  pending: null,
  outcome: null,
  error: null,
  message: available ? 'Credential custody has not been inspected.' : 'Native credential custody is unavailable in browser preview.',
});

export function createCloudCredentialController({ invoke, onChange = () => {} } = {}) {
  const available = typeof invoke === 'function';
  let state = initial(available);
  const publish = next => { state = next; onChange(state); };
  const failClosed = (pending, message, code = 'unavailable') => publish({
    ...state, status: 'unknown', backend: null, revision: null, pending: null, outcome: 'failed',
    error: { code }, message, needsInspection: true,
  });
  const apply = (result, pending) => {
    if (result.outcome === 'stored' || result.outcome === 'saved') {
      publish({ ...state, status: 'stored', needsInspection: pending === 'inspect' ? false : state.needsInspection, backend: result.backend, revision: result.revision, pending: null, outcome: result.outcome, error: null, message: pending === 'inspect' ? 'OpenAI credential is stored in native OS custody.' : 'Credential operation completed. Cloud transport remains unavailable.' });
    } else if (result.outcome === 'missing' || result.outcome === 'removed') {
      publish({ ...state, status: 'missing', needsInspection: pending === 'inspect' ? false : state.needsInspection, backend: result.backend, revision: null, pending: null, outcome: result.outcome, error: null, message: result.outcome === 'removed' ? 'OpenAI credential removed from native OS custody.' : 'No OpenAI credential is stored.' });
    } else if (result.outcome === 'cancelled') {
      publish({ ...state, pending: null, outcome: 'cancelled', error: null, message: 'Credential operation cancelled. Previous custody state remains unverified.' });
    } else {
      failClosed(pending, 'Credential operation failed. Inspect again before relying on custody state.', result.code);
    }
  };
  const run = async (pending, command) => {
    if (!available) { failClosed(pending, 'Native credential custody is unavailable in browser preview. No provider check or send was made.'); return false; }
    if (state.pending || (pending !== 'inspect' && state.needsInspection)) return false;
    if (pending === 'remove' && state.status !== 'stored') return false;
    publish({ ...state, pending, outcome: null, error: null, message: pending === 'inspect' ? 'Inspecting native credential custody…' : pending === 'remove' ? 'Waiting for native remove decision…' : 'Opening isolated credential entry…' });
    let result;
    try { result = await invoke(command); } catch { failClosed(pending, 'Credential operation was unavailable. Inspect again before relying on custody state.'); return false; }
    const valid = validateCloudCustody(result);
    if (!valid) { failClosed(pending, 'Credential operation returned an unrecognized result. Inspect again before relying on custody state.', 'invalid_request'); return false; }
    apply(valid, pending);
    return valid.outcome !== 'failed';
  };
  return {
    getState: () => state,
    inspect: () => run('inspect', 'inspect_cloud_credential'),
    edit: () => run('edit', 'edit_cloud_credential'),
    remove: () => run('remove', 'remove_cloud_credential'),
  };
}
