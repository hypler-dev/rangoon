import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceController } from '../preview/workspace-model.mjs';
import { renderWorkspaceView } from '../preview/workspace-view.mjs';

const id = (prefix, digit) => `${prefix}:${digit.repeat(64)}`;
const sourceId = id('source', '1');
const capabilityId = id('capability', '3');
const revisionId = id('revision', '4');
const workflowId = id('workflow', '5');
const workflowRevisionId = id('workflow-revision', '6');
const stateId = id('workspace', '7');
const backupId = id('backup', '8');
const counts = () => ({ sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 1, workflowRevisions: 1 });
const workspace = () => ({
  schemaVersion: 'rangoon.workspace-data.v2', stateId, records: counts(), databaseBytes: 4096, reusableBytes: 0,
  sources: [{ sourceId, displayName: 'AGENTS.md', sha256: 'a'.repeat(64), byteLength: 22, savedAtMs: 1 }],
  capabilities: [{ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId: id('fragment', '2') }, latestRevisionId: revisionId, title: 'Bounded skill', reviewed: false, revisionCount: 1 }],
  workflows: [{ id: workflowId, latestRevisionId: workflowRevisionId, label: 'Recovery workflow', labelAdjusted: false, intent: 'draft', structurallyValid: false, revisionCount: 1, savedAtMs: 1, unresolvedReferences: 1 }],
});
const plan = () => ({ schemaVersion: 'rangoon.restore-plan.v2', backupId, expectedStateId: stateId, byteLength: 4096, add: counts(), keptSources: 0, keptCapabilities: 0, keptWorkflows: 0 });

test('encrypted recovery stays unavailable for absent or malformed capability metadata and after purge', async () => {
  let load = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command !== 'get_workspace_data') throw new Error('unexpected command');
    load += 1;
    if (load === 1) return { outcome: 'loaded', workspace: workspace() };
    if (load === 2) return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: true };
    return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: 'yes' };
  } });
  await controller.load();
  assert.equal(controller.getState().encryptedRecoverySupported, false);
  await controller.load();
  assert.equal(controller.getState().encryptedRecoverySupported, true);
  await controller.load();
  assert.equal(controller.getState().encryptedRecoverySupported, false);
  assert.equal(controller.getState().status, 'ready');
  controller.purgeForWorkspaceLock();
  assert.equal(controller.getState().encryptedRecoverySupported, false);
});

test('encrypted backup commands pass no renderer data and enforce exact native receipts', async () => {
  const calls = [];
  const controller = createWorkspaceController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: true };
    if (command === 'create_workspace_recovery_key') return { outcome: 'recovery_key_created' };
    return { outcome: 'encrypted_exported', format: 'rangoon.recovery.sqlcipher.v1', backupId, byteLength: 4096 };
  } });
  await controller.load();
  controller.requestEncryptedExport();
  await controller.createRecoveryKey();
  assert.equal(controller.getState().dialog.kind, 'encrypted-export-warning');
  await controller.confirmEncryptedExport();
  assert.deepEqual(calls.find(([command]) => command === 'create_workspace_recovery_key'), ['create_workspace_recovery_key', undefined]);
  assert.deepEqual(calls.find(([command]) => command === 'export_encrypted_workspace_backup'), ['export_encrypted_workspace_backup', undefined]);
  assert.match(controller.getState().message, /Encrypted backup verified/);
});

test('recovery-key cancellation keeps the encrypted backup dialog and never retries', async () => {
  let creates = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: true };
    creates += 1;
    return { outcome: 'cancelled' };
  } });
  await controller.load();
  controller.requestEncryptedExport();
  await controller.createRecoveryKey();
  assert.equal(creates, 1);
  assert.equal(controller.getState().dialog.kind, 'encrypted-export-warning');
  assert.match(controller.getState().message, /creation cancelled/);
});

test('malformed recovery-key receipt locks actions and puts the fresh-start error inside the modal', async () => {
  const calls = [];
  const controller = createWorkspaceController({ invoke: async command => {
    calls.push(command);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: true };
    return { outcome: 'recovery_key_created', unexpected: true };
  } });
  await controller.load();
  controller.requestEncryptedExport();
  await controller.createRecoveryKey();
  const stale = controller.getState();
  assert.equal(stale.dialog.kind, 'encrypted-export-warning');
  assert.equal(stale.dialog.stale, true);
  assert.match(stale.error.message, /could not finish/);
  await controller.createRecoveryKey();
  await controller.confirmEncryptedExport();
  assert.deepEqual(calls, ['get_workspace_data', 'create_workspace_recovery_key']);

  const html = renderWorkspaceView(stale, { storageMode: 'managed_encrypted' });
  assert.match(html, /Recovery action needs a fresh start/);
  assert.match(html, /No key or backup success is recorded/);
  assert.match(html, /Close this dialog, then open Back up workspace/);
  assert.match(html, /Recovery requires both the encrypted backup and its separate secret recovery key file/);
  assert.match(html, /If the recovery key is lost, that backup cannot be restored/);
  assert.match(html, /data-workspace-confirm-encrypted-export[^>]*disabled/);
  assert.match(html, /data-workspace-create-recovery-key[^>]*disabled/);
  assert.doesNotMatch(html, /data-workspace-cancel autofocus disabled/);

  controller.cancel();
  const closed = controller.getState();
  assert.equal(closed.dialog, null);
  assert.match(closed.error.message, /could not finish/);
  assert.equal(closed.message, 'The recovery key file outcome is unknown. No recovery key or encrypted backup was confirmed. Open Back up workspace to start a new native action.');
  assert.doesNotMatch(closed.message, /cancelled|unchanged/i);
  controller.requestEncryptedExport();
  assert.equal(controller.getState().dialog.stale, undefined);
  assert.equal(controller.getState().error, null);
});

test('encrypted restore keeps an older plaintext preview and its origin after encrypted picker cancellation', async () => {
  let restorePickers = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace(), encryptedRecoverySupported: true };
    if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: plan() };
    if (command === 'prepare_encrypted_workspace_restore') return ++restorePickers === 1 ? { outcome: 'cancelled' } : { outcome: 'restore_ready', plan: plan() };
    throw new Error('unexpected command');
  } });
  await controller.load();
  await controller.prepareRestore();
  await controller.prepareEncryptedRestore();
  assert.equal(controller.getState().restoreOrigin, 'legacy');
  const legacy = renderWorkspaceView(controller.getState(), { storageMode: 'managed_encrypted' });
  assert.match(legacy, /OLDER PLAINTEXT BACKUP RESTORE/);
  await controller.prepareEncryptedRestore();
  assert.equal(controller.getState().restoreOrigin, 'encrypted');
  const encrypted = renderWorkspaceView(controller.getState(), { storageMode: 'managed_encrypted' });
  assert.match(encrypted, /ENCRYPTED RECOVERY RESTORE/);
});

test('managed recovery view shows supported primary actions and never renders a secret field', () => {
  const html = renderWorkspaceView({
    bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Ready.', workspace: workspace(),
    encryptedRecoverySupported: true, dialog: { kind: 'encrypted-export-warning' }, restorePlan: null, restoreOrigin: null, deletionPlan: null,
  }, { storageMode: 'managed_encrypted' });
  assert.match(html, /Back up workspace/);
  assert.match(html, /Restore backup/);
  assert.match(html, /Use recovery key/);
  assert.match(html, /Create recovery key/);
  assert.match(html, /More recovery options/);
  assert.match(html, /Import older backup/);
  assert.match(html, /recovery key file is itself unencrypted/);
  assert.match(html, /Protect it like a password and store it separately/);
  assert.match(html, /Anyone with both files can read this backup/);
  assert.match(html, /If the recovery key is lost, that backup cannot be restored/);
  assert.match(html, /<details id="workspace-encryption-limits" class="workbench-technical">/);
  assert.match(html, /Encryption covers saved workspace data and an explicitly created encrypted backup at rest/);
  assert.match(html, /does not protect an unlocked or compromised host, original files, or readable instruction exports/);
  assert.match(html, /does not establish an author, trust, or approval/);
  assert.match(html, /Windows and Linux native recovery qualification remains pending/);
  assert.doesNotMatch(html, /<input\b/i);
  assert.doesNotMatch(html, /recoveryKey|keyPath|backupPath/);
});
