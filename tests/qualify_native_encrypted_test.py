"""Synthetic parent-oracle tests. No native child, vault, or provider is used."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("qualification", ROOT / "scripts/qualify-native-encrypted.py")
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)


def observation(reads=0, writes=0, opened=False):
    return {"close": {"state": "open" if opened else "locked"}, "clientPresent": opened,
            "vaultOpens": reads + writes, "vaultReads": reads, "vaultWrites": writes}


def journey(case):
    events = []
    def add(event, fields):
        events.append({"seq": len(events) + 1, "event": event, "fields": fields})
    def stage(name, reads=0, writes=0):
        add("startup", {"state": name, "retainedOriginal": "present_unencrypted",
                        "observation": observation(reads, writes)})
    def consent(purpose, outcome):
        add("consent", {"purpose": purpose, "outcome": outcome})
    expectation = Q.case_record(case)["expectation"]
    add("started", {"casePlanSha256": Q.PLAN_SHA256})
    original = None
    if Q.seed_for_root(Q.case_record(case)["root"]) != "none":
        empty = Q.seed_for_root(Q.case_record(case)["root"]) == "legacy_empty"
        original = {"schema": 0 if empty else 4, "archiveSha256": "a" * 64, "identity": "b" * 64,
                    "databaseSha256": "c" * 64, "databaseBytes": 4096,
                    "inventory": dict(zip(["sources", "capabilities", "revisions", "reviews", "recipes",
                                           "applications", "derivations", "workflows", "workflowRevisions"],
                                          [0] * 9 if empty else [1, 3, 4, 2, 1, 1, 2, 1, 2]))}
        add("original_before", copy.deepcopy(original))
    add("context_validated", {})
    add("ready", {"state": observation(), "windowPresented": True})
    reads, writes = 0, 0
    if expectation in {"cold_cancel", "cold_relaunch"}:
        stage("fresh")
        stage("handoff")
        consent("create_empty", "cancelled")
    elif expectation == "legacy_copy":
        stage("legacy_found")
        stage("conversion_review")
        consent("copy_existing", "approved")
        add("slot_write", {"workspaceId": "d" * 64, "succeeded": True})
        reads, writes = 2, 1
        stage("restart_required", reads, writes)
    elif expectation == "legacy_verify":
        stage("verify_required")
        consent("verify_copy", "approved")
        reads = 2
        stage("handoff", reads)
    elif expectation == "legacy_verify_cancel":
        stage("verify_required")
        consent("verify_copy", "cancelled")
        stage("verify_required")
    elif expectation == "legacy_relaunch":
        stage("handoff")
        consent("unlock", "approved")
        reads = 1
        add("observed", observation(reads, opened=True))
    elif expectation in Q.ADOPTION_CHECKPOINTS or expectation == "interrupted_provisioning":
        if expectation == "crash_reopened":
            stage("verify_required")
            consent("verify_copy", "approved")
            reads = 2
        else:
            stage("legacy_found")
            stage("conversion_review")
            consent("copy_existing", "approved")
            add("slot_write", {"workspaceId": "d" * 64, "succeeded": True})
            reads, writes = 2, 1
        custody = {"vaultOpens": reads + writes, "vaultReads": reads, "vaultWrites": writes}
        if expectation == "interrupted_provisioning":
            add("provisioning_barrier", {"afterRealKeyWrite": True, **custody})
        else:
            boundary = Q.ADOPTION_CHECKPOINTS[expectation]
            add("adoption_barrier", {"stage": boundary[0], "boundary": boundary[1], **custody})
        return events
    elif expectation in Q.RECOVERY_CONSENTS:
        stage("recovery_required")
        for purpose, outcome in Q.RECOVERY_CONSENTS[expectation]:
            consent(purpose, outcome)
        reads = 0 if expectation in {"recovery_inspect_cancel", "recovery_selection_cancel"} else 2
        terminal = "recovery_required" if expectation.endswith("cancel") else ("handoff" if expectation == "recovery_complete_selection" else "restart_required")
        stage(terminal, reads)
    elif expectation in {"legacy_review_cancel", "legacy_review_relaunch", "legacy_copy_cancel"}:
        stage("legacy_found")
        stage("conversion_review")
        if expectation == "legacy_copy_cancel":
            consent("copy_existing", "cancelled")
        if expectation != "legacy_review_relaunch":
            stage("legacy_found")
    else:
        raise AssertionError("test journey unsupported")
    if original is not None:
        add("original_after", copy.deepcopy(original))
    add("exit", {"clean": True, "failedClosed": False, "state": observation(reads, writes)})
    return events


def renumber(events):
    for i, event in enumerate(events):
        event["seq"] = i + 1
    return events


class CasePlanTests(unittest.TestCase):
    def rejected(self, raw):
        with self.assertRaises((RuntimeError, ValueError, TypeError, KeyError)):
            Q.parse_plan(raw)

    def test_exact_profile_and_dependencies_are_shared_source_bytes(self):
        self.assertEqual(len(Q.CASES), 46)
        self.assertEqual(len(Q.ROOTS), 14)
        self.assertEqual(Q.PLAN_BYTES, Q.PLAN_PATH.read_bytes())
        self.assertEqual(Q.case_record("legacy-rich-verify")["prerequisite"],
                         {"kind": "completed", "case": "legacy-rich-verify-cancel"})
        self.assertEqual(Q.prior_marker("busy-owner"), "authoring-running")
        self.assertEqual(Q.prior_marker("cleanup"), "children-stopped")
        self.assertEqual(Q.seed_for_root("legacy-empty"), "legacy_empty")

    def test_duplicate_json_fields_including_escaped_and_enum_tags_refuse(self):
        for before, after in [
            ('"schemaVersion":', '"schemaVersion":"wrong","schemaVersion":'),
            ('"schemaVersion":', '"schema\\u0056ersion":"wrong","schemaVersion":'),
            ('"kind": "initial"', '"kind":"initial","kind":"initial"'),
            ('"case": "cold-cancel"', '"case":"cold-cancel","case":"cold-cancel"'),
        ]:
            with self.subTest(before=before, after=after):
                changed = Q.PLAN_BYTES.decode().replace(before, after, 1)
                self.assertNotEqual(changed.encode(), Q.PLAN_BYTES)
                self.rejected(changed.encode())

    def test_malformed_shape_names_versions_and_bounds_refuse(self):
        mutants = [b"{}", b"[]", b"null", b"", Q.PLAN_BYTES + b"{}", b" " * 16385]
        for key, value in [("schemaVersion", "v2"), ("roots", []), ("cases", []), ("extra", True)]:
            plan = copy.deepcopy(Q.PLAN)
            plan[key] = value
            mutants.append(json.dumps(plan).encode())
        for name in ["../escape", "/absolute", "Bad", "-bad", "bad-", "bad--name", "a" * 49, "", "x\x00", "a.b", "1case"]:
            plan = copy.deepcopy(Q.PLAN)
            plan["cases"][0]["name"] = name
            mutants.append(json.dumps(plan).encode())
        for raw in mutants:
            with self.subTest(raw=raw[:80]):
                self.rejected(raw)

    def test_forward_cycle_cross_root_and_reopen_seed_refuse(self):
        edits = [
            lambda p: p["roots"].append(p["roots"][0]),
            lambda p: p["roots"].append("unused"),
            lambda p: p["cases"].append(p["cases"][0]),
            lambda p: p["cases"][1]["prerequisite"].update(case="authoring"),
            lambda p: p["cases"][1]["prerequisite"].update(case="cold-relaunch"),
            lambda p: p["cases"][1].update(seed="legacy_empty"),
            lambda p: p["cases"][1].update(root="authoring"),
            lambda p: p["cases"][1]["prerequisite"].update(extra="ignored"),
            lambda p: p["cases"][0]["prerequisite"].update(case="cold-cancel"),
            lambda p: p["cases"][0].update(expectation="pretend_success"),
        ]
        for edit in edits:
            plan = copy.deepcopy(Q.PLAN)
            edit(plan)
            with self.subTest(plan=plan["cases"][:2]):
                self.rejected(json.dumps(plan).encode())

    def test_old_or_changed_plan_manifest_cannot_be_adopted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.chmod(0o700)
            for name in Q.ROOTS:
                (root / name).mkdir(mode=0o700)
            manifest = {"schema": "rangoon.e1c4.run.v1", "nonce": "a" * 32,
                        "directory": Q.identity(root), "roots": {n: Q.identity(root / n) for n in Q.ROOTS}}
            Q.write_new(root / "manifest.json", manifest)
            with patch.object(Q, "run_path", return_value=root):
                with self.assertRaises(RuntimeError):
                    Q.validate_run("a" * 32)
                manifest["schema"] = "rangoon.e1c4.run.v2"
                manifest["casePlanSha256"] = "0" * 64
                with patch.object(Q, "read_record", return_value=manifest):
                    with self.assertRaises(RuntimeError):
                        Q.validate_run("a" * 32)
                    manifest["casePlanSha256"] = Q.PLAN_SHA256
                    self.assertEqual(Q.validate_run("a" * 32), root)

    def test_completed_prerequisite_requires_verified_same_artifact_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = {"sourceSha256": "a" * 64, "executableSha256": "b" * 64}
            Q.write_new(root / "artifact.json", artifact)
            receipt = {"case": "legacy-rich-copy", "pid": 4321, "parentFailure": None,
                       "phaseExitVerified": True, "casePlanSha256": Q.PLAN_SHA256,
                       **artifact, "artifactRecordSha256": Q.digest(root / "artifact.json"),
                       "returncode": 0, "events": [{"event": "exit", "fields": {"clean": True}}]}
            original_read = Q.read_record
            def read(path):
                return receipt if path.name.startswith("receipt-") else original_read(path)
            with patch.object(Q, "read_record", side_effect=read), patch.object(Q, "alive", return_value=False):
                self.assertIs(Q.prior_receipt(root, "legacy-rich-copy"), receipt)
                for key, value in [("sourceSha256", "c" * 64), ("executableSha256", "d" * 64),
                                   ("artifactRecordSha256", "e" * 64), ("casePlanSha256", "f" * 64),
                                   ("phaseExitVerified", False), ("parentFailure", "failed")]:
                    old = receipt[key]
                    receipt[key] = value
                    with self.subTest(key=key), self.assertRaises(RuntimeError):
                        Q.prior_receipt(root, "legacy-rich-copy")
                    receipt[key] = old

    def test_embedded_fixture_change_invalidates_source_pin_and_links_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("apps/desktop/Cargo.toml", "apps/desktop/Cargo.lock", "apps/desktop/build.rs",
                         "apps/desktop/tauri.conf.json", "scripts/qualify-native-encrypted.py", "Cargo.toml", "Cargo.lock"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixed synthetic source\n")
            fixture = root / "fixtures/composition/decompose-v0.json"
            fixture.parent.mkdir(parents=True)
            fixture.write_bytes(b'{"original":"fixture"}\n')
            with patch.object(Q, "REPO", root):
                original = Q.source_digest()
                fixture.write_bytes(b'{"changed":"fixture"}\n')
                self.assertNotEqual(Q.source_digest(), original)
                fixture.unlink()
                fixture.symlink_to(root / "Cargo.toml")
                with self.assertRaises(RuntimeError):
                    Q.source_digest()


class WorkflowOracleTests(unittest.TestCase):
    def verify(self, case, events):
        Q.verify_exit(case, 0, events, False)

    def test_complete_new_synthetic_observation_sequences_are_accepted(self):
        names = [c["name"] for c in Q.PLAN["cases"] if c["expectation"].startswith("legacy_") and not Q.captures_storage(c["name"])]
        for case in ["cold-cancel", "cold-relaunch"] + names:
            with self.subTest(case=case):
                self.verify(case, journey(case))

    def test_clean_exit_without_required_journey_is_not_success(self):
        for case in ["cold-cancel", "legacy-rich-copy", "legacy-empty-verify", "legacy-review-cancel", "legacy-rich-relaunch"]:
            events = [e for e in journey(case) if e["event"] not in {"startup", "consent", "observed"}]
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, renumber(events))

    def test_missing_wrong_or_unavailable_consent_cannot_pass(self):
        for case in ["cold-relaunch", "legacy-rich-copy", "legacy-rich-verify", "legacy-rich-verify-cancel", "legacy-copy-cancel", "legacy-rich-relaunch"]:
            for replacement in [None, {"purpose": "verify_copy", "outcome": "unavailable"}, {"purpose": "unlock", "outcome": "cancelled"}]:
                events = journey(case)
                for event in events:
                    if event["event"] == "consent":
                        event["fields"] = replacement
                events = [e for e in events if e["fields"] is not None]
                with self.subTest(case=case, replacement=replacement), self.assertRaises(RuntimeError):
                    self.verify(case, renumber(events))

    def test_original_change_missing_after_and_wrong_inventory_refuse(self):
        for key, value in [("databaseSha256", "d" * 64), ("archiveSha256", "e" * 64), ("identity", "f" * 64), ("schema", 3), ("databaseBytes", 0)]:
            events = journey("legacy-rich-copy")
            next(e for e in events if e["event"] == "original_after")["fields"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                self.verify("legacy-rich-copy", events)
        events = [e for e in journey("legacy-rich-copy") if e["event"] != "original_after"]
        with self.assertRaises(RuntimeError):
            self.verify("legacy-rich-copy", renumber(events))
        events = journey("legacy-rich-copy")
        for event in events:
            if event["event"].startswith("original_"):
                event["fields"]["inventory"]["workflowRevisions"] = 0
        with self.assertRaises(RuntimeError):
            self.verify("legacy-rich-copy", events)

    def test_premature_client_or_unconfirmed_key_access_refuse(self):
        for case in ["cold-relaunch", "legacy-rich-copy", "legacy-rich-verify", "legacy-rich-verify-cancel", "legacy-copy-cancel", "legacy-review-cancel"]:
            events = journey(case)
            next(e for e in events if e["event"] == "startup")["fields"]["observation"]["clientPresent"] = True
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, events)
        for key in ["vaultOpens", "vaultReads", "vaultWrites"]:
            events = journey("legacy-copy-cancel")
            next(e for e in events if e["event"] == "startup")["fields"]["observation"][key] = 1
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                self.verify("legacy-copy-cancel", events)

    def test_reopen_key_write_failed_copy_write_and_wrong_plan_refuse(self):
        for case in ["legacy-rich-relaunch", "legacy-rich-verify", "legacy-review-relaunch"]:
            events = journey(case)
            events.insert(-1, {"event": "slot_write", "fields": {"succeeded": True}})
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, renumber(events))
        events = journey("legacy-rich-copy")
        next(e for e in events if e["event"] == "slot_write")["fields"]["succeeded"] = False
        with self.assertRaises(RuntimeError):
            self.verify("legacy-rich-copy", events)
        events = journey("legacy-rich-copy")
        events[0]["fields"]["casePlanSha256"] = "0" * 64
        with self.assertRaises(RuntimeError):
            self.verify("legacy-rich-copy", events)

    def test_cancel_requires_post_consent_choice_but_allows_repeated_prior_status(self):
        events = journey("legacy-rich-verify-cancel")
        first = copy.deepcopy(next(e for e in events if e["event"] == "startup"))
        events.insert(events.index(next(e for e in events if e["event"] == "consent")), first)
        self.verify("legacy-rich-verify-cancel", renumber(events))
        events = [e for e in events if not (e["event"] == "startup" and e["seq"] > next(c["seq"] for c in events if c["event"] == "consent"))]
        with self.assertRaises(RuntimeError):
            self.verify("legacy-rich-verify-cancel", renumber(events))

    def test_invalid_custody_counter_types_are_not_coerced(self):
        for value in [True, -1, "0", None]:
            events = journey("legacy-rich-copy")
            next(e for e in events if e["event"] == "startup")["fields"]["observation"]["vaultWrites"] = value
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                self.verify("legacy-rich-copy", events)


def storage_pair(case):
    def file(index, digest="a", size=128):
        return {"dev": 1, "ino": index, "uid": 501, "bytes": size, "sha256": digest * 64}
    base = {"original": file(1, "c", 4096), "ciphertext": file(2, "d", 8192),
            "records": {}, "control": file(3, size=0),
            "directories": [{"dev": 1, "ino": i, "uid": 501} for i in range(10, 17)], "workspaceId": "d" * 64}
    expectation = Q.case_record(case)["expectation"]
    if Q.case_record(case)["root"] == Q.PROVISIONING_ROOT:
        absent = expectation in {"interrupted_provisioning", "recovery_inspect_cancel", "recovery_initialize_empty_cancel"}
        base.update(ownerRecords={name: file(40 + i, size=0 if i == 0 else 128)
                                  for i, name in enumerate(Q.OWNER_FILES[:3])},
                    ciphertext=None, workspaceChildren=[],
                    managedChildren=sorted(set(Q.OWNER_FILES[:3]) | {"encrypted-workspaces"}))
        base["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES[:2])}
        before = None if expectation == "interrupted_provisioning" else copy.deepcopy(base)
        after = copy.deepcopy(base)
        if not absent:
            after["ciphertext"] = file(2, "d", 8192)
            after["ownerRecords"][Q.OWNER_FILES[3]] = file(43)
            after["managedChildren"] = sorted(set(Q.OWNER_FILES) | {"encrypted-workspaces"})
            after["workspaceChildren"] = ["workspace.sqlite3"]
            after["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES[:3])}
            if expectation in {"legacy_verify", "legacy_relaunch"}:
                before = copy.deepcopy(after)
                if expectation == "legacy_relaunch":
                    before["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES)}
                after["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES)}
        return before, after
    if expectation in {"crash_empty_target", "crash_matching_target", "legacy_copy"}:
        before = None
    else:
        before = copy.deepcopy(base)
        prefix = 3 if expectation in {"crash_reopened", "legacy_verify"} else (4 if expectation in {"recovery_selection_cancel", "recovery_complete_selection"} else (5 if expectation == "legacy_relaunch" else 2))
        before["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES[:prefix])}
    after = copy.deepcopy(before or base)
    count = (2 if expectation in {"crash_empty_target", "crash_matching_target", "recovery_inspect_cancel", "recovery_record_cancel"}
             else 4 if expectation in {"crash_reopened", "recovery_selection_cancel"}
             else 5 if expectation in {"recovery_complete_selection", "legacy_verify", "legacy_relaunch"} else 3)
    after["records"] = {name: file(30 + i) for i, name in enumerate(Q.JOURNAL_FILES[:count])}
    if expectation == "recovery_restore":
        after["ciphertext"]["sha256"] = "e" * 64
    return before, after


class RecoveryOracleTests(unittest.TestCase):
    def verify(self, case, events=None, before=None, after=None, **changes):
        events = journey(case) if events is None else events
        if before is None and after is None:
            before, after = storage_pair(case)
        expected = Q.ADOPTION_CHECKPOINTS.get(Q.case_record(case)["expectation"])
        interrupted = expected is not None or Q.case_record(case)["expectation"] == "interrupted_provisioning"
        args = dict(case=case, code=-9 if interrupted else 0, events=events, killed=interrupted,
                    adoption_checkpoint=expected, storage_before=before, storage_after=after)
        args.update(changes)
        Q.verify_exit(**args)

    def test_all_four_durable_chains_require_case_specific_oracles(self):
        for record in Q.PLAN["cases"]:
            if Q.captures_storage(record["name"]):
                with self.subTest(case=record["name"]):
                    self.verify(record["name"])

    def test_wrong_absent_duplicate_or_timed_out_checkpoint_refuses(self):
        case = "recovery-empty-crash"
        for mutate in [lambda e: e[-1]["fields"].update(boundary="after_restore"),
                       lambda e: e.pop(), lambda e: e.append(copy.deepcopy(e[-1])),
                       lambda e: e.append({"event": "adoption_barrier_timeout", "fields": {}}),
                       lambda e: e.append({"event": "observed", "fields": observation()})]:
            events = journey(case)
            mutate(events)
            with self.subTest(events=events[-1:]), self.assertRaises(RuntimeError):
                self.verify(case, renumber(events))
        for changes in [dict(killed=False), dict(code=0), dict(adoption_checkpoint=None),
                        dict(adoption_checkpoint=("destination", "after_restore"))]:
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                self.verify(case, **changes)

    def test_completed_startup_outcome_cannot_precede_crash_barrier(self):
        for case in ("recovery-empty-crash", "recovery-parity-crash", "recovery-selection-crash"):
            for state in ("restart_required", "handoff"):
                events = journey(case)
                events.insert(-1, {"event": "startup", "fields": {"state": state,
                    "retainedOriginal": "present_unencrypted", "observation": observation()}})
                with self.subTest(case=case, state=state), self.assertRaises(RuntimeError):
                    self.verify(case, renumber(events))

    def test_crash_needs_real_original_confirmation_and_exact_key_effect(self):
        for name in ("recovery-empty-crash", "recovery-parity-crash", "recovery-selection-crash"):
            for event in ("original_before", "consent"):
                events = [e for e in journey(name) if e["event"] != event]
                with self.subTest(name=name, event=event), self.assertRaises(RuntimeError):
                    self.verify(name, renumber(events))
            for key, value in [("vaultReads", 0), ("vaultWrites", True), ("vaultOpens", 65)]:
                events = journey(name)
                events[-1]["fields"][key] = value
                with self.subTest(name=name, key=key), self.assertRaises(RuntimeError):
                    self.verify(name, events)
            before, after = storage_pair(name)
            after["original"]["sha256"] = "e" * 64
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                self.verify(name, before=before, after=after)

    def test_recovery_rejects_pending_path_extra_consent_or_wrong_order(self):
        for case in ("recovery-empty-resume", "recovery-parity-resume", "recovery-selection-resume"):
            for mutation in ("extra", "reverse", "unavailable", "missing"):
                events = journey(case)
                consents = [e for e in events if e["event"] == "consent"]
                if mutation == "extra":
                    events.insert(events.index(consents[0]) + 1, {"event": "consent", "fields": {"purpose": "recovery_reconcile", "outcome": "approved"}})
                elif mutation == "reverse":
                    consents[0]["fields"]["purpose"] = "recovery_unlock"
                elif mutation == "unavailable":
                    consents[-1]["fields"]["outcome"] = "unavailable"
                else:
                    events.remove(consents[-1])
                with self.subTest(case=case, mutation=mutation), self.assertRaises(RuntimeError):
                    self.verify(case, renumber(events))

    def test_cancel_never_selects_writes_key_or_changes_storage(self):
        for case in ("recovery-empty-cancel", "recovery-parity-cancel", "recovery-selection-cancel"):
            for mutation in ("client", "key", "handoff", "no_fresh_choice"):
                events = journey(case)
                if mutation == "no_fresh_choice":
                    last = [e for e in events if e["event"] == "startup"][-1]
                    events.remove(last)
                elif mutation == "handoff":
                    [e for e in events if e["event"] == "startup"][-1]["fields"]["state"] = "handoff"
                else:
                    state = [e for e in events if e["event"] == "startup"][-1]["fields"]["observation"]
                    state["clientPresent" if mutation == "client" else "vaultWrites"] = True if mutation == "client" else 1
                with self.subTest(case=case, mutation=mutation), self.assertRaises(RuntimeError):
                    self.verify(case, renumber(events))
            before, after = storage_pair(case)
            after["ciphertext"]["sha256"] = "e" * 64
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, before=before, after=after)

    def test_vault_key_write_and_handoff_cannot_precede_native_approval(self):
        for case in ("recovery-empty-crash", "recovery-selection-crash", "recovery-empty-resume", "recovery-selection-resume"):
            events = journey(case)
            first = next(e for e in events if e["event"] == "startup")
            first["fields"]["observation"]["vaultReads"] = 1
            first["fields"]["observation"]["vaultOpens"] = 1
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, events)
        case = "recovery-empty-crash"
        events = journey(case)
        write = next(e for e in events if e["event"] == "slot_write")
        events.remove(write)
        events.insert(events.index(next(e for e in events if e["event"] == "consent")), write)
        with self.assertRaises(RuntimeError):
            self.verify(case, renumber(events))
        case = "recovery-selection-resume"
        events = journey(case)
        handoff = copy.deepcopy([e for e in events if e["event"] == "startup"][-1])
        events.insert(events.index(next(e for e in events if e["event"] == "consent")), handoff)
        with self.assertRaises(RuntimeError):
            self.verify(case, renumber(events))

    def test_cancellation_before_inspection_or_selection_never_reads_key(self):
        for case in ("recovery-empty-cancel", "recovery-selection-cancel"):
            events = journey(case)
            last = [e for e in events if e["event"] == "startup"][-1]
            last["fields"]["observation"]["vaultOpens"] = 1
            last["fields"]["observation"]["vaultReads"] = 1
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                self.verify(case, events)

    def test_completed_steps_cannot_be_repeated_or_retained_records_rewritten(self):
        for case in ("recovery-empty-resume", "recovery-parity-resume", "recovery-selection-resume"):
            for mutation in ("rewrite", "lost_record", "early_selected", "ciphertext", "directory"):
                before, after = storage_pair(case)
                if mutation == "rewrite":
                    after["records"]["target.json"]["sha256"] = "b" * 64
                elif mutation == "lost_record":
                    del after["records"]["intent.json"]
                elif mutation == "early_selected":
                    if "selected.json" in after["records"]:
                        del after["records"]["selected.json"]
                    else:
                        after["records"]["selected.json"] = copy.deepcopy(after["records"]["target.json"])
                elif mutation == "directory":
                    after["directories"][0]["ino"] += 1
                else:
                    after["ciphertext"]["sha256"] = before["ciphertext"]["sha256"] if case == "recovery-empty-resume" else "f" * 64
                with self.subTest(case=case, mutation=mutation), self.assertRaises(RuntimeError):
                    self.verify(case, before=before, after=after)

    def test_interrupted_prerequisite_requires_exact_termination_receipt(self):
        case = "recovery-empty-crash"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = {"sourceSha256": "a" * 64, "executableSha256": "b" * 64}
            Q.write_new(root / "artifact.json", artifact)
            before, after = storage_pair(case)
            receipt = {"case": case, "pid": 4321, "parentFailure": None, "phaseExitVerified": True,
                       "casePlanSha256": Q.PLAN_SHA256, **artifact, "artifactRecordSha256": Q.digest(root / "artifact.json"),
                       "returncode": -9, "events": journey(case), "killedAtProvisioningBarrier": False,
                       "killedAtAdoptionBarrier": True, "killCheckpoint": ["destination", "after_confirm"],
                       "storageBefore": before, "storageAfter": after}
            original_read = Q.read_record
            def read(path):
                return receipt if path.name.startswith("receipt-") else original_read(path)
            with patch.object(Q, "read_record", side_effect=read), patch.object(Q, "alive", return_value=False):
                self.assertIs(Q.prior_receipt(root, case), receipt)
                for key, value in [("killedAtAdoptionBarrier", False), ("killedAtProvisioningBarrier", True),
                                   ("killCheckpoint", ["destination", "after_restore"]), ("returncode", 0),
                                   ("storageAfter", None)]:
                    old = receipt[key]
                    receipt[key] = value
                    with self.subTest(key=key), self.assertRaises(RuntimeError):
                        Q.prior_receipt(root, case)
                    receipt[key] = old


class StorageObservationTests(unittest.TestCase):
    def fixture(self, root):
        root.chmod(0o700)
        nonce, case = "a" * 32, "recovery-empty-crash"
        anchor = root / "recovery-empty"
        anchor.mkdir(mode=0o700)
        for sub in ("source-workspace", "workspace-adoption", "managed-workspace", "managed-workspace/encrypted-workspaces", "managed-workspace/encrypted-workspaces/" + "d" * 64):
            (anchor / sub).mkdir(mode=0o700)
        for sub, raw in [("source-workspace/workspace.sqlite3", b"synthetic original"),
                         ("workspace-adoption/adoption.lock", b""), ("workspace-adoption/intent.json", b"opaque intent"),
                         ("workspace-adoption/target.json", b"opaque target"),
                         ("managed-workspace/encrypted-workspaces/" + "d" * 64 + "/workspace.sqlite3", b"synthetic opaque destination")]:
            path = anchor / sub
            path.write_bytes(raw)
            path.chmod(0o600)
        Q.write_new(root / "manifest.json", {"nonce": nonce, "casePlanSha256": Q.PLAN_SHA256,
                                             "directory": Q.identity(root), "roots": {"recovery-empty": Q.identity(anchor)}})
        Q.write_new(root / "slot-recovery-empty.json", {"schema": "rangoon.e1c4.slot.v1", "nonce": nonce,
                                                        "root": "recovery-empty", "workspace_id": "d" * 64})
        return nonce, case, anchor

    def test_observation_is_opaque_bounded_and_read_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            nonce, case, anchor = self.fixture(root)
            before = {p: p.read_bytes() for p in root.rglob("*") if p.is_file()}
            observed = Q.capture_storage(root, nonce, case)
            self.assertEqual(set(observed["records"]), {"intent.json", "target.json"})
            self.assertEqual(observed["original"]["bytes"], len(b"synthetic original"))
            self.assertEqual({p: p.read_bytes() for p in root.rglob("*") if p.is_file()}, before)
            self.assertNotIn("opaque", json.dumps(observed))

    def test_original_changed_later_in_snapshot_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            nonce, case, anchor = self.fixture(root)
            original = Q.private_file
            def read(directory, name, limit, **kwargs):
                observed = original(directory, name, limit, **kwargs)
                if name == "target.json":
                    (anchor / "source-workspace/workspace.sqlite3").write_bytes(b"changed after its read")
                return observed
            with patch.object(Q, "private_file", side_effect=read), self.assertRaises(RuntimeError):
                Q.capture_storage(root, nonce, case)

    def test_linked_permissive_hardlinked_oversize_or_replaced_paths_refuse(self):
        import os
        for mutation in ("symlink", "dir_link", "mode", "hardlink", "oversize", "slot_escape", "foreign_file"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                nonce, case, anchor = self.fixture(root)
                target = anchor / "workspace-adoption/target.json"
                if mutation == "symlink":
                    target.unlink()
                    target.symlink_to(anchor / "source-workspace/workspace.sqlite3")
                elif mutation == "dir_link":
                    managed = anchor / "managed-workspace"
                    managed.rename(anchor / "renamed")
                    managed.symlink_to(anchor / "renamed", target_is_directory=True)
                elif mutation == "mode":
                    target.chmod(0o644)
                elif mutation == "hardlink":
                    os.link(target, root / "second-link")
                elif mutation == "oversize":
                    target.write_bytes(b"x" * 4097)
                elif mutation == "slot_escape":
                    slot = root / "slot-recovery-empty.json"
                    value = json.loads(slot.read_bytes())
                    value["workspace_id"] = "../escape"
                    slot.write_text(json.dumps(value))
                else:
                    (anchor / "workspace-adoption/foreign").write_bytes(b"unexpected")
                with self.assertRaises((RuntimeError, OSError)):
                    Q.capture_storage(root, nonce, case)


class EmptyProvisioningOracleTests(unittest.TestCase):
    crash = "legacy-rich-provisioning-crash"
    cancel = "legacy-rich-provisioning-initialize-cancel"
    resume = "legacy-rich-provisioning-resume"

    def verify(self, case, **changes):
        RecoveryOracleTests.verify(self, case, **changes)

    def test_prior_forty_cases_stay_exact_and_all_six_new_phases_pass(self):
        old = [case for case in Q.PLAN["cases"] if case["root"] != Q.PROVISIONING_ROOT]
        raw = json.dumps(old, separators=(",", ":")).encode()
        self.assertEqual(len(old), 40)
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "16dc0ab838334cecaba0815a6c270f0066bfa8c2f39de77eea4e51a3d5bfad5b")
        for case in Q.PLAN["cases"]:
            if case["root"] == Q.PROVISIONING_ROOT:
                with self.subTest(case=case["name"]):
                    self.verify(case["name"])

    def test_crash_rejects_wrong_duplicate_missing_timed_out_or_mixed_barrier(self):
        for kind in ("wrong", "duplicate", "missing", "timeout", "adoption", "activity", "extra"):
            events = journey(self.crash)
            barrier = events[-1]
            if kind == "wrong":
                barrier["fields"]["afterRealKeyWrite"] = False
            elif kind == "duplicate":
                events.append(copy.deepcopy(barrier))
            elif kind == "missing":
                events.pop()
            elif kind == "extra":
                barrier["fields"]["unknown"] = True
            else:
                name = {"timeout": "provisioning_barrier_timeout", "adoption": "adoption_barrier",
                        "activity": "observed"}[kind]
                events.append({"event": name, "fields": observation() if kind == "activity" else {}})
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                self.verify(self.crash, events=renumber(events))
        for changes in (dict(code=0), dict(code=-15), dict(killed=False),
                        dict(adoption_checkpoint=("destination", "after_confirm"))):
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                self.verify(self.crash, **changes)

    def test_crash_requires_exact_custody_slot_and_confirmation_order(self):
        for key, value in (("vaultOpens", 0), ("vaultReads", 65), ("vaultWrites", True),
                           ("vaultWrites", 0), ("vaultReads", "2")):
            events = journey(self.crash)
            events[-1]["fields"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                self.verify(self.crash, events=events)
        for kind in ("write_before_consent", "wrong_slot", "failed_write", "early_vault", "early_handoff"):
            events = journey(self.crash)
            write = next(e for e in events if e["event"] == "slot_write")
            if kind == "write_before_consent":
                events.remove(write)
                events.insert(events.index(next(e for e in events if e["event"] == "consent")), write)
            elif kind == "wrong_slot":
                write["fields"]["workspaceId"] = "e" * 64
            elif kind == "failed_write":
                write["fields"]["succeeded"] = False
            else:
                first = next(e for e in events if e["event"] == "startup")
                if kind == "early_vault":
                    first["fields"]["observation"]["vaultReads"] = 1
                else:
                    first["fields"]["state"] = "handoff"
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                self.verify(self.crash, events=renumber(events))

    def test_initialization_and_restore_need_exact_separate_consents(self):
        for case in (self.cancel, self.resume):
            for kind in ("missing", "extra", "reordered", "cancelled", "client", "write"):
                events = journey(case)
                consents = [e for e in events if e["event"] == "consent"]
                if kind == "missing":
                    events.remove(consents[1])
                elif kind == "extra":
                    events.insert(events.index(consents[0]), copy.deepcopy(consents[0]))
                elif kind == "reordered":
                    consents[0]["fields"], consents[1]["fields"] = consents[1]["fields"], consents[0]["fields"]
                elif kind == "cancelled":
                    consents[0]["fields"]["outcome"] = "cancelled"
                else:
                    last = [e for e in events if e["event"] == "startup"][-1]["fields"]["observation"]
                    last["clientPresent" if kind == "client" else "vaultWrites"] = True if kind == "client" else 1
                with self.subTest(case=case, kind=kind), self.assertRaises(RuntimeError):
                    self.verify(case, events=renumber(events))

    def test_cancel_preserves_every_retained_metadata_and_absence_proof(self):
        for case in ("legacy-rich-provisioning-inspect-cancel", self.cancel):
            for key in ("original", "control", "binding", "pending", "directory", "ciphertext", "extra", "terminal"):
                before, after = storage_pair(case)
                if key in {"original", "control"}:
                    after[key]["sha256"] = "e" * 64
                elif key in {"binding", "pending"}:
                    name = Q.OWNER_FILES[1 if key == "binding" else 2]
                    after["ownerRecords"][name]["sha256"] = "e" * 64
                elif key == "directory":
                    after["directories"][0]["ino"] += 1
                elif key == "ciphertext":
                    after[key] = copy.deepcopy(after["original"])
                elif key == "extra":
                    after["workspaceChildren"] = ["workspace.sqlite3-journal"]
                else:
                    after["ownerRecords"][Q.OWNER_FILES[3]] = copy.deepcopy(after["original"])
                with self.subTest(case=case, key=key), self.assertRaises(RuntimeError):
                    self.verify(case, before=before, after=after)

    def test_resume_changes_only_expected_boundaries_and_readonly_phases_stay_readonly(self):
        for kind in ("already_present", "still_absent", "zero", "original", "directory", "control", "lost_pending"):
            before, after = storage_pair(self.resume)
            if kind == "already_present":
                before["ciphertext"] = copy.deepcopy(after["ciphertext"])
            elif kind == "still_absent":
                after["ciphertext"] = None
            elif kind == "zero":
                after["ciphertext"]["bytes"] = 0
            elif kind == "original":
                after["original"]["sha256"] = "e" * 64
            elif kind == "directory":
                after["directories"][0]["ino"] += 1
            elif kind == "control":
                after["ownerRecords"][Q.OWNER_FILES[0]]["ino"] += 1
            else:
                del after["ownerRecords"][Q.OWNER_FILES[2]]
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                self.verify(self.resume, before=before, after=after)
        for suffix in ("verify", "relaunch"):
            case = "legacy-rich-provisioning-" + suffix
            for key in ("binding", "ciphertext"):
                before, after = storage_pair(case)
                if key == "binding":
                    after["ownerRecords"][Q.OWNER_FILES[1]]["sha256"] = "e" * 64
                else:
                    after["ciphertext"]["sha256"] = "e" * 64
                with self.subTest(case=case, key=key), self.assertRaises(RuntimeError):
                    self.verify(case, before=before, after=after)

    def test_prior_crash_receipt_needs_exact_flags_and_current_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = {"sourceSha256": "a" * 64, "executableSha256": "b" * 64}
            Q.write_new(root / "artifact.json", artifact)
            before, after = storage_pair(self.crash)
            receipt = {"case": self.crash, "pid": 4321, "parentFailure": None, "phaseExitVerified": True,
                       "casePlanSha256": Q.PLAN_SHA256, **artifact, "artifactRecordSha256": Q.digest(root / "artifact.json"),
                       "returncode": -9, "events": journey(self.crash), "killedAtProvisioningBarrier": True,
                       "killedAtAdoptionBarrier": False, "killCheckpoint": None, "storageBefore": before, "storageAfter": after}
            read = Q.read_record
            with patch.object(Q, "read_record", side_effect=lambda path: receipt if path.name.startswith("receipt-") else read(path)), patch.object(Q, "alive", return_value=False):
                self.assertIs(Q.prior_receipt(root, self.crash), receipt)
                for key, value in (("killedAtProvisioningBarrier", 1), ("killedAtAdoptionBarrier", True),
                                   ("killCheckpoint", ["destination", "after_confirm"]), ("returncode", 0),
                                   ("sourceSha256", "c" * 64), ("casePlanSha256", "c" * 64), ("storageAfter", None)):
                    old = receipt[key]
                    receipt[key] = value
                    with self.subTest(key=key), self.assertRaises(RuntimeError):
                        Q.prior_receipt(root, self.crash)
                    receipt[key] = old

    def fixture(self, root):
        nonce, _, anchor = StorageObservationTests().fixture(root)
        moved = root / Q.PROVISIONING_ROOT
        anchor.rename(moved)
        workspace = moved / "managed-workspace/encrypted-workspaces" / ("d" * 64)
        (workspace / "workspace.sqlite3").unlink()
        for index, name in enumerate(Q.OWNER_FILES[:3]):
            path = moved / "managed-workspace" / name
            path.write_bytes(b"" if index == 0 else b"opaque metadata")
            path.chmod(0o600)
        (root / "manifest.json").unlink()
        Q.write_new(root / "manifest.json", {"nonce": nonce, "casePlanSha256": Q.PLAN_SHA256,
                    "directory": Q.identity(root), "roots": {Q.PROVISIONING_ROOT: Q.identity(moved)}})
        Q.write_new(root / ("slot-" + Q.PROVISIONING_ROOT + ".json"),
                    {"schema": "rangoon.e1c4.slot.v1", "nonce": nonce, "root": Q.PROVISIONING_ROOT, "workspace_id": "d" * 64})
        return nonce, moved, workspace

    def test_absent_database_snapshot_is_opaque_and_readonly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            nonce, _, _ = self.fixture(root)
            before = {p: p.read_bytes() for p in root.rglob("*") if p.is_file()}
            result = Q.capture_storage(root, nonce, self.crash)
            self.assertIsNone(result["ciphertext"])
            self.assertEqual(result["workspaceChildren"], [])
            self.assertEqual(set(result["ownerRecords"]), set(Q.OWNER_FILES[:3]))
            self.assertNotIn("opaque", json.dumps(result))
            self.assertEqual({p: p.read_bytes() for p in root.rglob("*") if p.is_file()}, before)

    def test_absence_observer_rejects_zero_partial_sidecar_extra_and_links(self):
        for name in ("workspace.sqlite3", "workspace.sqlite3-wal", "workspace.sqlite3-shm", "workspace.sqlite3-journal", "extra", "link"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                nonce, anchor, workspace = self.fixture(root)
                path = workspace / ("workspace.sqlite3" if name == "link" else name)
                if name == "link":
                    path.symlink_to(anchor / "source-workspace/workspace.sqlite3")
                else:
                    path.write_bytes(b"")
                    path.chmod(0o600)
                with self.assertRaises((RuntimeError, OSError)):
                    Q.capture_storage(root, nonce, self.crash)

    def test_metadata_and_children_drift_during_snapshot_refuse(self):
        for kind in ("binding", "pending", "control", "child", "directory"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                nonce, anchor, workspace = self.fixture(root)
                read = Q.private_file
                def mutate(fd, name, limit, **kwargs):
                    result = read(fd, name, limit, **kwargs)
                    if name == Q.OWNER_FILES[2]:
                        if kind in {"binding", "pending", "control"}:
                            target = Q.OWNER_FILES[{"control": 0, "binding": 1, "pending": 2}[kind]]
                            (anchor / "managed-workspace" / target).write_bytes(b"changed")
                        elif kind == "child":
                            (workspace / "workspace.sqlite3-shm").write_bytes(b"unexpected")
                        else:
                            workspace.rename(workspace.with_name("replaced"))
                            workspace.mkdir(mode=0o700)
                    return result
                with patch.object(Q, "private_file", side_effect=mutate), self.assertRaises((RuntimeError, OSError)):
                    Q.capture_storage(root, nonce, self.crash)


class RecoveryObservationControls(unittest.TestCase):
    def test_only_authoring_and_relaunch_offer_finite_snapshot(self):
        for case in Q.CASES:
            controls = Q.controls_for_case(case)
            self.assertEqual("recovery-snapshot" in controls, case in {"authoring", "relaunch"})
            self.assertTrue(set(controls) <= {"observe", "quit", "arm-read", "release-read", "capture-client", "check-headers", "recovery-snapshot"})
            self.assertFalse(any("/" in value or "=" in value for value in controls))


class RecoverySnapshotReceipts(unittest.TestCase):
    def test_exact_bounded_receipt_and_closed_scope(self):
        valid={"event":"recovery_snapshot","fields":{"backupId":"backup:"+"a"*64,"byteLength":1234}}
        for case in ("authoring","relaunch"):
            Q.verify_recovery_snapshots(case,[valid])
        for fields in ({**valid["fields"],"key":"secret"},{**valid["fields"],"path":"/tmp/key"},{**valid["fields"],"content":"private"},{**valid["fields"],"byteLength":True},{**valid["fields"],"byteLength":0},{**valid["fields"],"byteLength":68*1024*1024+1},{**valid["fields"],"backupId":"backup:not-canonical"}):
            with self.assertRaises(RuntimeError):Q.verify_recovery_snapshots("authoring",[{"event":"recovery_snapshot","fields":fields}])
        for case,events in (("cleanup",[valid]),("authoring",[valid,valid]),("authoring",[{"event":"recovery_snapshot_failed","fields":{}}])):
            with self.assertRaises(RuntimeError):Q.verify_recovery_snapshots(case,events)


if __name__ == "__main__":
    unittest.main()
