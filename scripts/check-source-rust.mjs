// Pin Cargo and every compiler subprocess. A PATH-selected cargo-clippy can
// otherwise bypass the toolchain recorded in rust-toolchain.toml.
import { execFileSync, spawnSync } from 'node:child_process';
import { accessSync, constants, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TOOLS = ['cargo', 'rustc', 'rustdoc', 'cargo-fmt', 'rustfmt', 'cargo-clippy', 'clippy-driver'];
const WRAPPERS = ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER'];

function noComment(line) {
  let quoted = false;
  for (let i = 0; i < line.length; i += 1) {
    if (line[i] === '"') quoted = !quoted;
    if (line[i] === '#' && !quoted) return line.slice(0, i);
  }
  return line;
}

function tomlString(text, section, key, label) {
  let current = null;
  let matchingSections = 0;
  const found = [];
  for (const raw of text.split(/\r?\n/u)) {
    const line = noComment(raw).trim();
    if (!line) continue;
    const header = line.match(/^\[([A-Za-z0-9_.-]+)\]$/u);
    if (header) {
      current = header[1];
      if (current === section && ++matchingSections > 1) throw new Error(`${label} section must appear exactly once.`);
      continue;
    }
    const assignment = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/u);
    if (!assignment || current !== section || assignment[1] !== key) continue;
    const value = assignment[2].trim().match(/^"([^"\\]*)"$/u);
    if (!value) throw new Error(`${label} must be one unescaped TOML string.`);
    found.push(value[1]);
  }
  if (matchingSections !== 1) throw new Error(`${label} section must appear exactly once.`);
  if (found.length !== 1) throw new Error(`${label} must appear exactly once.`);
  return found[0];
}

export function clippyVersionFor(pin) {
  const [major, minor] = pin.split('.');
  return `0.${major}.${minor}`;
}

export function validateConfiguration({ toolchainToml, workspaceToml, desktopToml, nativeInputToml, ciRustVersion } = {}) {
  if (![toolchainToml, workspaceToml, desktopToml, nativeInputToml].every(value => typeof value === 'string')) throw new Error('Rust version configuration files must be readable text.');
  const pin = tomlString(toolchainToml, 'toolchain', 'channel', 'rust-toolchain.toml [toolchain].channel');
  if (!/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/u.test(pin)) throw new Error('rust-toolchain.toml [toolchain].channel must be an exact major.minor.patch pin.');
  for (const [label, value] of [
    ['Cargo.toml [workspace.package].rust-version', tomlString(workspaceToml, 'workspace.package', 'rust-version', 'Cargo.toml [workspace.package].rust-version')],
    ['apps/desktop/Cargo.toml [package].rust-version', tomlString(desktopToml, 'package', 'rust-version', 'apps/desktop/Cargo.toml [package].rust-version')],
    ['apps/desktop/native-input/Cargo.toml [package].rust-version', tomlString(nativeInputToml, 'package', 'rust-version', 'apps/desktop/native-input/Cargo.toml [package].rust-version')],
  ]) if (value !== pin) throw new Error(`${label} must equal rust-toolchain.toml channel ${pin}.`);
  if (ciRustVersion !== undefined && ciRustVersion !== '' && ciRustVersion !== pin) throw new Error(`RANGOON_RUST_VERSION must equal rust-toolchain.toml channel ${pin}.`);
  return { pin, clippyVersion: clippyVersionFor(pin) };
}

export function parseCliArgs(argv) {
  const options = { checkConfig: false, desktop: false, toolchainBin: null };
  for (let i = 0; i < argv.length; i += 1) {
    const argument = argv[i];
    if (argument === '--check-config') {
      if (options.checkConfig) throw new Error('Duplicate --check-config.');
      options.checkConfig = true;
    } else if (argument === '--desktop') {
      if (options.desktop) throw new Error('Duplicate --desktop.');
      options.desktop = true;
    } else if (argument === '--toolchain-bin') {
      if (options.toolchainBin !== null) throw new Error('Duplicate --toolchain-bin.');
      const value = argv[++i];
      if (!value || value.startsWith('--') || !path.isAbsolute(value)) throw new Error('--toolchain-bin requires an absolute directory path.');
      options.toolchainBin = path.normalize(value);
    } else throw new Error(`Unknown argument: ${argument}`);
  }
  if (options.checkConfig && (options.desktop || options.toolchainBin !== null)) throw new Error('--check-config cannot be combined with toolchain execution options.');
  return options;
}

export function resolveToolchain({ pin, toolchainBin, cwd = ROOT, env = process.env, execFileSyncFn = execFileSync, accessSyncFn = accessSync, platform = process.platform } = {}) {
  const suffix = platform === 'win32' ? '.exe' : '';
  let bin = toolchainBin;
  if (bin === null || bin === undefined) {
    // rustup expects the logical tool name on every OS; returned Windows paths already include .exe.
    const cargo = String(execFileSyncFn('rustup', ['which', '--toolchain', pin, 'cargo'], { cwd, env: { ...env, RUSTUP_AUTO_INSTALL: '0' }, encoding: 'utf8' })).trim();
    if (!path.isAbsolute(cargo)) throw new Error('rustup which returned a non-absolute Cargo path.');
    bin = path.dirname(cargo);
  } else if (!path.isAbsolute(bin)) throw new Error('--toolchain-bin requires an absolute directory path.');
  const tools = Object.fromEntries(TOOLS.map(name => [name.replace(/-/gu, '_'), path.join(bin, `${name}${suffix}`)]));
  for (const executable of Object.values(tools)) accessSyncFn(executable, constants.X_OK);
  return { bin, tools };
}

export function prepareChildEnv({ env = process.env, pin, bin, tools } = {}) {
  for (const key of WRAPPERS) if (env[key]) throw new Error(`${key} must be unset for pinned source validation.`);
  const child = { ...env, RUSTUP_TOOLCHAIN: pin, RUSTUP_AUTO_INSTALL: '0' };
  const inheritedPath = Object.entries(child).filter(([key]) => key.toLowerCase() === 'path').map(([, value]) => value).join(path.delimiter);
  for (const key of Object.keys(child)) if (key.toLowerCase() === 'path') delete child[key];
  child.PATH = `${bin}${inheritedPath ? `${path.delimiter}${inheritedPath}` : ''}`;
  child.CARGO = tools.cargo;
  child.RUSTC = tools.rustc;
  child.RUSTDOC = tools.rustdoc;
  child.RUSTFMT = tools.rustfmt;
  // Empty direct variables override Cargo file configuration for this child only.
  child.RUSTC_WRAPPER = '';
  child.RUSTC_WORKSPACE_WRAPPER = '';
  return child;
}

export function verifyToolIdentities({ pin, clippyVersion, tools, cwd = ROOT, env, execFileSyncFn = execFileSync } = {}) {
  const escape = value => value.replaceAll('.', '\\.');
  const run = (executable, args) => {
    const version = String(execFileSyncFn(executable, args, { cwd, env, encoding: 'utf8' })).trim();
    console.log(version);
    return version;
  };
  const compiler = run(tools.rustc, ['--version', '--verbose']);
  const compilerCommit = compiler.match(/^commit-hash: ([0-9a-f]{40})$/mu)?.[1];
  if (!new RegExp(`^rustc ${escape(pin)}(?:\\s|$)`, 'u').test(compiler)
    || !new RegExp(`^release: ${escape(pin)}$`, 'mu').test(compiler)
    || !compilerCommit) throw new Error(`rustc identity does not match pinned Rust ${pin}.`);
  const rustdoc = run(tools.rustdoc, ['--version', '--verbose']);
  if (!new RegExp(`^rustdoc ${escape(pin)}(?:\\s|$)`, 'u').test(rustdoc)
    || !new RegExp(`^release: ${escape(pin)}$`, 'mu').test(rustdoc)
    || !rustdoc.includes(`commit-hash: ${compilerCommit}`)) throw new Error(`rustdoc identity does not match pinned Rust ${pin}.`);
  const cargo = run(tools.cargo, ['--version']);
  if (!new RegExp(`^cargo ${escape(pin)}(?:\\s|$)`, 'u').test(cargo)) throw new Error(`cargo identity does not match pinned Rust ${pin}.`);
  const assertCommit = (executable, name, expected) => {
    const version = run(executable, ['--version']);
    const match = version.match(expected);
    if (!match || !compilerCommit.startsWith(match[1])) throw new Error(`${name} identity does not match rustc commit ${compilerCommit}.`);
  };
  const commit = '([0-9a-f]{10,40})';
  assertCommit(tools.rustfmt, 'rustfmt', new RegExp(`^rustfmt \\S+ \\(${commit}(?:\\s|\\))`, 'u'));
  assertCommit(tools.cargo_fmt, 'cargo-fmt', new RegExp(`^rustfmt \\S+ \\(${commit}(?:\\s|\\))`, 'u'));
  assertCommit(tools.cargo_clippy, 'cargo-clippy', new RegExp(`^clippy ${escape(clippyVersion)} \\(${commit}(?:\\s|\\))`, 'u'));
  assertCommit(tools.clippy_driver, 'clippy-driver', new RegExp(`^clippy ${escape(clippyVersion)} \\(${commit}(?:\\s|\\))`, 'u'));
}

export function commandPlan({ desktop = false } = {}) {
  if (desktop) return [
    ['fmt', '--manifest-path', 'apps/desktop/Cargo.toml', '--', '--check'],
    ['test', '--manifest-path', 'apps/desktop/Cargo.toml', '--locked'],
    ['clippy', '--manifest-path', 'apps/desktop/Cargo.toml', '--all-targets', '--locked', '--', '-D', 'warnings'],
    ['build', '--manifest-path', 'apps/desktop/Cargo.toml', '--locked'],
  ];
  return [['fmt', '--all', '--', '--check'], ['test', '--workspace', '--locked'], ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']];
}

export function runValidator(argv = process.argv.slice(2), dependencies = {}) {
  const cwd = dependencies.cwd ?? ROOT;
  const env = dependencies.env ?? process.env;
  const read = dependencies.readFileSyncFn ?? readFileSync;
  const options = parseCliArgs(argv);
  const config = validateConfiguration({
    toolchainToml: read(path.join(cwd, 'rust-toolchain.toml'), 'utf8'),
    workspaceToml: read(path.join(cwd, 'Cargo.toml'), 'utf8'),
    desktopToml: read(path.join(cwd, 'apps/desktop/Cargo.toml'), 'utf8'),
    nativeInputToml: read(path.join(cwd, 'apps/desktop/native-input/Cargo.toml'), 'utf8'),
    ciRustVersion: env.RANGOON_RUST_VERSION,
  });
  if (options.checkConfig) return config;
  const resolved = resolveToolchain({ pin: config.pin, toolchainBin: options.toolchainBin, cwd, env, execFileSyncFn: dependencies.execFileSyncFn, accessSyncFn: dependencies.accessSyncFn, platform: dependencies.platform });
  const childEnv = prepareChildEnv({ env, pin: config.pin, bin: resolved.bin, tools: resolved.tools });
  verifyToolIdentities({ pin: config.pin, clippyVersion: config.clippyVersion, tools: resolved.tools, cwd, env: childEnv, execFileSyncFn: dependencies.execFileSyncFn });
  const spawn = dependencies.spawnSyncFn ?? spawnSync;
  for (const args of commandPlan(options)) {
    const result = spawn(resolved.tools.cargo, args, { cwd, env: childEnv, stdio: 'inherit' });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`Pinned Cargo command failed: ${args.join(' ')}`);
  }
  return config;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { runValidator(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
