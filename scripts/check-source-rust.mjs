// Pin both Cargo and its subprocesses; a Homebrew cargo-clippy earlier on PATH
// can otherwise bypass rustup's requested toolchain.
import { execFileSync, spawnSync } from 'node:child_process';
import { accessSync, constants } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const toolchain = '1.85.0';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const cargo = execFileSync('rustup', ['which', '--toolchain', toolchain, 'cargo'], {
  cwd: root, encoding: 'utf8',
}).trim();
const bin = path.dirname(cargo);
const suffix = process.platform === 'win32' ? '.exe' : '';
for (const executable of ['cargo', 'rustc', 'rustdoc', 'cargo-fmt', 'rustfmt', 'cargo-clippy', 'clippy-driver']) {
  accessSync(path.join(bin, `${executable}${suffix}`), constants.X_OK);
}
const env = { ...process.env, RUSTUP_TOOLCHAIN: toolchain };
const pathKeys = Object.keys(env).filter(key => key.toLowerCase() === 'path');
const inheritedPath = pathKeys.map(key => env[key]).join(path.delimiter);
for (const key of pathKeys) delete env[key];
env.PATH = `${bin}${path.delimiter}${inheritedPath}`;
env.CARGO = cargo;
env.RUSTC = path.join(bin, `rustc${suffix}`);
env.RUSTDOC = path.join(bin, `rustdoc${suffix}`);
env.RUSTFMT = path.join(bin, `rustfmt${suffix}`);
// A caller-supplied compiler wrapper would invalidate the evidence this command
// promises. Fail explicitly rather than silently changing that configuration.
for (const key of ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']) {
  if (env[key]) throw new Error(`${key} must be unset for pinned source validation.`);
}
// Empty values override Cargo file configuration for these child processes.
// This leaves the user's configuration files unchanged.
env.RUSTC_WRAPPER = '';
env.RUSTC_WORKSPACE_WRAPPER = '';

for (const [executable, args, expected] of [
  [env.RUSTC, ['--version'], /^rustc 1\.85\.0\b/],
  [cargo, ['--version'], /^cargo 1\.85\.0\b/],
  [cargo, ['clippy', '--version'], /^clippy 0\.1\.85\b/],
]) {
  const version = execFileSync(executable, args, { cwd: root, env, encoding: 'utf8' }).trim();
  console.log(version);
  if (!expected.test(version)) throw new Error('Source toolchain version does not match the pinned validator.');
}

for (const args of [
  ['fmt', '--all', '--', '--check'],
  ['test', '--workspace', '--locked'],
  ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'],
]) {
  const result = spawnSync(cargo, args, { cwd: root, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
