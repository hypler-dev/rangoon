#!/usr/bin/env python3
"""Unmounted macOS target-qualification prerequisite; synthetic self-test only.

No target launch command exists. This tool does not qualify Codex, Claude,
production containment, instruction loading, or execution authority.
"""
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import selectors
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

SANDBOX = Path('/usr/bin/sandbox-exec')
COMPILER = Path('/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang')
SDK = Path('/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk')
PROBE_SOURCE = Path(__file__).resolve().parents[1] / 'tests/fixtures/target-isolation-probe.c'
DYLD_SUPPORT = Path('/System/Library/Sandbox/Profiles/dyld-support.sb')
DYLD_SUPPORT_SHA256 = '06215a5d32689aefe395c29710e182eb54ba22162f50df8b4842290f8a19bf1c'
CANARY = b'RANGOON-SYNTHETIC-CANARY\n'


@dataclass(frozen=True)
class Capture:
    outcome: str
    returncode: int | None
    stdout: bytes
    stderr: bytes
    drained: bool


def clean_environment(scratch):
    """Do not copy ambient environment or redirect HOME/CODEX_HOME."""
    return {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C',
            'TMPDIR': str(scratch), 'PYTHONDONTWRITEBYTECODE': '1'}


def _group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def run_bounded(argv, *, cwd, timeout=3.0, byte_limit=65536):
    """Test infrastructure only; not an application command/execution API."""
    if (not isinstance(argv, list) or not argv or not all(isinstance(v, str) for v in argv)
            or type(timeout) not in (int, float) or not 0 < timeout <= 30
            or type(byte_limit) is not int or not 1 <= byte_limit <= 1024 * 1024):
        raise ValueError('invalid capture contract')
    # Allocate controller resources before any child exists. The outer guard
    # covers Popen and every subsequent pipe/selector setup operation.
    selector = selectors.DefaultSelector()
    process = None
    try:
        try:
            process = subprocess.Popen(argv, cwd=cwd, env=clean_environment(cwd),
                                       stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, close_fds=True, start_new_session=True)
        except (OSError, ValueError):
            return Capture('spawn_failed', None, b'', b'', True)
        buffers = [bytearray(), bytearray()]
        outcome = None
        deadline = time.monotonic() + timeout
        cleanup_deadline = None

        def stop(reason):
            nonlocal outcome, cleanup_deadline
            if outcome is not None:
                return
            outcome = reason
            cleanup_deadline = time.monotonic() + 1.0
            # Reap a leader that already exited before group termination.
            process.poll()
            if not _group_exists(process.pid):
                return
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError:
                process.poll()
                if _group_exists(process.pid):
                    outcome = 'termination_failed'

        for index, pipe in enumerate((process.stdout, process.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, index)
        while True:
            now = time.monotonic()
            if outcome is None and now >= deadline:
                stop('timeout')
            if outcome is not None and now >= cleanup_deadline:
                break
            for key, _ in selector.select(0.02):
                remaining = byte_limit - sum(map(len, buffers))
                try:
                    block = os.read(key.fileobj.fileno(), min(65536, remaining + 1))
                except BlockingIOError:
                    continue
                if not block:
                    selector.unregister(key.fileobj)
                else:
                    buffers[key.data].extend(block[:max(0, remaining)])
                    if len(block) > remaining:
                        stop('output_limit')
            exited = process.poll() is not None
            if exited and not selector.get_map():
                if _group_exists(process.pid):
                    stop('descendants_active')
                else:
                    break
        drained = process.poll() is not None and not selector.get_map() and not _group_exists(process.pid)
        if not drained:
            outcome = 'drain_incomplete'
        elif outcome is None:
            outcome = 'completed' if process.returncode == 0 else 'exit_failed'
        return Capture(outcome, process.returncode, bytes(buffers[0]), bytes(buffers[1]), drained)
    finally:
        try:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except OSError:
                    # Exact-leader cleanup is not successful group drain.
                    try:
                        process.kill()
                    except OSError:
                        pass
        finally:
            try:
                selector.close()
            finally:
                if process is not None:
                    for pipe in (process.stdout, process.stderr):
                        if pipe is not None:
                            try:
                                pipe.close()
                            except OSError:
                                pass
                    try:
                        process.wait(timeout=1.0)
                    except subprocess.TimeoutExpired:
                        pass


def _literal(path):
    path = str(path)
    if not path.startswith('/') or not re.fullmatch(r'[A-Za-z0-9/._-]+', path):
        raise ValueError('unsupported policy path')
    return json.dumps(path)


def probe_policy(binary, readable, scratch):
    """Candidate policy is restricted to our compiled probe, not arbitrary targets."""
    for path in (binary, readable, scratch):
        if not Path(path).is_absolute() or Path(path).resolve() != Path(path):
            raise ValueError('policy path must be canonical')
    return '\n'.join([
        '(version 1)', '(deny default)',
        f'(import {_literal(DYLD_SUPPORT)})',
        f'(allow process-exec (literal {_literal(binary)}))',
        '(allow file-read* (subpath "/System/Library") (subpath "/usr/lib"))',
        f'(allow file-read* (literal {_literal(binary)}) (literal {_literal(readable)}))',
        f'(allow file-map-executable (literal {_literal(binary)}) (subpath "/System/Library") (subpath "/usr/lib"))',
        f'(allow file-read* file-write* (subpath {_literal(scratch)}))',
    ])


def _digest(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def self_test():
    if platform.system() != 'Darwin' or not SANDBOX.is_file() or not COMPILER.is_file() or not SDK.is_dir():
        return {'schemaVersion': 'rangoon.target-isolation-selftest.v1', 'outcome': 'unavailable',
                'targetLaunched': False, 'reason': 'required_macos_tools_unavailable'}
    if not DYLD_SUPPORT.is_file() or _digest(DYLD_SUPPORT) != DYLD_SUPPORT_SHA256:
        return {'schemaVersion': 'rangoon.target-isolation-selftest.v1', 'outcome': 'unavailable',
                'targetLaunched': False, 'reason': 'reviewed_dyld_policy_unavailable'}
    root = Path(tempfile.mkdtemp(prefix='rangoon-target-isolation-', dir='/private/tmp')).resolve()
    identity = root.stat()
    scratch = root / 'scratch'
    scratch.mkdir(mode=0o700)
    readable, forbidden, binary, profile, endpoint = [root / name for name in (
        'allowed-input', 'blocked-input', 'probe', 'probe.sb', 'blocked.sock')]
    listener = None
    try:
        for path in (readable, forbidden):
            with path.open('xb') as file:
                file.write(CANARY)
            path.chmod(0o600)
        before = _digest(forbidden)
        compilation = run_bounded([str(COMPILER), '--no-default-config', '-std=c11',
                                   '-Wall', '-Wextra', '-Werror', '-O1', '-fno-modules',
                                   '-isysroot', str(SDK), '-o', str(binary), str(PROBE_SOURCE)], cwd=scratch, timeout=30)
        if compilation.outcome != 'completed':
            return {'schemaVersion': 'rangoon.target-isolation-selftest.v1', 'outcome': 'failed',
                    'targetLaunched': False, 'reason': 'probe_build_failed',
                    'captureOutcome': compilation.outcome}
        profile.write_text(probe_policy(binary, readable, scratch))
        profile.chmod(0o600)
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(str(endpoint))
        listener.listen(1)
        cases = [('allowed_read', 'read', readable, 0),
                 ('allowed_write', 'write', scratch / 'permitted-output', 0),
                 ('blocked_read', 'read', forbidden, 42),
                 ('blocked_write', 'write', forbidden, 42),
                 ('blocked_tcp', 'tcp', readable, 42),
                 ('blocked_unix', 'unix', endpoint, 42),
                 ('blocked_exec', 'exec', readable, 42)]
        rows = []
        for name, mode, path, expected in cases:
            result = run_bounded([str(SANDBOX), '-f', str(profile), str(binary), mode, str(path)], cwd=scratch)
            passed = result.drained and result.returncode == expected and result.outcome in ('completed', 'exit_failed') and result.stdout == b'rangoon.probe.v1\n'
            rows.append({'case': name, 'passed': passed, 'outcome': result.outcome,
                         'returncode': result.returncode, 'drained': result.drained,
                         'stdoutBytes': len(result.stdout), 'stderrBytes': len(result.stderr)})
        intact = _digest(forbidden) == before
        policy_intact = _digest(DYLD_SUPPORT) == DYLD_SUPPORT_SHA256
        output = scratch / 'permitted-output'
        output_valid = output.is_file() and output.read_bytes() == CANARY
        return {'schemaVersion': 'rangoon.target-isolation-selftest.v1',
                'outcome': 'passed' if all(row['passed'] for row in rows) and intact and policy_intact and output_valid else 'failed',
                'targetLaunched': False, 'cases': rows, 'blockedFixtureUnchanged': intact,
                'permittedOutputValid': output_valid, 'dyldPolicyUnchanged': policy_intact,
                'dyldPolicySha256': DYLD_SUPPORT_SHA256, 'probeSourceSha256': _digest(PROBE_SOURCE),
                'probeSha256': _digest(binary), 'policySha256': _digest(profile),
                'sandboxSha256': _digest(SANDBOX), 'compilerSha256': _digest(COMPILER),
                'platform': platform.platform(), 'scope': 'synthetic_canaries_only'}
    finally:
        if listener is not None:
            listener.close()
        current = root.lstat()
        if (current.st_dev, current.st_ino, current.st_uid) != (identity.st_dev, identity.st_ino, os.getuid()) or not stat.S_ISDIR(current.st_mode):
            raise RuntimeError('disposable fixture identity changed; cleanup refused')
        # Remove named generated files only. Unexpected entries remain for inspection.
        for path in (readable, forbidden, binary, profile, endpoint, scratch / 'permitted-output'):
            path.unlink(missing_ok=True)
        scratch.rmdir()
        root.rmdir()


def main(argv):
    if argv != ['--self-test']:
        print(json.dumps({'outcome': 'usage', 'message': 'expected: qualify-target-isolation.py --self-test'}))
        return 2
    try:
        result = self_test()
    except (OSError, RuntimeError, ValueError):
        result = {'schemaVersion': 'rangoon.target-isolation-selftest.v1', 'outcome': 'failed',
                  'targetLaunched': False, 'reason': 'selftest_infrastructure_failed'}
    print(json.dumps(result, separators=(',', ':')))
    return 0 if result['outcome'] == 'passed' else 3


if __name__ == '__main__':
    raise SystemExit(main(sys.argv[1:]))
