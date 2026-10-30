import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { commandPlan, parseCliArgs, prepareChildEnv, resolveToolchain, runValidator, validateConfiguration, verifyToolIdentities } from '../scripts/check-source-rust.mjs';

const pin = '1.99.1';
const compilerCommit = '58cf8f9c35b355199727627d07ae34a4e2846d67';
const bin = path.join(path.sep, 'opt', 'rust', 'bin');
const toolchainToml = `[toolchain]\nchannel = "${pin}"\nprofile = "minimal"\ncomponents = ["rustfmt", "clippy"]\n`;
const workspaceToml = `[workspace.package]\nrust-version = "${pin}"\n`;
const desktopToml = `[package]\nrust-version = "${pin}"\n`;
const nativeInputToml = `[package]\nrust-version = "${pin}"\n`;
const tools = Object.fromEntries(['cargo', 'rustc', 'rustdoc', 'cargo-fmt', 'rustfmt', 'cargo-clippy', 'clippy-driver'].map(name => [name.replace(/-/g, '_'), path.join(bin, name)]));
const corePlan = [['fmt', '--all', '--', '--check'], ['test', '--workspace', '--locked'], ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']];
const desktopPlan = [['fmt', '--manifest-path', 'apps/desktop/Cargo.toml', '--', '--check'], ['test', '--manifest-path', 'apps/desktop/Cargo.toml', '--locked'], ['clippy', '--manifest-path', 'apps/desktop/Cargo.toml', '--all-targets', '--locked', '--', '-D', 'warnings'], ['build', '--manifest-path', 'apps/desktop/Cargo.toml', '--locked']];

function versionOutput(executable, args, mixed = false) {
  const name = path.basename(executable);
  const commit = mixed && name === 'rustdoc' ? 'a'.repeat(40) : compilerCommit;
  if (name === 'rustc' || name === 'rustdoc') return `${name} ${pin} (${commit.slice(0, 10)} 2026-10-13)\ncommit-hash: ${commit}\nrelease: ${pin}`;
  if (name === 'cargo') return `cargo ${pin} (5f94df478 2026-08-27)`;
  if (name === 'rustfmt' || name === 'cargo-fmt') return `rustfmt 1.10.0-stable (${commit.slice(0, 10)} 2026-10-13)`;
  if (name === 'cargo-clippy' || name === 'clippy-driver') return `clippy 0.1.99 (${commit.slice(0, 10)} 2026-10-13)`;
  throw new Error(`Unexpected version identity: ${name} ${args.join(' ')}`);
}

test('configuration requires one exact shared patch pin', () => {
  assert.deepEqual(validateConfiguration({ toolchainToml, workspaceToml, desktopToml, nativeInputToml, ciRustVersion: pin }), { pin, clippyVersion: '0.1.99' });
  assert.throws(() => validateConfiguration({ toolchainToml, workspaceToml: workspaceToml.replace(pin, '1.99.0'), desktopToml, nativeInputToml }), /must equal/);
  assert.throws(() => validateConfiguration({ toolchainToml: toolchainToml.replace(pin, '1.99'), workspaceToml, desktopToml, nativeInputToml }), /exact major/);
  assert.throws(() => validateConfiguration({ toolchainToml: `${toolchainToml}[toolchain]\nprofile = "minimal"\n`, workspaceToml, desktopToml, nativeInputToml }), /section must appear exactly once/);
  assert.throws(() => validateConfiguration({ toolchainToml, workspaceToml, desktopToml, nativeInputToml, ciRustVersion: '1.99.0' }), /RANGOON_RUST_VERSION/);
  assert.throws(() => validateConfiguration({ toolchainToml, workspaceToml, desktopToml }), /readable text/);
  assert.throws(() => validateConfiguration({ toolchainToml, workspaceToml, desktopToml, nativeInputToml: nativeInputToml.replace(pin, '1.99.0') }), /apps\/desktop\/native-input\/Cargo\.toml \[package\]\.rust-version must equal/);
  assert.throws(() => validateConfiguration({ toolchainToml, workspaceToml, desktopToml, nativeInputToml: `${nativeInputToml}[package]\nrust-version = "${pin}"\n` }), /apps\/desktop\/native-input\/Cargo\.toml \[package\]\.rust-version section must appear exactly once/);
});

test('CLI rejects unknown, duplicate, and conflicting arguments', () => {
  assert.deepEqual(parseCliArgs(['--desktop', '--toolchain-bin', '/tmp/rust/bin']), { checkConfig: false, desktop: true, toolchainBin: '/tmp/rust/bin' });
  assert.throws(() => parseCliArgs(['--wat']), /Unknown/);
  assert.throws(() => parseCliArgs(['--desktop', '--desktop']), /Duplicate/);
  assert.throws(() => parseCliArgs(['--toolchain-bin', 'relative']), /absolute/);
  assert.throws(() => parseCliArgs(['--check-config', '--desktop']), /cannot be combined/);
});

test('resolution requires seven absolute executables and default rustup cannot install', () => {
  const accessed = [];
  const bin = path.join(path.sep, 'opt', 'rust', 'bin');
  const tools = resolveToolchain({ pin, toolchainBin: bin, accessSyncFn: executable => accessed.push(executable) }).tools;
  assert.equal(accessed.length, 7);
  assert.equal(tools.cargo, path.join(bin, 'cargo'));
  assert.throws(() => resolveToolchain({ pin, toolchainBin: bin, accessSyncFn: executable => { if (executable.endsWith('clippy-driver')) throw new Error('missing'); } }), /missing/);
  let rustup;
  resolveToolchain({ pin, env: {}, execFileSyncFn: (...args) => { rustup = args; return '/opt/rust/bin/cargo\n'; }, accessSyncFn: () => {} });
  assert.deepEqual(rustup[0], 'rustup');
  assert.equal(rustup[2].env.RUSTUP_AUTO_INSTALL, '0');
});

test('wrapper environment is refused and child disables configured wrappers', () => {
  for (const wrapper of ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']) {
    assert.throws(() => prepareChildEnv({ env: { [wrapper]: '/evil' }, pin, bin: '/opt/rust/bin', tools: {} }), new RegExp(wrapper));
  }
  const bin = path.join(path.sep, 'opt', 'rust', 'bin');
  const env = prepareChildEnv({ env: { PATH: '/base', RUSTUP_AUTO_INSTALL: '1' }, pin, bin, tools: { cargo: path.join(bin, 'cargo'), rustc: path.join(bin, 'rustc'), rustdoc: path.join(bin, 'rustdoc'), rustfmt: path.join(bin, 'rustfmt') } });
  assert.equal(env.RUSTC_WRAPPER, '');
  assert.equal(env.RUSTC_WORKSPACE_WRAPPER, '');
  assert.equal(env.RUSTUP_AUTO_INSTALL, '0');
  assert.equal(env.PATH, `${bin}${path.delimiter}/base`);
});

test('identity checks pin every tool and rejects mixed components', () => {
  const calls = [];
  verifyToolIdentities({ pin, clippyVersion: '0.1.99', tools, env: {}, execFileSyncFn: (executable, args) => {
    calls.push([executable, args]);
    return versionOutput(executable, args);
  } });
  assert.deepEqual(calls.map(([executable]) => executable), [tools.rustc, tools.rustdoc, tools.cargo, tools.rustfmt, tools.cargo_fmt, tools.cargo_clippy, tools.clippy_driver]);
  assert.throws(() => verifyToolIdentities({ pin, clippyVersion: '0.1.99', tools, env: {}, execFileSyncFn: (executable, args) => versionOutput(executable, args, true) }), /rustdoc identity/);
  assert.throws(() => verifyToolIdentities({ pin, clippyVersion: '0.1.99', tools, env: {}, execFileSyncFn: (executable, args) => executable.endsWith('cargo') ? 'cargo 1.99.1-nightly' : versionOutput(executable, args) }), /cargo identity/);
  for (const mixedTool of [tools.rustfmt, tools.cargo_fmt, tools.cargo_clippy, tools.clippy_driver]) {
    assert.throws(() => verifyToolIdentities({ pin, clippyVersion: '0.1.99', tools, env: {}, execFileSyncFn: (executable, args) => {
      const output = versionOutput(executable, args);
      return executable === mixedTool ? output.replace(compilerCommit.slice(0, 10), 'aaaaaaaaaa') : output;
    } }), /identity does not match rustc commit/);
  }
  assert.deepEqual(commandPlan(), corePlan);
  assert.deepEqual(commandPlan({ desktop: true }), desktopPlan);
});

test('check-config is static and never resolves a compiler', () => {
  const files = new Map([['rust-toolchain.toml', toolchainToml], ['Cargo.toml', workspaceToml], [path.join('apps', 'desktop', 'Cargo.toml'), desktopToml], [path.join('apps', 'desktop', 'native-input', 'Cargo.toml'), nativeInputToml]]);
  const result = runValidator(['--check-config'], {
    cwd: '/repo',
    env: {},
    readFileSyncFn: file => files.get(path.relative('/repo', file)),
    execFileSyncFn: () => { throw new Error('must not execute'); },
    spawnSyncFn: () => { throw new Error('must not execute'); },
  });
  assert.equal(result.pin, pin);
});

test('check-config reads and requires native-input manifest', () => {
  const root = path.join(path.sep, 'repo');
  const reads = [];
  const files = new Map([['rust-toolchain.toml', toolchainToml], ['Cargo.toml', workspaceToml], [path.join('apps', 'desktop', 'Cargo.toml'), desktopToml]]);
  assert.throws(() => runValidator(['--check-config'], {
    cwd: root,
    env: {},
    readFileSyncFn: file => {
      const relative = path.relative(root, file);
      reads.push(relative);
      return files.get(relative);
    },
    execFileSyncFn: () => { throw new Error('must not execute'); },
    spawnSyncFn: () => { throw new Error('must not execute'); },
  }), /readable text/);
  assert.deepEqual(reads, ['rust-toolchain.toml', 'Cargo.toml', path.join('apps', 'desktop', 'Cargo.toml'), path.join('apps', 'desktop', 'native-input', 'Cargo.toml')]);
});

test('full execution pins child environment, command plan, and failure status', () => {
  const root = path.join(path.sep, 'repo');
  const files = new Map([['rust-toolchain.toml', toolchainToml], ['Cargo.toml', workspaceToml], [path.join('apps', 'desktop', 'Cargo.toml'), desktopToml], [path.join('apps', 'desktop', 'native-input', 'Cargo.toml'), nativeInputToml]]);
  const execute = (argv, status) => {
    const calls = [];
    const dependencies = {
      cwd: root,
      env: { PATH: '/base', RUSTUP_AUTO_INSTALL: '1' },
      readFileSyncFn: file => files.get(path.relative(root, file)),
      execFileSyncFn: (executable, args) => {
        if (executable === 'rustup') return `${path.join(path.sep, 'opt', 'rust', 'bin', 'cargo')}\n`;
        return versionOutput(executable, args);
      },
      accessSyncFn: () => {},
      spawnSyncFn: (executable, args, options) => { calls.push([executable, args, options]); return { status }; },
    };
    if (status === 0) runValidator(argv, dependencies);
    else assert.throws(() => runValidator(argv, dependencies), /Pinned Cargo command failed/);
    return calls;
  };
  const core = execute([], 0);
  assert.equal(core.length, 3);
  assert.equal(core[0][0], path.join(path.sep, 'opt', 'rust', 'bin', 'cargo'));
  assert.equal(core[0][2].env.RUSTUP_TOOLCHAIN, pin);
  assert.equal(core[0][2].env.RUSTUP_AUTO_INSTALL, '0');
  assert.deepEqual(core.map(([, args]) => args), corePlan);
  const desktop = execute(['--desktop'], 0);
  assert.deepEqual(desktop.map(([, args]) => args), desktopPlan);
  assert.equal(execute([], 2).length, 1);
});
