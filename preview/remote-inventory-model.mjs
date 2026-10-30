// Internal RM-1 offline projection. No transport, filesystem, storage or authority.
export const INVENTORY_LIMITS = Object.freeze({ page: 50, pages: 64, records: 1000, payloadBytes: 131072, maxAgeMs: 300000 });
const VERSION = 'rangoon.remote-inventory.offline.v1';
const OP = 'inventory.metadata';
const BINDING_KEYS = ['installationId', 'installationEpoch', 'principalId', 'grantId', 'rootId'];
const messages = Object.freeze({
  invalid: 'The inventory report is not compatible with this offline profile.',
  unsupported: 'This offline reader supports metadata inventory only.',
  scope_mismatch: 'The report belongs to another system, root or session.',
  stale_request: 'This request is no longer current. Its result was ignored.',
  cursor_conflict: 'The inventory page does not continue the current snapshot.',
  snapshot_conflict: 'The snapshot coverage changed between pages. Start a new scope.',
  duplicate: 'The report repeats a file identity or path. Previous pages remain available.',
  expired: 'The reported grant or metadata consent has expired. Select a new scope.',
  clock_changed: 'The local clock moved backwards. Select the scope again.',
  capacity: 'This view reached its bounded record or page limit. Narrow the inventory scope.',
  busy: 'Finish or cancel the current metadata request first.',
  unavailable: 'Select an offline inventory scope first.',
  complete: 'There are no more pages in this reported inventory.',
});
function fail(code) { return { ok: false, error: { code, message: messages[code] } }; }
function exact(value, keys) { return value !== null && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)); }
const id = value => typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/.test(value);
const time = value => Number.isSafeInteger(value) && value >= 0 && value <= 8_640_000_000_000_000;
const optionalTime = value => value === null || time(value);
const text = (value, limit) => typeof value === 'string' && value.length > 0 && value.length <= limit && !/[\p{Cc}\p{Cf}\p{Cs}]/u.test(value);
const sameBinding = (a, b) => BINDING_KEYS.every(key => a[key] === b[key]);
function uniqueJsonKeys(json) {
  let pos = 0;
  const space = () => { while (/\s/.test(json[pos] ?? '') && pos < json.length) pos++; };
  function string() {
    const start = pos++; while (pos < json.length) {
      const c = json[pos++]; if (c === '\\') { pos++; continue; }
      if (c === '"') return JSON.parse(json.slice(start, pos));
    }
    throw new Error('unterminated');
  }
  function value(depth) {
    if (depth > 16) throw new Error('depth');
    space(); const c = json[pos];
    if (c === '"') { string(); return; }
    if (c === '{' || c === '[') {
      pos++; space(); const end = c === '{' ? '}' : ']', keys = new Set();
      if (json[pos] === end) { pos++; return; }
      while (pos < json.length) {
        if (c === '{') {
          space(); if (json[pos] !== '"') throw new Error('key');
          const key = string(); if (keys.has(key)) throw new Error('duplicate'); keys.add(key);
          space(); if (json[pos++] !== ':') throw new Error('colon');
        }
        value(depth + 1); space(); const separator = json[pos++];
        if (separator === end) return;
        if (separator !== ',') throw new Error('separator');
      }
      throw new Error('unterminated');
    }
    const primitive = /^(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/.exec(json.slice(pos));
    if (!primitive) throw new Error('primitive'); pos += primitive[0].length;
  }
  value(0); space(); if (pos !== json.length) throw new Error('trailing');
}
function decode(json) {
  if (typeof json !== 'string' || json.length > INVENTORY_LIMITS.payloadBytes || new TextEncoder().encode(json).length > INVENTORY_LIMITS.payloadBytes) return null;
  try { uniqueJsonKeys(json); return JSON.parse(json); } catch { return null; }
}
function binding(value) { return exact(value, BINDING_KEYS) && BINDING_KEYS.every(key => id(value[key])); }
const operations = value => Array.isArray(value) && value.length >= 1 && value.length <= 8 && value.every(id) && new Set(value).size === value.length;
const sameOperations = (a, b) => a.length === b.length && a.every(op => b.includes(op));
const adapterValue = value => exact(value, ['id', 'product', 'version', 'operations']) && id(value.id) && text(value.product, 80) && id(value.version) && operations(value.operations) && value.operations.includes(OP);
const sameAdapter = (a, b) => a.id === b.id && a.product === b.product && a.version === b.version && sameOperations(a.operations, b.operations);
const accessValue = value => exact(value, ['grantOperations', 'grantExpiresAtMs', 'consentId', 'consentOperation', 'consentExpiresAtMs']) && operations(value.grantOperations) && value.grantOperations.includes(OP) && time(value.grantExpiresAtMs) && id(value.consentId) && value.consentOperation === OP && time(value.consentExpiresAtMs);
const sameAccess = (a, b) => a.grantExpiresAtMs === b.grantExpiresAtMs && a.consentId === b.consentId && a.consentOperation === b.consentOperation && a.consentExpiresAtMs === b.consentExpiresAtMs && sameOperations(a.grantOperations, b.grantOperations);
function scopeValue(value) {
  return exact(value, ['version', 'adapter', 'binding', 'access', 'sessionId', 'snapshotId']) && value.version === VERSION && adapterValue(value.adapter) &&
    binding(value.binding) && accessValue(value.access) && id(value.sessionId) && id(value.snapshotId);
}
function relativeMarkdown(value) {
  if (!text(value, 1024) || /[\\:%?#]/.test(value) || !/\.md$/i.test(value)) return false;
  return value.split('/').every(part => part && part !== '.' && part !== '..' && part.length <= 255 && !/[. ]$/.test(part) && !/^(?:\.git|\.ssh|\.aws|node_modules)$/i.test(part) && !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part));
}
function entry(value) {
  return exact(value, ['fileId', 'relativePath', 'byteLength', 'modifiedAtMs', 'sha256', 'revisionId']) && id(value.fileId) && relativeMarkdown(value.relativePath) &&
    Number.isSafeInteger(value.byteLength) && value.byteLength >= 0 && optionalTime(value.modifiedAtMs) && (value.sha256 === null || typeof value.sha256 === 'string' && /^[a-f0-9]{64}$/.test(value.sha256)) && (value.revisionId === null || id(value.revisionId));
}
function pageValue(value) {
  return exact(value, ['version', 'adapter', 'binding', 'access', 'sessionId', 'snapshotId', 'requestId', 'afterCursor', 'nextCursor', 'observedAtMs', 'coverage', 'items']) && value.version === VERSION && adapterValue(value.adapter) && binding(value.binding) && accessValue(value.access) && id(value.sessionId) && id(value.snapshotId) &&
    Number.isSafeInteger(value.requestId) && value.requestId > 0 && (value.afterCursor === null || id(value.afterCursor)) && (value.nextCursor === null || id(value.nextCursor)) && optionalTime(value.observedAtMs) &&
    ['complete', 'partial', 'unknown'].includes(value.coverage) && Array.isArray(value.items) && value.items.length <= INVENTORY_LIMITS.page && value.items.every(entry);
}

/** Data-only, in-memory inspection of unverified reports; never authentication. */
export function createRemoteInventoryModel({ now = Date.now } = {}) {
  let scope = null, pending = null, rows = [], pages = [], cursor = null, finished = false;
  let generation = 0, serial = 0, lastNow = null, disposed = false, status = 'unconfigured', error = null;
  const cursors = new Set();
  function clear(nextStatus) { generation++; scope = null; pending = null; rows = []; pages = []; cursors.clear(); cursor = null; finished = false; status = nextStatus; error = null; }
  function clock() {
    const value = now();
    if (!time(value) || lastNow !== null && value < lastNow) { clear('clock_changed'); error = fail('clock_changed').error; lastNow = time(value) ? value : null; return null; }
    lastNow = value; return value;
  }
  function validScope() {
    if (disposed) return fail('unavailable');
    const current = clock(); if (current === null) return fail('clock_changed');
    if (!scope) return fail('unavailable');
    if (current >= scope.access.grantExpiresAtMs || current >= scope.access.consentExpiresAtMs) { clear('expired'); error = fail('expired').error; return fail('expired'); }
    return { ok: true, current };
  }
  function freshness(observed, received, current) {
    if (observed === null) return 'unknown';
    if (observed > received || observed > current) return 'clock_skew';
    // Receiver age is independent of an untrusted producer clock.
    return current - received > INVENTORY_LIMITS.maxAgeMs || current - observed > INVENTORY_LIMITS.maxAgeMs ? 'stale' : 'recent_report';
  }
  return {
    selectScope(json) {
      if (disposed) return fail('unavailable');
      const selected = decode(json); if (!scopeValue(selected)) return fail('invalid');
      const current = clock(); if (current === null) return fail('clock_changed');
      if (current >= selected.access.grantExpiresAtMs || current >= selected.access.consentExpiresAtMs) return fail('expired');
      clear('offline'); scope = selected;
      return { ok: true };
    },
    beginPage(operation = OP) {
      if (operation !== OP) return fail('unsupported');
      const check = validScope(); if (!check.ok) return check;
      if (pending) return fail('busy');
      if (finished) return fail('complete');
      if (rows.length >= INVENTORY_LIMITS.records || pages.length >= INVENTORY_LIMITS.pages) return fail('capacity');
      const request = Object.freeze({ requestId: ++serial, generation, adapter: Object.freeze({ ...scope.adapter, operations: Object.freeze([...scope.adapter.operations]) }), binding: Object.freeze({ ...scope.binding }), access: Object.freeze({ ...scope.access, grantOperations: Object.freeze([...scope.access.grantOperations]) }), sessionId: scope.sessionId, snapshotId: scope.snapshotId, afterCursor: cursor, operation: OP });
      pending = { request }; return { ok: true, request };
    },
    acceptPage(request, json) {
      const check = validScope(); if (!check.ok) return check;
      if (!pending || request !== pending.request || request.generation !== generation) return fail('stale_request');
      // An admitted callback consumes the request even when its report is bad.
      pending = null;
      const page = decode(json); if (!pageValue(page)) { error = fail('invalid').error; return fail('invalid'); }
      if (!sameAdapter(page.adapter, scope.adapter) || !sameAccess(page.access, scope.access) || !sameBinding(page.binding, scope.binding) || page.sessionId !== scope.sessionId || page.snapshotId !== scope.snapshotId) { error = fail('scope_mismatch').error; return fail('scope_mismatch'); }
      if (page.requestId !== request.requestId || page.afterCursor !== cursor || page.nextCursor !== null && (page.nextCursor === page.afterCursor || cursors.has(page.nextCursor))) { error = fail('cursor_conflict').error; return fail('cursor_conflict'); }
      if (pages.length && page.coverage !== pages[0].coverage) { error = fail('snapshot_conflict').error; return fail('snapshot_conflict'); }
      if (rows.length + page.items.length > INVENTORY_LIMITS.records) { error = fail('capacity').error; return fail('capacity'); }
      const ids = new Set(rows.map(row => row.fileId)), paths = new Set(rows.map(row => row.relativePath));
      for (const row of page.items) {
        if (ids.has(row.fileId) || paths.has(row.relativePath)) { error = fail('duplicate').error; return fail('duplicate'); }
        ids.add(row.fileId); paths.add(row.relativePath);
      }
      if (page.nextCursor !== null) cursors.add(page.nextCursor);
      rows.push(...page.items); pages.push({ observedAtMs: page.observedAtMs, receivedAtMs: check.current, coverage: page.coverage, count: page.items.length });
      cursor = page.nextCursor; finished = cursor === null; error = null; status = (rows.length >= INVENTORY_LIMITS.records || pages.length >= INVENTORY_LIMITS.pages) && !finished ? 'capacity' : 'offline';
      return { ok: true, accepted: page.items.length };
    },
    getState() {
      if (scope) validScope();
      const current = lastNow;
      return { mode: 'offline_unverified', status, scope: scope ? structuredClone(scope) : null, pending: Boolean(pending), records: structuredClone(rows), pages: pages.map(page => ({ ...page, freshness: freshness(page.observedAtMs, page.receivedAtMs, current) })), nextCursor: cursor, finished, reportedCoverage: pages[0]?.coverage ?? 'unknown', window: pages.length === 0 ? 'not_started' : finished ? 'complete' : status === 'capacity' ? 'capacity' : 'partial', error: error ? { ...error } : null,
        provenance: 'Unverified producer metadata. No connection, authenticated identity, approval or execution is established.' };
    },
    invalidate(reason = 'cancelled') {
      if (disposed) return;
      clear(['cancelled', 'disconnected', 'revoked', 'locked', 'identity_changed'].includes(reason) ? reason : 'cancelled');
    },
    dispose() { clear('disposed'); disposed = true; },
  };
}
