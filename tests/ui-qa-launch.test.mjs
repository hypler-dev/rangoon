import test from 'node:test';
import assert from 'node:assert/strict';
import { uiQaLaunch } from '../scripts/ui-qa-launch.mjs';

const input = { platform: 'darwin', hostArm64: true, browserPath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', explicitOverride: false, args: ['--headless=new', '--user-data-dir=/tmp/profile with spaces', 'about:blank'] };
test('ARM macOS selects native browser independently of Node architecture', () => {
  for (const nodeArch of ['x64', 'arm64']) {
    assert.deepEqual(uiQaLaunch({ ...input, nodeArch }), { command: '/usr/bin/arch', args: ['-arm64', input.browserPath, ...input.args], selection: 'default_macos_arm64' });
  }
});
test('Intel macOS and non-macOS hosts preserve direct launch', () => {
  for (const host of [{ platform: 'darwin', hostArm64: false }, { platform: 'linux', hostArm64: true }, { platform: 'win32' }]) {
    const result = uiQaLaunch({ ...input, ...host });
    assert.equal(result.command, input.browserPath);
    assert.deepEqual(result.args, input.args);
    assert.equal(result.selection, 'direct_host_default');
  }
});
test('explicit browser override is never architecture-wrapped or shell-expanded', () => {
  const browserPath = '/tmp/Chrome $(not-a-command)';
  const result = uiQaLaunch({ ...input, browserPath, explicitOverride: true });
  assert.equal(result.command, browserPath);
  assert.deepEqual(result.args, input.args);
  assert.equal(result.selection, 'explicit_browser_override');
  result.args.push('extra');
  assert.equal(input.args.length, 3);
});
test('unknown macOS architecture and malformed arguments fail before launch', () => {
  for (const change of [{ hostArm64: undefined }, { hostArm64: '1' }, { browserPath: '' }, { args: [null] }, { args: 'flags' }]) {
    assert.throws(() => uiQaLaunch({ ...input, ...change }), TypeError);
  }
});
