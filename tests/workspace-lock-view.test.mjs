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
    assert.match(shell, /<summary>Workspace help<\/summary>/);
    assert.doesNotMatch(shell, /href=/);
    assert.doesNotMatch(shell, /analysis-nav|workspace-lock-open|workspace-lock-confirm|retry|unlock|setup|progress/i);
  }
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
