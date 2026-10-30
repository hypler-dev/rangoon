"""Offline supervisor/policy tests; no target binary, credentials or provider."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('target_isolation', Path(__file__).resolve().parents[1] / 'scripts/qualify-target-isolation.py')
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


@unittest.skipUnless(os.name == 'posix', 'POSIX process-group supervisor')
class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='rangoon-target-supervisor-')
        self.cwd = Path(self.directory.name).resolve()

    def tearDown(self):
        self.directory.cleanup()

    def child(self, source, **kwargs):
        return MODULE.run_bounded([sys.executable, '-I', '-S', '-c', source], cwd=self.cwd, **kwargs)

    def test_exact_output_and_exit_are_retained_only_with_drain(self):
        result = self.child("import os;os.write(1,b'good');os.write(2,b'notice')")
        self.assertEqual((result.outcome, result.returncode, result.drained), ('completed', 0, True))
        self.assertEqual((result.stdout, result.stderr), (b'good', b'notice'))
        failure = self.child('raise SystemExit(42)')
        self.assertEqual((failure.outcome, failure.returncode, failure.drained), ('exit_failed', 42, True))

    def test_exact_combined_output_limit_is_not_a_truncation_failure(self):
        result = self.child("import os;os.write(1,b'123');os.write(2,b'45')", byte_limit=5)
        self.assertEqual(result.outcome, 'completed')
        self.assertEqual(len(result.stdout) + len(result.stderr), 5)

    def test_combined_overflow_never_passes_or_grows_past_limit(self):
        for source in ["import os;os.write(1,b'x'*100000)",
                       "import os;os.write(2,b'x'*100000)",
                       "import os;os.write(1,b'x'*17);os.write(2,b'y'*17)"]:
            result = self.child(source, byte_limit=32)
            self.assertEqual(result.outcome, 'output_limit')
            self.assertTrue(result.drained)
            self.assertLessEqual(len(result.stdout) + len(result.stderr), 32)

    def test_silent_timeout_kills_and_drains(self):
        started = time.monotonic()
        result = self.child('import time;time.sleep(20)', timeout=0.15)
        self.assertEqual(result.outcome, 'timeout')
        self.assertTrue(result.drained)
        self.assertLess(time.monotonic() - started, 3)

    def test_closed_pipes_do_not_hide_a_still_running_child(self):
        result = self.child('import os,time;os.close(1);os.close(2);time.sleep(20)', timeout=0.15)
        self.assertEqual(result.outcome, 'timeout')
        self.assertTrue(result.drained)

    def test_leader_exit_with_descendant_holding_pipes_never_passes(self):
        source = "import os,subprocess,sys;subprocess.Popen([sys.executable,'-I','-S','-c','import time;time.sleep(20)']);os._exit(0)"
        started = time.monotonic()
        result = self.child(source, timeout=0.2)
        self.assertIn(result.outcome, ('timeout', 'drain_incomplete'))
        self.assertLess(time.monotonic() - started, 3)
        self.assertNotEqual(result.outcome, 'completed')

    def test_infrastructure_error_cannot_become_completed(self):
        result = MODULE.run_bounded(['/nonexistent/rangoon-probe'], cwd=self.cwd)
        self.assertEqual((result.outcome, result.returncode, result.stdout, result.stderr), ('spawn_failed', None, b'', b''))

    def test_no_ambient_environment_or_stdin_is_forwarded(self):
        with patch.dict(os.environ, {'RANGOON_SYNTHETIC_SECRET': 'test-only-value'}):
            result = self.child("import os,sys;assert 'RANGOON_SYNTHETIC_SECRET' not in os.environ;assert 'HOME' not in os.environ;assert 'CODEX_HOME' not in os.environ;assert sys.stdin.read()==''")
        self.assertEqual(result.outcome, 'completed')
        self.assertEqual(set(MODULE.clean_environment(self.cwd)), {'PATH', 'LANG', 'LC_ALL', 'TMPDIR', 'PYTHONDONTWRITEBYTECODE'})

    def test_interrupt_kills_and_reaps_owned_child(self):
        actual_popen = MODULE.subprocess.Popen
        children = []
        def popen(*args, **kwargs):
            process = actual_popen(*args, **kwargs)
            children.append(process)
            return process
        with patch.object(MODULE.subprocess, 'Popen', side_effect=popen), patch.object(MODULE.selectors.DefaultSelector, 'select', side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.child('import time;time.sleep(20)')
        self.assertEqual(len(children), 1)
        self.assertIsNotNone(children[0].poll())
        self.assertFalse(MODULE._group_exists(children[0].pid))

    def test_selector_construction_failure_happens_before_spawn(self):
        with patch.object(MODULE.selectors, 'DefaultSelector', side_effect=OSError('test-only selector failure')), patch.object(MODULE.subprocess, 'Popen') as spawn:
            with self.assertRaises(OSError):
                self.child('import time;time.sleep(20)')
            spawn.assert_not_called()

    def test_pipe_setup_failure_or_interrupt_cleans_owned_process(self):
        actual_popen = MODULE.subprocess.Popen
        for failure in (OSError('test-only pipe failure'), KeyboardInterrupt()):
            children = []
            def popen(*args, **kwargs):
                process = actual_popen(*args, **kwargs)
                children.append(process)
                return process
            with self.subTest(failure=type(failure).__name__), patch.object(MODULE.subprocess, 'Popen', side_effect=popen), patch.object(MODULE.os, 'set_blocking', side_effect=failure):
                with self.assertRaises(type(failure)):
                    self.child('import time;time.sleep(20)')
            self.assertEqual(len(children), 1)
            self.assertIsNotNone(children[0].poll())
            self.assertTrue(children[0].stdout.closed)
            self.assertTrue(children[0].stderr.closed)
            self.assertFalse(MODULE._group_exists(children[0].pid))

    def test_invalid_bounds_fail_before_spawn(self):
        for kwargs in [{'timeout': 0}, {'timeout': float('nan')}, {'timeout': float('inf')},
                       {'timeout': True}, {'timeout': 31}, {'byte_limit': 0}, {'byte_limit': True},
                       {'byte_limit': 1024 * 1024 + 1}]:
            with self.subTest(kwargs=kwargs), patch.object(MODULE.subprocess, 'Popen') as spawn:
                with self.assertRaises(ValueError):
                    MODULE.run_bounded([sys.executable], cwd=self.cwd, **kwargs)
                spawn.assert_not_called()

    def test_denied_group_termination_never_reports_completion(self):
        actual_killpg = MODULE.os.killpg
        def guarded(pid, sig):
            if sig == MODULE.signal.SIGKILL:
                raise PermissionError('synthetic denial')
            return actual_killpg(pid, sig)
        with patch.object(MODULE.os, 'killpg', side_effect=guarded):
            result = self.child('import time;time.sleep(20)', timeout=0.1)
        self.assertEqual(result.outcome, 'drain_incomplete')
        self.assertFalse(result.drained)


class PolicyTests(unittest.TestCase):
    def test_exact_probe_policy_does_not_allow_network_or_full_system_tree(self):
        with tempfile.TemporaryDirectory(prefix='rangoon-policy-') as tmp:
            root = Path(tmp).resolve()
            policy = MODULE.probe_policy(root / 'probe', root / 'input', root / 'scratch')
        self.assertIn('(deny default)', policy)
        self.assertEqual([line for line in policy.splitlines() if line.startswith('(import ')],
                         ['(import \"/System/Library/Sandbox/Profiles/dyld-support.sb\")'])
        self.assertIn('(allow process-exec (literal ', policy)
        self.assertNotIn('(allow network', policy)
        self.assertNotIn('(allow mach', policy)
        self.assertNotIn('(subpath "/System")', policy)
        self.assertNotIn('(allow process-fork', policy)
        self.assertIn('(allow file-map-executable (literal ', policy)
        mapping = [line for line in policy.splitlines() if line.startswith('(allow file-map-executable')]
        expected_map = '(allow file-map-executable (literal ' + MODULE._literal(root / "probe") + ') (subpath "/System/Library") (subpath "/usr/lib"))'
        self.assertEqual(mapping, [expected_map])
        self.assertNotIn(MODULE._literal(root / 'scratch'), mapping[0])

    def test_policy_rejects_relative_noncanonical_and_injectable_paths(self):
        for path in ['relative', '/tmp/../input', '/private/tmp/input" ) (allow default)', '/private/tmp/a\nb']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                MODULE.probe_policy(Path(path), Path('/private/tmp/input'), Path('/private/tmp/scratch'))

    def test_cli_has_no_target_launch_or_arbitrary_command_mode(self):
        for argv in [[], ['--help'], ['--self-test', '/bin/sh'], ['--run', '/bin/sh']]:
            with contextlib.redirect_stdout(io.StringIO()), patch.object(MODULE, 'self_test') as run:
                self.assertEqual(MODULE.main(argv), 2)
                run.assert_not_called()

    def test_unsupported_platform_is_unavailable_without_fallback(self):
        with patch.object(MODULE.platform, 'system', return_value='Linux'), patch.object(MODULE, 'run_bounded') as run:
            result = MODULE.self_test()
        self.assertEqual(result['outcome'], 'unavailable')
        self.assertFalse(result['targetLaunched'])
        run.assert_not_called()

    def test_unknown_os_bootstrap_policy_fails_before_build_or_launch(self):
        with patch.object(MODULE.platform, 'system', return_value='Darwin'), patch.object(MODULE.Path, 'is_file', return_value=True), patch.object(MODULE.Path, 'is_dir', return_value=True), patch.object(MODULE, '_digest', return_value='0' * 64), patch.object(MODULE, 'run_bounded') as run:
            result = MODULE.self_test()
        self.assertEqual(result['outcome'], 'unavailable')
        self.assertEqual(result['reason'], 'reviewed_dyld_policy_unavailable')
        run.assert_not_called()

    def test_cli_failure_receipt_excludes_arbitrary_exception_text(self):
        stream = io.StringIO()
        with patch.object(MODULE, 'self_test', side_effect=OSError('synthetic-private-marker')), contextlib.redirect_stdout(stream):
            self.assertEqual(MODULE.main(['--self-test']), 3)
        self.assertNotIn('synthetic-private-marker', stream.getvalue())
        self.assertIn('selftest_infrastructure_failed', stream.getvalue())


if __name__ == '__main__':
    unittest.main()
