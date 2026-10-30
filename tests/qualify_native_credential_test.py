#!/usr/bin/env python3
"""Offline C1-M runner tests. No native child, GUI, or provider is launched."""
import importlib.util
from copy import deepcopy
import io
import json
import os
from pathlib import Path
import queue
import stat
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).parents[1] / "scripts/qualify-native-credential.py"
SPEC = importlib.util.spec_from_file_location("native_credential_runner", SCRIPT)
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)


def rows(case="accept-read", nonce="a" * 32, pid=77):
    phases = ["native_review_queued", "native_review_accepted", "native_input_queued", "native_input_presented", "native_input_accepted", "read_returned"]
    events = [("ready", {"parentLabel": "c1-native-parent", "pid": pid, "context": Q.case_context(case, nonce)})]
    events += [("phase", {"phase": phase}) for phase in phases]
    events += [("result", {"status": "completed", "code": None, "observation": True,
                            "rpcCounts": {"health": 1, "policyStatus": 1, "logs": 1},
                            "authority": "none", "execution": "unavailable", "outcome": "unknown"}),
               ("exit", {"code": 0, "clean": True})]
    return [{"schema": Q.SCHEMA, "case": case, "nonce": nonce, "sequence": number,
             "event": event, "fields": fields} for number, (event, fields) in enumerate(events, 1)]


def stop_rows(case):
    phases = ["native_review_queued"] if case == "stop-queued" else ["native_review_queued", "native_review_accepted", "native_input_queued", "native_input_presented"]
    events = [("ready", {"parentLabel": "c1-native-parent", "pid": 77, "context": Q.case_context(case, "a" * 32)})]
    if case == "stop-queued":
        events += [("main_held", {}), ("phase", {"phase": phases[0]}), ("command_received", {"command": "stop"}), ("stopping", {"status":"closing", "admitted":True}), ("command_received", {"command":"release-main"}), ("main_released", {})]
    else:
        events += [("phase", {"phase": phase}) for phase in phases]
        events += [("command_received", {"command":"hold-main"}), ("main_held", {}), ("command_received", {"command":"stop"}), ("stopping", {"status":"closing", "admitted":True}), ("command_received", {"command":"release-main"}), ("main_released", {})]
    events += [("result", {"status":"cancelled", "code":"cancelled", "observation":False, "rpcCounts":{"health":0,"policyStatus":0,"logs":0}, "authority":"none", "execution":"unavailable", "outcome":"unknown"}), ("exit", {"code":0,"clean":True})]
    return [{"schema":Q.SCHEMA,"case":case,"nonce":"a" * 32,"sequence":number,"event":event,"fields":fields} for number,(event,fields) in enumerate(events,1)]


class EventTests(unittest.TestCase):
    def test_complete_accept_read_passes(self):
        self.assertEqual(len(Q.verify_events("accept-read", "a" * 32, 77, rows())), 9)

    def test_nonce_unordered_duplicate_and_redaction_refuse(self):
        for changed in ("nonce", "case", "unordered", "duplicate", "redaction", "bool"):
            candidate = rows()
            if changed == "nonce": candidate[0]["nonce"] = "b" * 32
            elif changed == "case": candidate[0]["case"] = "cancel-review"
            elif changed == "unordered": candidate[2]["fields"] = {"phase": "native_input_queued"}
            elif changed == "duplicate": candidate.insert(-1, {**candidate[-2], "sequence": 8}); candidate[-1]["sequence"] = 9
            elif changed == "redaction": candidate[0]["fields"]["token"] = "no"
            else: candidate[-1]["fields"] = {"code": False, "clean": 1}
            with self.subTest(changed=changed), self.assertRaises(Q.QualificationError):
                Q.verify_events("accept-read", "a" * 32, 77, candidate)

    def test_duplicate_json_keys_refuse(self):
        with self.assertRaises(Q.DuplicateKeyError):
            json.loads('{"event":"ready","event":"result"}', object_pairs_hook=Q.no_duplicate_object)

    def test_expiry_requires_parent_elapsed_and_no_input_accept(self):
        candidate = rows("expire-open")
        candidate = [row for row in candidate if row["event"] not in {"phase", "result"}]
        candidate[0:1] += []
        phases = ["native_review_queued", "native_review_accepted", "native_input_queued", "native_input_presented"]
        body = [{"schema": Q.SCHEMA, "case": "expire-open", "nonce": "a" * 32, "sequence": 0, "event": "phase", "fields": {"phase": phase}} for phase in phases]
        result = {"schema": Q.SCHEMA, "case": "expire-open", "nonce": "a" * 32, "sequence": 0, "event": "result", "fields": {"status": "error", "code": "credential_lease_expired", "observation": False, "rpcCounts": {"health": 0, "policyStatus": 0, "logs": 0}, "authority": "none", "execution": "unavailable", "outcome": "unknown"}}
        candidate = [candidate[0], *body, result, candidate[-1]]
        for number, row in enumerate(candidate, 1): row["sequence"] = number
        with self.assertRaises(Q.QualificationError): Q.verify_events("expire-open", "a" * 32, 77, candidate, 118.9)
        self.assertEqual(len(Q.verify_events("expire-open", "a" * 32, 77, candidate, 119)), 7)

    def test_stop_case_orders_and_required_phases(self):
        for case in ("stop-queued", "stop-open"):
            candidate = stop_rows(case)
            with self.subTest(case=case): self.assertEqual(len(Q.verify_events(case, "a" * 32, 77, candidate)), len(candidate))
            missing = [row for row in candidate if not (row["event"] == "phase" and row["fields"]["phase"] == "native_review_queued")]
            for number, row in enumerate(missing, 1): row["sequence"] = number
            with self.subTest(case=case, missing=True), self.assertRaises(Q.QualificationError): Q.verify_events(case, "a" * 32, 77, missing)
        reordered = stop_rows("stop-open")
        held, input_presented = next(i for i,row in enumerate(reordered) if row["event"] == "main_held"), next(i for i,row in enumerate(reordered) if row["event"] == "phase" and row["fields"]["phase"] == "native_input_presented")
        reordered[held], reordered[input_presented] = reordered[input_presented], reordered[held]
        for number, row in enumerate(reordered, 1): row["sequence"] = number
        with self.assertRaises(Q.QualificationError): Q.verify_events("stop-open", "a" * 32, 77, reordered)

    def test_open_negative_cases_refuse_queued_only_late_and_parent_preopen(self):
        def numbered(candidate):
            for number, row in enumerate(candidate, 1):
                row["sequence"] = number
            return candidate

        parent = None
        for case in ("stop-open", "parent-close", "expire-open"):
            candidate = stop_rows("stop-open")
            for row in candidate:
                row["case"] = case
                if row["event"] == "ready": row["fields"]["context"] = Q.case_context(case, "a" * 32)
            if case != "stop-open":
                candidate = [row for row in candidate if row["event"] in {"ready", "phase", "result", "exit"}]
            result_index = next(i for i, row in enumerate(candidate) if row["event"] == "result")
            if case == "parent-close":
                candidate.insert(result_index, {"schema": Q.SCHEMA, "case": case, "nonce": "a" * 32, "sequence": 0, "event": "command_received", "fields": {"command": "close-parent"}})
            if case == "expire-open":
                candidate[result_index]["fields"].update(status="error", code="credential_lease_expired")
            numbered(candidate)
            # Positive control proves each negative starts from a valid case;
            # identity/order failures cannot mask its intended phase refusal.
            with self.subTest(case=case, valid=True):
                self.assertEqual(len(Q.verify_events(case, "a" * 32, 77, candidate, 119)), len(candidate))
            if case == "parent-close":
                parent = deepcopy(candidate)
            queued_only = numbered([row for row in deepcopy(candidate) if not (row["event"] == "phase" and row["fields"]["phase"] == "native_input_presented")])
            with self.subTest(case=case, queued_only=True), self.assertRaisesRegex(Q.QualificationError, "native input phase evidence incomplete"):
                Q.verify_events(case, "a" * 32, 77, queued_only, 119)
            for extra in (("native_input_accepted",), ("native_input_accepted", "read_returned")):
                late = deepcopy(candidate)
                insert = next(i for i, row in enumerate(late) if row["event"] == "result")
                late[insert:insert] = [{"schema": Q.SCHEMA, "case": case, "nonce": "a" * 32, "sequence": 0, "event": "phase", "fields": {"phase": phase}} for phase in extra]
                numbered(late)
                with self.subTest(case=case, late=extra), self.assertRaisesRegex(Q.QualificationError, "native input phase evidence incomplete"):
                    Q.verify_events(case, "a" * 32, 77, late, 119)
        close = next(i for i, row in enumerate(parent) if row["event"] == "command_received")
        presented = next(i for i, row in enumerate(parent) if row["event"] == "phase" and row["fields"]["phase"] == "native_input_presented")
        parent[close], parent[presented] = parent[presented], parent[close]
        numbered(parent)
        with self.assertRaisesRegex(Q.QualificationError, "parent close command missing"):
            Q.verify_events("parent-close", "a" * 32, 77, parent)


class SourceAndPinTests(unittest.TestCase):
    def test_untracked_authored_file_is_hashed(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve(); (repo / ".git").mkdir()
            for relative in ("tracked.rs", "new/native.rs", Q.CONFIG, Q.CASE_PLAN, Q.RUNNER):
                path = repo / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(relative)
            completed = subprocess.CompletedProcess(["git"], 0, b"tracked.rs\0new/native.rs\0")
            with patch.object(Q, "exact_root", return_value=repo), patch.object(Q.subprocess, "run", return_value=completed) as run:
                manifest = Q.source_manifest(repo)
            self.assertIn("new/native.rs", manifest["files"])
            self.assertIn("--cached", run.call_args.args[0]); self.assertIn("--others", run.call_args.args[0]); self.assertIn("--exclude-standard", run.call_args.args[0])

    def test_pinned_env_refusal_stops_before_cargo(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory); (repo / "scripts").mkdir(); (repo / "scripts/check-source-rust.mjs").write_text("")
            refused = subprocess.CompletedProcess(["node"], 1, b"", b"wrapper refused")
            with patch.object(Q.shutil, "which", return_value="/usr/bin/node"), patch.object(Q.subprocess, "run", return_value=refused):
                with self.assertRaises(Q.QualificationError): Q.toolchain_record(repo, Path("/tmp/rust/bin"))

    def test_coherent_wrong_pin_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory); (repo / "scripts").mkdir(); (repo / "scripts/check-source-rust.mjs").write_text("")
            tools = {key: f"/tmp/rust/bin/{key}" for key in Q.TOOL_KEYS}
            payload = {"pin":"1.100.0","bin":"/tmp/rust/bin","tools":tools,
                       "env":{"CARGO":tools["cargo"],"RUSTC_WRAPPER":"","RUSTC_WORKSPACE_WRAPPER":""},
                       "identities":["identity"] * 7}
            result = subprocess.CompletedProcess(["node"], 0, json.dumps(payload).encode(), b"")
            with patch.object(Q.shutil, "which", return_value="/usr/bin/node"), patch.object(Q.subprocess, "run", return_value=result):
                with self.assertRaises(Q.QualificationError): Q.toolchain_record(repo, Path("/tmp/rust/bin"))

    def test_node_import_does_not_invoke_helper_cli_entrypoint(self):
        node = Q.shutil.which("node")
        self.assertIsNotNone(node, "Node is required by native qualification")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            for relative in ("rust-toolchain.toml", "Cargo.toml", "apps/desktop/Cargo.toml", "apps/desktop/native-input/Cargo.toml"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture")
            helper = root / "scripts/check-source-rust.mjs"
            helper.parent.mkdir()
            helper.write_text("""import { fileURLToPath } from 'node:url';
if (process.argv[1] === fileURLToPath(import.meta.url)) throw new Error('CLI entrypoint invoked during import');
export const validateConfiguration = () => ({ pin: '1.99.1', clippyVersion: '0.1.99' });
export const resolveToolchain = ({toolchainBin}) => ({bin: toolchainBin, tools: Object.fromEntries(['cargo','rustc','rustdoc','cargo_fmt','rustfmt','cargo_clippy','clippy_driver'].map(key => [key, `${toolchainBin}/${key}`]))});
export const prepareChildEnv = ({tools}) => ({CARGO: tools.cargo, RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: ''});
export const verifyToolIdentities = () => { for (let i = 0; i < 7; i++) console.log('fixture identity'); };
""")
            record = Q.toolchain_record(root, root / "tools", node=node)
            self.assertEqual(record["pin"], "1.99.1")
            self.assertEqual(len(record["identities"]), 7)
            self.assertEqual(record["env"]["CARGO"], str(root / "tools/cargo"))

    def test_wrong_target_and_artifact_hash_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); executable = root / "apps/desktop/target/debug/deps/native_credential_qualification-acde"
            executable.parent.mkdir(parents=True); executable.write_bytes(b"INERT native fixture")
            record = {"reason":"compiler-artifact","target":{"name":Q.TARGET,"kind":["test"]},"executable":str(executable)}
            self.assertEqual(Q.select_artifact(root, [record]), executable.resolve())
            record["target"]["name"] = "ordinary-binary"
            with self.assertRaises(Q.QualificationError): Q.select_artifact(root, [record])
            artifact = {"sha256":Q.sha256_file(executable),"executable":str(executable)}
            evidence = root / "evidence"; evidence.mkdir()
            wrapped, launch = Q.prepare_native_bundle(executable, artifact, evidence)
            Q.verify_native_bundle(wrapped, launch, evidence)
            executable.write_bytes(b"INERT source tamper")
            with patch.object(Q, "capture_child") as capture:
                with self.assertRaisesRegex(Q.QualificationError, "compiled artifact changed"):
                    Q.execute_case(root, wrapped, launch, {}, evidence, "accept-read", 1)
                capture.assert_not_called()
            executable.write_bytes(b"INERT native fixture")
            copied_evidence = root / "copied-evidence"; copied_evidence.mkdir()
            copied, copied_launch = Q.prepare_native_bundle(executable, {"sha256":Q.sha256_file(executable),"executable":str(executable)}, copied_evidence)
            Q.verify_native_bundle(copied, copied_launch, copied_evidence)
            copied.write_bytes(b"INERT copied tamper")
            with patch.object(Q, "capture_child") as capture:
                with self.assertRaisesRegex(Q.QualificationError, "native bundle executable changed"):
                    Q.execute_case(root, copied, copied_launch, {}, copied_evidence, "accept-read", 1)
                capture.assert_not_called()


class NativeBundleTests(unittest.TestCase):
    def make_bundle(self, root):
        source = root / "target" / "native_credential_qualification-acde"
        source.parent.mkdir(parents=True)
        source.write_bytes(b"INERT native credential fixture\\x00")
        artifact = {"sha256":Q.sha256_file(source),"executable":str(source)}
        evidence = root / "evidence"; evidence.mkdir()
        return source, *Q.prepare_native_bundle(source, artifact, evidence), evidence

    def assert_bundle_valid(self, wrapped, launch, evidence):
        self.assertIsNone(Q.verify_native_bundle(wrapped, launch, evidence))

    def test_prepare_fresh_bundle_preserves_inert_bytes_plist_modes_and_launch_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            source, wrapped, launch, evidence = self.make_bundle(Path(directory).resolve())
            info = wrapped.parent.parent / "Info.plist"
            self.assert_bundle_valid(wrapped, launch, evidence)
            self.assertEqual(wrapped.read_bytes(), source.read_bytes())
            self.assertEqual(Q.sha256_file(wrapped), launch["sha256"])
            self.assertEqual(info.read_bytes(), Q.bundle_info())
            self.assertEqual(stat.S_IMODE(wrapped.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE(info.stat().st_mode), 0o600)
            self.assertEqual(json.loads((evidence / "launch-artifact.json").read_text()), launch)

    def test_prepare_refuses_changed_source_and_existing_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); source = root / "native"
            source.write_bytes(b"INERT source")
            artifact = {"sha256":Q.sha256_file(source),"executable":str(source)}
            source.write_bytes(b"INERT source changed")
            evidence = root / "changed"; evidence.mkdir()
            with self.assertRaisesRegex(Q.QualificationError, "compiled artifact changed"):
                Q.prepare_native_bundle(source, artifact, evidence)
            source.write_bytes(b"INERT source")
            artifact["sha256"] = Q.sha256_file(source)
            existing = root / "existing"; existing.mkdir(); (existing / Q.BUNDLE_NAME).mkdir()
            with self.assertRaisesRegex(Q.QualificationError, "disposable native bundle creation failed"):
                Q.prepare_native_bundle(source, artifact, existing)

    def test_bundle_tampering_refuses_after_valid_control(self):
        cases = ("source-bytes", "copied-bytes", "plist", "launch-identity", "executable-symlink", "metadata-symlink", "directory-symlink")
        for changed in cases:
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory:
                source, wrapped, launch, evidence = self.make_bundle(Path(directory).resolve())
                info = wrapped.parent.parent / "Info.plist"
                self.assert_bundle_valid(wrapped, launch, evidence)
                if changed == "source-bytes":
                    source.write_bytes(b"INERT changed source")
                    expected = "compiled artifact changed"
                elif changed == "copied-bytes":
                    wrapped.write_bytes(b"INERT changed copy")
                    expected = "native bundle executable changed"
                elif changed == "plist":
                    info.write_bytes(b"INERT changed plist")
                    expected = "native bundle metadata changed"
                elif changed == "launch-identity":
                    launch["launchScope"] = "product-package"
                    expected = "invalid native bundle identity"
                elif changed == "executable-symlink":
                    wrapped.unlink(); wrapped.symlink_to(source)
                    expected = "native bundle requires exact regular paths"
                elif changed == "metadata-symlink":
                    info.unlink(); info.symlink_to(source)
                    expected = "native bundle requires exact regular paths"
                else:
                    macos = wrapped.parent; moved = macos.with_name("safe-macos")
                    macos.rename(moved); macos.symlink_to(moved, target_is_directory=True)
                    expected = "native bundle requires exact regular paths"
                with self.assertRaisesRegex(Q.QualificationError, expected):
                    Q.verify_native_bundle(wrapped, launch, evidence)

    def test_bundle_io_failure_is_fixed_and_refuses_before_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source, wrapped, launch, evidence = self.make_bundle(root)
            self.assert_bundle_valid(wrapped, launch, evidence)
            with patch.object(Q, "sha256_file", side_effect=OSError("untrusted path or payload")), patch.object(Q, "capture_child") as capture:
                with self.assertRaisesRegex(Q.QualificationError, "^native bundle inspection failed$"):
                    Q.execute_case(root, wrapped, launch, {}, evidence, "accept-read", 1)
                capture.assert_not_called()
                with self.assertRaisesRegex(Q.QualificationError, "^disposable native bundle creation failed$"):
                    Q.prepare_native_bundle(source, {"executable":str(source)}, evidence)

    def test_execute_case_refuses_tampered_bundle_before_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); source, wrapped, launch, evidence = self.make_bundle(root)
            self.assert_bundle_valid(wrapped, launch, evidence)
            wrapped.write_bytes(b"INERT changed copy")
            with patch.object(Q, "capture_child") as capture:
                with self.assertRaisesRegex(Q.QualificationError, "native bundle executable changed"):
                    Q.execute_case(root, wrapped, launch, {}, evidence, "accept-read", 1)
                capture.assert_not_called()

    def test_execute_case_rechecks_bundle_after_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); source, wrapped, launch, evidence = self.make_bundle(root)
            manifest = {"files":{"fixture":"hash"}}
            capture = {"pid":77,"code":0,"rows":rows(),"stderrPresent":False,"elapsed":1,
                       "stdoutEof":True,"stderrEof":True,"timedOut":False,"stdoutOverflow":False,
                       "stderrOverflow":False,"cleanupFailure":False,"eventFailure":False,"commandFailure":False}
            def tamper_after_capture(*_args, **_kwargs):
                wrapped.write_bytes(b"INERT changed after capture")
                return capture
            self.assert_bundle_valid(wrapped, launch, evidence)
            pump = Q.CommandPump(io.StringIO("")); pump.thread.join(1)
            with patch.object(Q, "capture_child", side_effect=tamper_after_capture) as child, patch.object(Q, "source_manifest", return_value=manifest), patch.object(Q.secrets, "token_hex", return_value="a" * 32), patch.object(Q.CaseController, "start"):
                with self.assertRaisesRegex(Q.QualificationError, "native bundle executable changed"):
                    Q.execute_case(root, wrapped, launch, manifest, evidence, "accept-read", 1, pump)
                child.assert_called_once()

    def test_missing_or_wrong_start_refuses_before_child_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); _, wrapped, launch, evidence = self.make_bundle(root)
            with patch.object(Q, "capture_child") as capture:
                with self.assertRaisesRegex(Q.QualificationError, "controller required before launch"):
                    Q.execute_case(root, wrapped, launch, {}, evidence, "accept-read", 1)
                pump = Q.CommandPump(io.StringIO(json.dumps({"case":"accept-read","nonce":"a" * 32,"kind":"start","challenge":"b" * 32}) + "\n")); pump.thread.join(1)
                with patch.object(Q.secrets, "token_hex", return_value="a" * 32):
                    with self.assertRaisesRegex(Q.QualificationError, "start challenge mismatch"):
                        Q.execute_case(root, wrapped, launch, {}, evidence, "accept-read", 1, pump)
                capture.assert_not_called()

    def test_nonzero_child_exit_never_reaches_terminal_handoff(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); _, wrapped, launch, evidence = self.make_bundle(root)
            manifest = {"files":{"fixture":"hash"}}
            capture = {"pid":77,"code":70,"rows":rows(),"stderrPresent":True,"elapsed":1,
                       "stdoutEof":True,"stderrEof":True}
            pump = Q.CommandPump(io.StringIO("")); pump.thread.join(1)
            with patch.object(Q.CaseController,"start"), patch.object(Q.CaseController,"handoff") as handoff, patch.object(Q,"source_manifest",return_value=manifest), patch.object(Q,"capture_child",return_value=capture):
                with self.assertRaisesRegex(Q.QualificationError,"child capture failed"):
                    Q.execute_case(root,wrapped,launch,manifest,evidence,"accept-read",1,pump)
                handoff.assert_not_called()

    def test_execute_capture_relay_and_terminal_succeed_with_inert_native_double(self):
        """Real parent threads/protocol; the child is not a process or UI."""
        class Lines:
            def __init__(self): self.items = queue.Queue()
            def readline(self, _bound=None): return self.items.get()

        class InertChild:
            pid, returncode = 91, None
            def __init__(self):
                self.stdin, self.stdout, self.stderr = io.BytesIO(), Lines(), io.BytesIO()
                self.done = threading.Event()
            def poll(self): return self.returncode
            def wait(self, timeout=None):
                if not self.done.wait(timeout): raise Q.subprocess.TimeoutExpired("inert-child",timeout)
                return self.returncode
            def kill(self):
                self.returncode = -9; self.stdout.items.put(b""); self.done.set()

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); _, wrapped, artifact, evidence = self.make_bundle(root)
            manifest = {"files":{"fixture":"hash"}}; incoming = Lines(); pump = Q.CommandPump(incoming)
            child = InertChild(); metadata = {}; sequence = 0
            write_json, receive, native = Q.write_json_new, Q.CaseController.receive, Q.CaseController.native
            def send(record): incoming.items.put(json.dumps(record)+"\n")
            def emit(event, fields):
                nonlocal sequence
                sequence += 1
                child.stdout.items.put((json.dumps({"schema":Q.SCHEMA,"case":"accept-read","nonce":metadata["nonce"],"sequence":sequence,"event":event,"fields":fields})+"\n").encode())
            def write(path, value):
                write_json(path,value)
                if path.name.endswith(".start.json"):
                    send({key:value[key] for key in ("case","nonce","kind","challenge")})
                elif path.name.endswith(".launch.json"):
                    metadata.update(value)
                    emit("ready",{"parentLabel":"c1-native-parent","pid":91,"context":Q.case_context("accept-read",value["nonce"])})
                    emit("phase",{"phase":"native_review_queued"})
                elif path.name.endswith(".terminal.json"): send(value)
            def intent(action, digit):
                send({"case":"accept-read","nonce":metadata["nonce"],"kind":"intent","id":digit * 32,"action":action,"pid":91,"artifactSha256":artifact["sha256"],"context":Q.case_context("accept-read",metadata["nonce"])})
            def observe(controller, row):
                native(controller,row)
                if row["event"]=="phase" and row["fields"]["phase"]=="native_review_queued": intent("review-continue","1")
                if row["event"]=="phase" and row["fields"]["phase"]=="native_input_presented": intent("input-fill","2")
            def dispatch(controller, record):
                value = receive(controller,record)
                if record.get("kind")=="intent":
                    send({"case":"accept-read","nonce":metadata["nonce"],"kind":"return","id":record["id"],"outcome":"dispatched"})
                    if record["action"]=="review-continue":
                        for phase in ("native_review_accepted","native_input_queued","native_input_presented"): emit("phase",{"phase":phase})
                    elif record["action"]=="input-fill": intent("input-submit","3")
                    elif record["action"]=="input-submit":
                        emit("phase",{"phase":"native_input_accepted"}); emit("phase",{"phase":"read_returned"})
                        emit("result",deepcopy(rows()[-2]["fields"])); emit("exit",{"code":0,"clean":True})
                        child.stdout.items.put(b""); child.returncode = 0; child.done.set()
                return value
            def launch(*args, **kwargs):
                self.assertEqual(Path(args[0][0]),wrapped)
                journal = [json.loads(line) for line in (evidence/"accept-read.controller.jsonl").read_text().splitlines()]
                self.assertEqual(journal[-1]["kind"],"start-accepted")
                return child
            try:
                with patch.object(Q,"write_json_new",side_effect=write), patch.object(Q.CaseController,"receive",dispatch), patch.object(Q.CaseController,"native",observe), patch.object(Q,"source_manifest",return_value=manifest), patch.object(Q.subprocess,"Popen",side_effect=launch) as popen:
                    result = Q.execute_case(root,wrapped,artifact,manifest,evidence,"accept-read",2,pump)
                self.assertTrue(result["parentOraclePassed"])
                self.assertTrue(result["controllerCorrelationComplete"])
                popen.assert_called_once()
                self.assertFalse((evidence/"accept-read.failure.json").exists())
                records = [json.loads(line) for line in (evidence/"accept-read.controller.jsonl").read_text().splitlines()]
                self.assertEqual(records[-1]["kind"],"handoff-accepted")
                self.assertEqual(len([row for row in records if row["kind"]=="native"]),9)
                self.assertEqual([row["sequence"] for row in records],list(range(1,len(records)+1)))
            finally:
                incoming.items.put(""); pump.thread.join(1)
                self.assertFalse(pump.thread.is_alive())


class ControllerTests(unittest.TestCase):
    """Exercise the actual private journal/oracle, without a native process."""
    def controller(self, case="accept-read", attach=True):
        directory = tempfile.TemporaryDirectory(); self.addCleanup(directory.cleanup)
        pump = Q.CommandPump(io.StringIO("")); pump.thread.join(1)
        controller = Q.CaseController(case, "a" * 32, {"sha256":"f" * 64}, Path(directory.name), pump)
        self.addCleanup(controller.close)
        if attach: controller.attach(77)
        return controller

    def event(self, controller, event, fields):
        row = {"schema":Q.SCHEMA,"case":controller.case,"nonce":controller.nonce,
               "sequence":controller.native_count + 1,"event":event,"fields":fields}
        controller.native(row)
        return row

    def ready(self, controller):
        self.event(controller, "ready", {"parentLabel":"c1-native-parent","pid":77,
                                        "context":Q.case_context(controller.case, controller.nonce)})
        self.event(controller, "phase", {"phase":"native_review_queued"})

    def intent(self, controller, action, digit="1"):
        record = {"case":controller.case,"nonce":controller.nonce,"kind":"intent","id":digit * 32,
                  "action":action,"pid":77,"artifactSha256":"f" * 64,
                  "context":Q.case_context(controller.case, controller.nonce)}
        controller.receive(record)
        return record

    def returned(self, controller, record, outcome="dispatched"):
        controller.receive({"case":controller.case,"nonce":controller.nonce,"kind":"return",
                            "id":record["id"],"outcome":outcome})

    def presented(self, controller):
        self.ready(controller)
        intent = self.intent(controller, "review-continue")
        for phase in ("native_review_accepted", "native_input_queued", "native_input_presented"):
            self.event(controller, "phase", {"phase":phase})
        self.returned(controller, intent)

    def result(self, controller):
        result = deepcopy(rows()[-2]["fields"])
        if controller.case != "accept-read":
            result.update(status="cancelled", code="cancelled", observation=False,
                          rpcCounts={"health":0,"policyStatus":0,"logs":0})
        self.event(controller, "result", result)
        self.event(controller, "exit", {"code":0,"clean":True})

    def completed(self):
        controller = self.controller(); self.presented(controller)
        fill = self.intent(controller, "input-fill", "2"); self.returned(controller, fill)
        submit = self.intent(controller, "input-submit", "3")
        for phase in ("native_input_accepted", "read_returned"):
            self.event(controller, "phase", {"phase":phase})
        self.result(controller)
        return controller, submit

    def test_unexplained_review_progress_refuses_v9_pattern_live(self):
        controller = self.controller("stop-open"); self.ready(controller)
        with self.assertRaisesRegex(Q.QualificationError, "unattributed native transition"):
            self.event(controller, "phase", {"phase":"native_review_accepted"})
        self.assertEqual(controller.phases, ["native_review_queued"])
        self.assertNotIn("native_review_accepted", controller.path.read_text())

    def test_unexplained_input_submit_and_terminal_refuse(self):
        controller = self.controller(); self.presented(controller)
        with self.assertRaisesRegex(Q.QualificationError, "unattributed native transition"):
            self.event(controller, "phase", {"phase":"native_input_accepted"})
        with self.assertRaisesRegex(Q.QualificationError, "unattributed native result"):
            self.result(controller)

    def test_intent_precedes_native_progress_and_return_may_follow_exit(self):
        controller, submit = self.completed()
        self.assertTrue(controller.exited)
        with self.assertRaisesRegex(Q.QualificationError, "controller return timed out"):
            controller.settle_returns(0)
        self.returned(controller, submit)
        controller.settle_returns()
        records = [json.loads(line) for line in controller.path.read_text().splitlines()]
        submit_index = next(i for i,row in enumerate(records) if row["kind"]=="action-intent" and row["action"]=="input-submit")
        native_index = next(i for i,row in enumerate(records) if row["kind"]=="native" and row["event"].get("fields")=={"phase":"native_input_accepted"})
        self.assertLess(submit_index, native_index)
        self.assertEqual(records[-1]["kind"], "action-return")

    def test_post_exit_return_tolerates_transport_delay_without_new_actions(self):
        controller, submit = self.completed()
        def delayed_return(_seconds):
            with self.assertRaisesRegex(Q.QualificationError, "controller action after terminal"):
                self.intent(controller, "input-fill", "4")
            self.returned(controller, submit)
        with patch.object(Q.time, "monotonic", side_effect=[0, 11, 12]), patch.object(Q.time, "sleep", side_effect=delayed_return):
            controller.settle_returns()
        self.assertIsNone(controller.pending)
        self.assertTrue(controller.exited)

    def test_missing_post_exit_return_still_expires_at_bounded_deadline(self):
        controller, _submit = self.completed()
        with patch.object(Q.time, "monotonic", side_effect=[0, 59, 60]), patch.object(Q.time, "sleep"), self.assertRaisesRegex(Q.QualificationError, "controller return timed out"):
            controller.settle_returns()
        self.assertIsNotNone(controller.pending)

    def test_wrong_phase_duplicate_pending_and_reused_id_refuse(self):
        controller = self.controller(); self.ready(controller)
        with self.assertRaisesRegex(Q.QualificationError, "controller action phase mismatch"):
            self.intent(controller, "input-fill")
        review = self.intent(controller, "review-continue")
        with self.assertRaisesRegex(Q.QualificationError, "controller action out of order"):
            self.intent(controller, "review-continue", "2")
        self.returned(controller, review)
        with self.assertRaisesRegex(Q.QualificationError, "controller action binding mismatch"):
            self.intent(controller, "review-continue", "2")
        for phase in ("native_review_accepted", "native_input_queued", "native_input_presented"):
            self.event(controller, "phase", {"phase":phase})
        with self.assertRaisesRegex(Q.QualificationError, "controller action binding mismatch"):
            self.intent(controller, "input-fill", "1")

    def test_failed_interfered_and_wrong_action_returns_refuse(self):
        for outcome in ("failed", "interference"):
            with self.subTest(outcome=outcome):
                controller = self.controller(); self.ready(controller)
                intent = self.intent(controller, "review-continue")
                with self.assertRaisesRegex(Q.QualificationError, "controller action refused"):
                    self.returned(controller, intent, outcome)
                controller.refuse()
                with self.assertRaisesRegex(Q.QualificationError, "controller action after terminal"):
                    self.returned(controller, intent)
        controller = self.controller(); self.ready(controller)
        intent = self.intent(controller, "review-continue")
        with self.assertRaisesRegex(Q.QualificationError, "controller return mismatch"):
            self.returned(controller, {**intent,"id":"e" * 32})

    def test_context_pid_hash_and_cross_case_binding_refuse(self):
        for field, value in (("context","c1-other"),("pid",78),("artifactSha256","e" * 64),
                             ("case","stop-open"),("nonce","b" * 32)):
            with self.subTest(field=field):
                controller = self.controller(); self.ready(controller)
                record = {"case":controller.case,"nonce":controller.nonce,"kind":"intent","id":"1" * 32,
                          "action":"review-continue","pid":77,"artifactSha256":"f" * 64,
                          "context":Q.case_context(controller.case, controller.nonce)}
                record[field] = value
                with self.assertRaises(Q.QualificationError): controller.receive(record)
                self.assertEqual(controller.actions, {})

    def test_native_context_duplicate_and_after_terminal_refuse(self):
        controller = self.controller()
        with self.assertRaisesRegex(Q.QualificationError, "invalid ready event"):
            self.event(controller,"ready",{"parentLabel":"c1-native-parent","pid":77,"context":"c1-wrong"})
        self.ready(controller)
        with self.assertRaisesRegex(Q.QualificationError, "native sequence mismatch"):
            self.event(controller,"ready",{"parentLabel":"c1-native-parent","pid":77,"context":Q.case_context(controller.case,controller.nonce)})
        complete, submit = self.completed(); self.returned(complete, submit)
        with self.assertRaisesRegex(Q.QualificationError,"native event after terminal"):
            self.event(complete,"exit",{"code":0,"clean":True})
        with self.assertRaisesRegex(Q.QualificationError,"controller action after terminal"):
            self.intent(complete,"input-fill","4")

    def test_controls_require_actual_visible_and_held_phases(self):
        controller = self.controller("stop-open"); self.ready(controller)
        control = lambda command: {"case":controller.case,"nonce":controller.nonce,"command":command}
        with self.assertRaisesRegex(Q.QualificationError,"controller control before input"):
            controller.receive(control("hold-main"))
        with self.assertRaisesRegex(Q.QualificationError,"unattributed native control"):
            self.event(controller,"command_received",{"command":"hold-main"})
        # Use a fresh control after the refused sequence, not repaired evidence.
        controller = self.controller("stop-open"); self.presented(controller)
        fill = self.intent(controller,"input-fill","2"); self.returned(controller,fill)
        self.assertEqual(controller.receive(control("hold-main")),"hold-main")
        self.event(controller,"command_received",{"command":"hold-main"})
        with self.assertRaisesRegex(Q.QualificationError,"controller control before custody"):
            controller.receive(control("stop"))
        self.event(controller,"main_held",{})
        self.assertEqual(controller.receive(control("stop")),"stop")
        self.event(controller,"command_received",{"command":"stop"})
        self.event(controller,"stopping",{"status":"closing","admitted":True})
        self.assertEqual(controller.receive(control("release-main")),"release-main")
        self.event(controller,"command_received",{"command":"release-main"})
        self.event(controller,"main_released",{})
        self.result(controller)

    def test_start_and_handoff_challenges_require_current_exact_identity(self):
        controller = self.controller()
        with patch.object(controller,"next_record",return_value={"case":controller.case,"nonce":controller.nonce,"kind":"start","challenge":"b" * 32}), patch.object(Q.secrets,"token_hex",return_value="b" * 32):
            controller.start()
        with self.assertRaisesRegex(Q.QualificationError,"unclean controller terminal"):
            controller.handoff()
        controller, submit = self.completed(); self.returned(controller,submit)
        expected = {"case":controller.case,"nonce":controller.nonce,"kind":"handoff","pid":77,
                    "artifactSha256":"f" * 64,"terminalId":"c" * 32}
        with patch.object(controller,"next_record",return_value=expected), patch.object(Q.secrets,"token_hex",return_value="c" * 32):
            controller.handoff()
        self.assertEqual(json.loads(controller.path.read_text().splitlines()[-1])["kind"],"handoff-accepted")

    def test_wrong_handoff_pid_nonce_hash_and_challenge_refuse(self):
        for field,value in (("pid",78),("nonce","b" * 32),("artifactSha256","e" * 64),("terminalId","d" * 32)):
            with self.subTest(field=field):
                controller, submit = self.completed(); self.returned(controller,submit)
                record = {"case":controller.case,"nonce":controller.nonce,"kind":"handoff","pid":77,
                          "artifactSha256":"f" * 64,"terminalId":"c" * 32}
                record[field] = value
                with patch.object(controller,"next_record",return_value=record), patch.object(Q.secrets,"token_hex",return_value="c" * 32):
                    with self.assertRaises(Q.QualificationError): controller.handoff()
                self.assertNotIn("handoff-accepted",controller.path.read_text())

    def test_unknown_payloads_are_refused_without_journaling(self):
        controller = self.controller(); self.ready(controller)
        record = {"case":controller.case,"nonce":controller.nonce,"kind":"intent","id":"1" * 32,
                  "action":"review-continue","pid":77,"artifactSha256":"f" * 64,
                  "context":Q.case_context(controller.case,controller.nonce),"token":"secret-canary"}
        with self.assertRaisesRegex(Q.QualificationError,"invalid controller record"):
            controller.receive(record)
        self.assertNotIn("secret-canary",controller.path.read_text())
        pump = Q.CommandPump(io.StringIO(json.dumps(record)+"\n")); pump.thread.join(1)
        self.assertEqual(pump.failure,"invalid command input")
        self.assertTrue(pump.commands.empty())

    def test_journal_write_failure_refuses_before_action_is_armed(self):
        controller = self.controller(); self.ready(controller)
        with patch.object(Q.os,"fsync",side_effect=OSError("untrusted detail")):
            with self.assertRaisesRegex(Q.QualificationError,"^controller journal write failed$"):
                self.intent(controller,"review-continue")
        self.assertEqual(controller.actions,{})

    def test_live_capture_kills_only_owned_child_on_unexplained_progress(self):
        controller = self.controller("stop-open", attach=False)
        candidate = rows("stop-open",pid=91)[:3]
        child = RelayTests.Child(); child.stdout = io.BytesIO(b"".join((json.dumps(row)+"\n").encode() for row in candidate))
        with patch.object(Q.subprocess,"Popen",return_value=child), patch.object(child,"kill",wraps=child.kill) as kill:
            capture = Q.capture_child(Path("/tmp/fake"),{"sha256":"f" * 64},"stop-open","a" * 32,1,controller.evidence,controller.pump,controller)
        self.assertTrue(capture["attributionFailure"])
        self.assertTrue(capture["eventFailure"])
        self.assertFalse(capture["timedOut"])
        kill.assert_called_once()
        self.assertEqual(capture["code"],-9)
        self.assertTrue(controller.failed)

    def test_all_six_raw_oracles_correlate_with_explicit_controller_actions(self):
        for case in Q.CASES:
            with self.subTest(case=case):
                controller = self.controller(case)
                if case.startswith("stop-"):
                    candidate = stop_rows(case)
                else:
                    candidate = rows(case)
                    if case != "accept-read":
                        candidate = [row for row in candidate if row["event"]!="phase" or row["fields"]["phase"] in (("native_review_queued",) if case=="cancel-review" else ("native_review_queued","native_review_accepted","native_input_queued","native_input_presented"))]
                        result = candidate[-2]["fields"]
                        result.update(status="error" if case=="expire-open" else "cancelled",code="credential_lease_expired" if case=="expire-open" else "cancelled",observation=False,rpcCounts={"health":0,"policyStatus":0,"logs":0})
                        if case=="parent-close":
                            candidate.insert(-2,{"schema":Q.SCHEMA,"case":case,"nonce":"a" * 32,"event":"command_received","fields":{"command":"close-parent"}})
                    for index,row in enumerate(candidate,1): row["sequence"]=index
                self.assertEqual(Q.verify_events(case,"a" * 32,77,candidate,120),candidate)
                review = submit = None
                for row in candidate:
                    event,fields = row["event"],row["fields"]
                    if event=="phase" and fields["phase"]=="native_review_accepted":
                        review = self.intent(controller,"review-continue")
                    if event=="phase" and fields["phase"]=="native_input_accepted":
                        submit = self.intent(controller,"input-submit","3")
                    if event=="result" and case=="cancel-review":
                        review = self.intent(controller,"review-cancel")
                    if event=="command_received":
                        self.assertEqual(controller.receive({"case":case,"nonce":"a" * 32,"command":fields["command"]}),fields["command"])
                    controller.native(row)
                    if event=="phase" and fields["phase"]=="native_input_presented":
                        self.returned(controller,review)
                        fill = self.intent(controller,"input-fill","2"); self.returned(controller,fill)
                if submit: self.returned(controller,submit)
                elif case=="cancel-review": self.returned(controller,review)
                terminal = {"case":case,"nonce":"a" * 32,"kind":"handoff","pid":77,"artifactSha256":"f" * 64,"terminalId":"c" * 32}
                controller.pump.commands.put(terminal)
                with patch.object(Q.secrets,"token_hex",return_value="c" * 32): controller.handoff()
                records = [json.loads(line) for line in controller.path.read_text().splitlines()]
                self.assertEqual([row["sequence"] for row in records],list(range(1,len(records)+1)))
                self.assertEqual(records[-1]["kind"],"handoff-accepted")

    def test_start_and_terminal_eof_never_enable_next_case(self):
        controller = self.controller(attach=False)
        with self.assertRaisesRegex(Q.QualificationError,"controller input closed"):
            controller.start()
        controller, submit = self.completed(); self.returned(controller,submit)
        with self.assertRaisesRegex(Q.QualificationError,"controller input closed"):
            controller.handoff()
        with self.assertRaisesRegex(Q.QualificationError,"controller child already bound"):
            controller.attach(78)

    def test_controller_input_eof_stale_records_and_removed_control_refuse(self):
        controller = self.controller()
        with self.assertRaisesRegex(Q.QualificationError,"controller input closed"):
            controller.next_record(.2)
        with self.assertRaisesRegex(Q.QualificationError,"stale controller identity"):
            controller.receive({"case":"accept-read","nonce":"b" * 32,"command":"stop"})
        with self.assertRaisesRegex(Q.QualificationError,"invalid controller record"):
            controller.receive({"case":"accept-read","nonce":"a" * 32,"command":"finish"})


class RelayTests(unittest.TestCase):
    def test_sequential_case_boundary_drops_stale_commands(self):
        command = lambda nonce, value: json.dumps({"case":"stop-open","nonce":nonce,"command":value}) + "\n"
        pump = Q.CommandPump(io.StringIO(command("a" * 32, "hold-main") + command("a" * 32, "stop") + command("a" * 32, "release-main"))); pump.thread.join(1)
        self.assertEqual(pump.commands.get_nowait()["command"], "hold-main")
        pump.discard()
        self.assertTrue(pump.commands.empty())
        self.assertIsNone(pump.failure)

    class Child:
        pid, returncode = 91, 0
        class RecordingInput(io.BytesIO):
            def close(self): pass
        def __init__(self): self.stdin, self.stdout, self.stderr = self.RecordingInput(), io.BytesIO(), io.BytesIO()
        def poll(self): return None
        def kill(self): self.returncode = -9
        def wait(self, timeout=None): __import__("time").sleep(.12); return 0

    def test_old_nonce_after_boundary_never_reaches_new_child(self):
        pump = Q.CommandPump(io.StringIO("")); pump.thread.join(1); pump.discard()
        old = {"case":"stop-open","nonce":"a" * 32,"command":"stop"}
        fresh = {"case":"stop-open","nonce":"b" * 32,"command":"stop"}
        pump.commands.put(old); pump.commands.put(fresh)
        with tempfile.TemporaryDirectory() as directory:
            child = self.Child(); artifact = {"sha256":"a" * 64}
            with patch.object(Q.subprocess, "Popen", return_value=child):
                Q.capture_child(Path("/tmp/fake"), artifact, "stop-open", "b" * 32, 1, Path(directory), pump)
            self.assertEqual(child.stdin.getvalue(), b"stop\n")


class CaptureTests(unittest.TestCase):
    class Child:
        pid, returncode = 91, 0
        def __init__(self): self.stdin, self.stdout, self.stderr = io.BytesIO(), io.BytesIO(), io.BytesIO()
        def poll(self): return None
        def kill(self): self.returncode = -9

    def test_timeout_and_cleanup_failure_stay_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory); child = self.Child(); calls = []
            def wait(*, timeout=None):
                calls.append(timeout)
                if len(calls) == 1: raise Q.subprocess.TimeoutExpired("child", timeout)
                return 0
            child.wait = wait
            artifact = {"sha256": "a" * 64}
            with patch.object(Q.subprocess, "Popen", return_value=child):
                result = Q.capture_child(Path("/tmp/fake"), artifact, "accept-read", "a" * 32, 1, evidence)
            self.assertTrue(result["timedOut"]); self.assertFalse(result["cleanupFailure"])

    def test_output_overflow_stops_owned_child_before_timeout(self):
        class UntilKilled(self.Child):
            def __init__(self):
                super().__init__(); self.stopped = threading.Event()
            def kill(self): super().kill(); self.stopped.set()
            def wait(self, timeout=None):
                if not self.stopped.wait(min(timeout,.5)):
                    raise Q.subprocess.TimeoutExpired("inert-child",timeout)
                return self.returncode
        for channel in ("oversized-stdout","unterminated-stdout","malformed-stdout","stderr"):
            with self.subTest(channel=channel), tempfile.TemporaryDirectory() as directory:
                child = UntilKilled()
                if channel=="oversized-stdout": child.stdout = io.BytesIO(b"x" * (Q.LINE_LIMIT+1))
                elif channel=="unterminated-stdout": child.stdout = io.BytesIO(b"{}")
                elif channel=="malformed-stdout": child.stdout = io.BytesIO(b"{bad-json}\n")
                else: child.stderr = io.BytesIO(b"x" * (Q.STDERR_LIMIT+1))
                with patch.object(Q.subprocess,"Popen",return_value=child), patch.object(child,"kill",wraps=child.kill) as kill:
                    result = Q.capture_child(Path("/tmp/fake"),{"sha256":"f" * 64},"accept-read","a" * 32,10,Path(directory))
                kill.assert_called_once()
                self.assertIs(result["timedOut"],False)
                self.assertIs(result["cleanupFailure"],False)
                self.assertEqual(result["code"],-9)
                key = "stderrOverflow" if channel=="stderr" else "eventFailure" if channel=="malformed-stdout" else "stdoutOverflow"
                self.assertIs(result[key],True)

    def test_setup_failure_records_bounded_cleanup_failure_without_raw_exception(self):
        for failure in ("launch-write","controller-attach","kill","wait"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                evidence = Path(directory); child = self.Child(); child.returncode = None
                def wait(timeout=None):
                    if failure=="wait": raise Q.subprocess.TimeoutExpired("untrusted-detail",timeout)
                    child.returncode = -9; return -9
                child.wait = wait
                if failure=="kill":
                    child.kill = lambda: (_ for _ in ()).throw(OSError("untrusted-kill-detail"))
                write_json = Q.write_json_new
                def write(path,value):
                    if path.name.endswith(".launch.json") and failure!="controller-attach": raise OSError("untrusted-launch-detail")
                    return write_json(path,value)
                from unittest.mock import Mock
                controller = Mock() if failure=="controller-attach" else None
                if controller: controller.attach.side_effect = Q.QualificationError("fixed attach refusal")
                with patch.object(Q.subprocess,"Popen",return_value=child), patch.object(Q,"write_json_new",side_effect=write):
                    with self.assertRaisesRegex(Q.QualificationError,"^owned child setup failed$"):
                        Q.capture_child(Path("/tmp/fake"),{"sha256":"f" * 64},"accept-read","a" * 32,10,evidence,controller=controller)
                receipt = json.loads((evidence/"accept-read.failure.json").read_text())
                self.assertEqual(receipt["code"],"owned-child-setup-failed")
                self.assertEqual(receipt["pid"],91)
                self.assertEqual(receipt["flags"]["cleanupFailure"],failure in {"kill","wait"})
                self.assertFalse(receipt["flags"]["timedOut"])
                self.assertNotIn("untrusted",json.dumps(receipt))

    def test_event_receipt_and_pipe_io_failures_stop_owned_child(self):
        class UntilKilled(self.Child):
            def __init__(self):
                super().__init__(); self.stopped = threading.Event()
            def kill(self): super().kill(); self.stopped.set()
            def wait(self,timeout=None):
                if not self.stopped.wait(min(timeout,.5)): raise Q.subprocess.TimeoutExpired("inert-child",timeout)
                return self.returncode
        class BadWriter(io.StringIO):
            def write(self,_value): raise OSError("untrusted-write-detail")
        for failure in ("open","write","stdout-read","stderr-read"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                child = UntilKilled(); child.stdout = io.BytesIO((json.dumps(rows(pid=91)[0])+"\n").encode())
                real_open = Path.open
                def open_event(path,*args,**kwargs):
                    if path.name.endswith(".events.jsonl"):
                        if failure=="open": raise OSError("untrusted-open-detail")
                        if failure=="write": return BadWriter()
                    return real_open(path,*args,**kwargs)
                pipe = child.stdout if failure=="stdout-read" else child.stderr
                method = "readline" if failure=="stdout-read" else "read"
                with patch.object(Q.subprocess,"Popen",return_value=child), patch.object(Path,"open",open_event), patch.object(child,"kill",wraps=child.kill) as kill:
                    if failure.endswith("read"):
                        with patch.object(pipe,method,side_effect=OSError("untrusted-pipe-detail")):
                            result = Q.capture_child(Path("/tmp/fake"),{"sha256":"f" * 64},"accept-read","a" * 32,10,Path(directory))
                    else:
                        result = Q.capture_child(Path("/tmp/fake"),{"sha256":"f" * 64},"accept-read","a" * 32,10,Path(directory))
                kill.assert_called_once()
                self.assertTrue(result["ioFailure"])
                self.assertFalse(result["timedOut"])
                self.assertFalse(result["cleanupFailure"])


class SummaryTests(unittest.TestCase):
    def test_parent_controller_matrix_never_grants_native_qualification(self):
        for selected in (None, "stop-open"):
            with self.subTest(selected=selected), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve(); evidence = root/"evidence"
                manifest = {"files":{"fixture":"hash"}}
                argv = ["--repo",str(root),"--toolchain-bin",str(root),"--evidence",str(evidence)]
                if selected: argv += ["--case",selected]
                def result(*args):
                    return {"case":args[5],"parentOraclePassed":True,"controllerCorrelationComplete":True}
                with patch.object(Q,"exact_root",return_value=root), patch.object(Q,"source_manifest",return_value=manifest), patch.object(Q,"verify_plan",return_value=Q.CASES), patch.object(Q,"toolchain_record",return_value={}), patch.object(Q,"build",return_value=(root/"inert",{})), patch.object(Q,"prepare_native_bundle",return_value=(root/"inert",{})), patch.object(Q,"verify_native_bundle"), patch.object(Q,"CommandPump"), patch.object(Q,"execute_case",side_effect=result), patch("sys.stdout",new_callable=io.StringIO):
                    Q.main(argv)
                summary = json.loads((evidence/"results.json").read_text())
                self.assertIs(summary["qualificationComplete"],False)
                self.assertEqual(summary["parentControllerMatrixComplete"],selected is None)
                self.assertEqual(summary["independentUiReview"],"required")
                self.assertEqual(summary["diagnosticSubset"],selected is not None)


if __name__ == "__main__":
    unittest.main()
