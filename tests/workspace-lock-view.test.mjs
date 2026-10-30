import test from 'node:test';
import assert from 'node:assert/strict';
import { renderWorkspaceCloseShell, renderWorkspaceGuard, renderWorkspaceLockDialog, workspaceCloseAnnouncement } from '../preview/workspace-lock-view.mjs';

test('guard only enables lock for observed managed open state', () => {
  const open = renderWorkspaceGuard({ status: 'open', canLock: true });
  assert.match(open, /data-workspace-guard/);
  assert.match(open, /id="workspace-lock-open"/);
  assert.match(open, /data-workspace-lock-open/);
  for (const status of ['browser', 'legacy', 'unavailable', 'checking', 'closing', 'locked', 'recovery_required']) {
    assert.doesNotMatch(renderWorkspaceGuard({ status, canLock: true }), /workspace-lock-open/);
  }
});

test('lock dialog keeps Cancel first and contains only fixed commit hooks', () => {
  const dialog = renderWorkspaceLockDialog();
  assert.match(dialog, /<dialog id="workspace-lock-dialog"/);
  assert.match(dialog, /id="workspace-lock-dialog-title"/);
  assert.match(dialog, /id="workspace-lock-dialog-description"/);
  assert.match(dialog, /id="workspace-lock-cancel" type="button" data-workspace-lock-cancel autofocus/);
  assert.match(dialog, /id="workspace-lock-confirm" type="button" data-workspace-lock-confirm/);
  assert.ok(dialog.indexOf('workspace-lock-cancel') < dialog.indexOf('workspace-lock-confirm'));
  assert.match(dialog, /Unsaved drafts and window history will be discarded/);
  assert.match(dialog, /Writes already begun may have completed/);
  assert.doesNotMatch(dialog, /<form|onclick=|onkeydown=/);
});

test('close shell is source-free and retains only theme and help controls', () => {
  for (const status of ['checking', 'unavailable', 'closing', 'locked', 'recovery_required']) {
    const shell = renderWorkspaceCloseShell({ status }, { theme: 'light' });
    assert.match(shell, /<main id="analysis-main"/);
    assert.match(shell, /<h1 id="workspace-close-title" tabindex="-1">/);
    assert.match(shell, /id="analysis-theme"/);
    assert.match(shell, /Dark mode/);
    assert.match(shell, /<summary>Technical details<\/summary>/);
    assert.doesNotMatch(shell, /href=/);
    assert.doesNotMatch(shell, /analysis-nav|workspace-lock-open|workspace-lock-confirm|retry|unlock|setup|progress/i);
  }
});

test('protection shell exposes exactly one fixed action for eligible absent or locked layouts', () => {
  for (const [layout, label] of [['absent', 'Set up encrypted workspace'], ['locked', 'Unlock workspace']]) {
    const shell = renderWorkspaceCloseShell({ status: 'protection', layout, canProtect: true, pending: false });
    assert.match(shell, new RegExp(`>${label}<`));
    assert.match(shell, /data-workspace-protection-action/);
    assert.match(shell, new RegExp(`data-workspace-layout="${layout}"`));
    assert.equal((shell.match(/data-workspace-protection-action/g) ?? []).length, 1);
    assert.match(workspaceCloseAnnouncement({ status: 'protection', layout, canProtect: true, pending: false }), new RegExp(`^${label}\\.`));
  }
  for (const state of [{ status: 'protection', layout: 'busy', canProtect: true }, { status: 'protection', layout: 'locked', canProtect: false }, { status: 'protection', layout: 'locked', canProtect: true, pending: true }]) {
    assert.doesNotMatch(renderWorkspaceCloseShell(state), /data-workspace-protection-action/);
  }
});

test('protection shell names pending and unavailable layouts without enabling an action', () => {
  for (const [state, copy] of [
    [{ status: 'protection', layout: 'busy' }, 'Workspace busy'],
    [{ status: 'protection', layout: 'recovery_required' }, 'Workspace recovery required'],
    [{ status: 'protection', layout: 'unavailable' }, 'Workspace unavailable'],
    [{ status: 'protection', layout: 'locked', pending: true }, 'Preparing workspace'],
  ]) {
    const shell = renderWorkspaceCloseShell(state);
    assert.match(shell, new RegExp(`>${copy}<`));
    assert.doesNotMatch(shell, /data-workspace-protection-action/);
  }
});

test('protection announcements and technical disclosure stay finite and source-free', () => {
  assert.equal(workspaceCloseAnnouncement({ status: 'protection', layout: 'busy' }), 'Workspace busy. Workspace protection is still completing. This window remains closed.');
  assert.equal(workspaceCloseAnnouncement({ status: 'protection', layout: 'locked', pending: true }), 'Preparing workspace. Workspace protection is preparing. This window remains closed.');
  const shell = renderWorkspaceCloseShell({ status: 'protection', layout: 'legacy' });
  assert.match(shell, /native desktop storage/);
  assert.match(shell, /Exports can remain readable/);
  assert.doesNotMatch(shell, /attemptId|epoch|private-text|sourceId/);
});

test('exact opened transition announces reload without a false failure or action', () => {
  const state = { status: 'reloading' };
  assert.equal(workspaceCloseAnnouncement(state), 'Opening workspace. Reloading after native workspace protection opened.');
  const shell = renderWorkspaceCloseShell(state);
  assert.match(shell, /Opening workspace/);
  assert.doesNotMatch(shell, /Workspace unavailable|data-workspace-protection-action/);
});

test('close shell does not reflect unexpected state data', () => {
  const shell = renderWorkspaceCloseShell({ status: '<img src=x onerror=alert(1)>', source: 'private source' });
  assert.match(shell, /Workspace unavailable/);
  assert.doesNotMatch(shell, /private source|onerror|alert/);
});

test('closed announcements use fixed source-free copy for every state and malformed input', () => {
  assert.equal(workspaceCloseAnnouncement({ status: 'locked', source: 'private-text' }), 'Workspace locked. This window no longer shows workspace source content.');
  for (const status of ['checking', 'unavailable', 'closing', 'locked', 'recovery_required', '<private-text>']) {
    const announcement = workspaceCloseAnnouncement({ status, message: 'private-text', attemptId: 'private-token' });
    assert.ok(announcement.length > 30);
    assert.doesNotMatch(announcement, /private-text|private-token/);
  }
});


test('fresh setup after Cancel retains canonical title, description, Details and native Quit', () => {
  const shell = renderWorkspaceCloseShell({ status: 'protection', layout: 'absent', canProtect: true, pending: false }, { quitAvailable: true });
  assert.match(shell, /Set up encrypted workspace/);
  assert.match(shell, /Create an encrypted workspace on this computer\./);
  assert.match(shell, /<summary>Details<\/summary>/);
  assert.match(shell, /No original copy is part of this setup\./);
  assert.match(shell, /Encrypted portable backup unavailable\./);
  assert.match(shell, /data-workspace-close-quit>Quit Rangoon/);
  assert.doesNotMatch(renderWorkspaceCloseShell({ status: 'protection', layout: 'absent', canProtect: true }), /data-workspace-close-quit/);
});

test('generic recovery provides conditional preservation guidance without diagnosing missing key', () => {
  for (const state of [{ status: 'recovery_required' }, { status: 'protection', layout: 'recovery_required' }]) {
    const shell = renderWorkspaceCloseShell({ ...state, diagnostic: 'private diagnostic', key: 'secret-key' }, { quitAvailable: true });
    assert.match(shell, /Keep workspace files intact/);
    assert.match(shell, /If operating-system account or credential-store access changed/);
    assert.match(shell, /If the original key is unavailable, Rangoon cannot replace it/);
    assert.match(shell, /does not repair damaged workspace files/);
    assert.match(shell, /data-workspace-close-quit/);
    assert.doesNotMatch(shell, /data-workspace-protection-action|private diagnostic|secret-key/);
  }
});
