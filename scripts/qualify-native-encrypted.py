#!/usr/bin/env python3
"""Disposable macOS runner for the shared encrypted startup path.

init builds the explicit custom target and creates a new isolated run. run NONCE
CASE starts exactly one phase; stdin accepts finite observation/exit controls and
authoring-only arm-read/release-read and capture-client/check-headers, never paths or script.
Receipts report observations, not a blanket qualification verdict. No normal
workspace, provider credential, or arbitrary Keychain entry is selected.
"""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]
LIMIT = 1024 * 1024
BUNDLE = "Rangoon E1c4.app"
PLAN_PATH = REPO / "apps/desktop/native-encrypted-qualification/cases.v1.json"
EXPECTATIONS = {
    "cold_cancel", "cold_relaunch", "authoring", "relaunch", "export", "lose_key",
    "missing_key", "busy_owner", "interrupted_setup", "interrupted_relaunch",
    "anchor_failure", "cleanup_failure", "window_failure", "cleanup", "legacy_copy",
    "legacy_verify_cancel", "legacy_verify", "legacy_relaunch", "legacy_review_cancel",
    "legacy_review_relaunch", "legacy_copy_cancel", "crash_empty_target", "crash_matching_target",
    "crash_reopened", "recovery_inspect_cancel", "recovery_record_cancel", "recovery_selection_cancel",
    "recovery_restore", "recovery_record_copied", "recovery_complete_selection",
    "interrupted_provisioning", "recovery_initialize_empty_cancel", "recovery_initialize_empty_restore",
}


ADOPTION_CHECKPOINTS = {
    "crash_empty_target": ("destination", "after_confirm"),
    "crash_matching_target": ("destination", "after_restore"),
    "crash_reopened": ("verification", "before_selection"),
}
RECOVERY_CONSENTS = {
    "recovery_inspect_cancel": [("recovery_inspect", "cancelled")],
    "recovery_record_cancel": [("recovery_inspect", "approved"), ("recovery_record_copied", "cancelled")],
    "recovery_selection_cancel": [("recovery_complete_selection", "cancelled")],
    "recovery_restore": [("recovery_inspect", "approved"), ("recovery_restore", "approved")],
    "recovery_record_copied": [("recovery_inspect", "approved"), ("recovery_record_copied", "approved")],
    "recovery_complete_selection": [("recovery_complete_selection", "approved")],
    "recovery_initialize_empty_cancel": [("recovery_inspect", "approved"), ("recovery_initialize_empty", "cancelled")],
    "recovery_initialize_empty_restore": [("recovery_inspect", "approved"),
        ("recovery_initialize_empty", "approved"), ("recovery_unlock", "approved"), ("recovery_restore", "approved")],
}
JOURNAL_FILES = ("intent.json", "target.json", "copied.json", "reopened.json", "selected.json")
PROVISIONING_ROOT = "legacy-rich-provisioning"
OWNER_FILES = ("encrypted-workspace.lock", "encrypted-workspace.binding",
               "encrypted-workspace.operation.pending", "encrypted-workspace.operation.terminal")


def bundle_info():
    return plistlib.dumps({"CFBundleIdentifier": "si.rangoon.encrypted-qualification",
                          "CFBundleName": "Rangoon E1c4", "CFBundleDisplayName": "Rangoon E1c4",
                          "CFBundleExecutable": "native-encrypted-qualification", "CFBundlePackageType": "APPL",
                          "CFBundleVersion": "1", "NSHighResolutionCapable": True})


def fail(message):
    raise RuntimeError(message)


def require(condition, message):
    if not condition:
        fail(message)


def json_exact(raw):
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate record field")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=object_pairs)


def parse_plan(raw):
    require(isinstance(raw, bytes) and len(raw) <= 16384, "case plan limit")
    plan = json_exact(raw)
    require(isinstance(plan, dict) and set(plan) == {"schemaVersion", "roots", "cases"}
            and plan["schemaVersion"] == "rangoon.e1c5c.case-plan.v1", "unsupported case plan")
    roots, cases = plan["roots"], plan["cases"]
    require(isinstance(roots, list) and 1 <= len(roots) <= 32
            and isinstance(cases, list) and 1 <= len(cases) <= 64, "case plan count")
    def name(value):
        return isinstance(value, str) and re.fullmatch(r"[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,47}", value) is not None
    require(all(name(root) for root in roots) and len(set(roots)) == len(roots), "invalid plan roots")
    seen, initial = {}, set()
    for case in cases:
        require(isinstance(case, dict) and set(case) == {"name", "root", "prerequisite", "seed", "expectation"},
                "invalid case shape")
        require(name(case["name"]) and case["name"] not in seen and case["root"] in roots
                and isinstance(case["seed"], str) and case["seed"] in {"none", "legacy_empty", "legacy_rich"}
                and isinstance(case["expectation"], str) and case["expectation"] in EXPECTATIONS,
                "invalid case vocabulary")
        prior = case["prerequisite"]
        require(isinstance(prior, dict) and isinstance(prior.get("kind"), str), "invalid prerequisite")
        kind = prior["kind"]
        if kind in {"initial", "all_stopped"}:
            require(set(prior) == {"kind"}, "unexpected prerequisite case")
            if kind == "initial":
                require(case["root"] not in initial, "duplicate initial phase")
                initial.add(case["root"])
        else:
            require(kind in {"completed", "running"} and set(prior) == {"kind", "case"}
                    and isinstance(prior["case"], str) and prior["case"] in seen
                    and seen[prior["case"]]["root"] == case["root"], "invalid phase chain")
        require(kind == "initial" or case["seed"] == "none", "reopen seed forbidden")
        seen[case["name"]] = case
    require(initial == set(roots) and {case["root"] for case in cases} == set(roots), "missing root initial phase")
    return plan


require(not PLAN_PATH.is_symlink(), "case plan source link forbidden")
PLAN_BYTES = PLAN_PATH.read_bytes()
PLAN = parse_plan(PLAN_BYTES)
PLAN_SHA256 = hashlib.sha256(PLAN_BYTES).hexdigest()
ROOTS = tuple(PLAN["roots"])
CASES = tuple(case["name"] for case in PLAN["cases"])


def case_record(case):
    require(case in CASES, "invalid case")
    return next(item for item in PLAN["cases"] if item["name"] == case)


def prior_marker(case):
    prior = case_record(case)["prerequisite"]
    if prior["kind"] == "completed":
        return prior["case"]
    if prior["kind"] == "running":
        return prior["case"] + "-running"
    return "children-stopped" if prior["kind"] == "all_stopped" else None


def seed_for_root(root):
    return next(item["seed"] for item in PLAN["cases"]
                if item["root"] == root and item["prerequisite"]["kind"] == "initial")


def write_new(path, value):
    raw = json.dumps(value, indent=2, sort_keys=True).encode() + b"\n"
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def read_record(path):
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
            and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
            and before.st_size <= LIMIT, "invalid runner record")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as stream:
        after = os.fstat(stream.fileno())
        require((before.st_dev, before.st_ino) == (after.st_dev, after.st_ino), "record replaced")
        raw = stream.read(LIMIT + 1)
    require(len(raw) <= LIMIT, "record limit")
    return json_exact(raw)


def identity(path):
    value = path.lstat()
    require(stat.S_ISDIR(value.st_mode) and value.st_uid == os.getuid()
            and stat.S_IMODE(value.st_mode) == 0o700, "invalid run directory")
    return {"dev": value.st_dev, "ino": value.st_ino, "uid": value.st_uid}


def run_path(nonce):
    require(re.fullmatch(r"[0-9a-f]{32}", nonce) is not None, "invalid nonce")
    return Path(tempfile.gettempdir()).resolve() / ("rangoon-e1c4-" + nonce)


def validate_run(nonce):
    root = run_path(nonce)
    manifest = read_record(root / "manifest.json")
    require(set(manifest) == {"schema", "nonce", "directory", "roots", "casePlanSha256"}
            and manifest.get("schema") == "rangoon.e1c4.run.v2" and manifest.get("nonce") == nonce
            and manifest["casePlanSha256"] == PLAN_SHA256,
            "invalid manifest")
    require(identity(root) == manifest["directory"] and set(manifest["roots"]) == set(ROOTS), "run replaced")
    for name in ROOTS:
        require(identity(root / name) == manifest["roots"][name], "case directory replaced")
    return root


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()



def private_file(directory, name, limit, retain=False, checks=None):
    """Hash fixed child bytes without following links; retain only slot metadata."""
    before = os.stat(name, dir_fd=directory, follow_symlinks=False)
    def fingerprint(value):
        return (value.st_dev, value.st_ino, value.st_uid, value.st_mode, value.st_nlink,
                value.st_size, value.st_mtime_ns, value.st_ctime_ns)
    require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
            and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
            and 0 <= before.st_size <= limit, "unsafe storage observation file")
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
    result, contents, total = hashlib.sha256(), bytearray(), 0
    with os.fdopen(fd, "rb") as stream:
        require(fingerprint(os.fstat(stream.fileno())) == fingerprint(before), "storage file replaced")
        while True:
            chunk = stream.read(min(65536, limit + 1 - total))
            if not chunk:
                break
            total += len(chunk)
            require(total <= limit, "storage observation limit")
            result.update(chunk)
            if retain:
                contents.extend(chunk)
        require(fingerprint(os.fstat(stream.fileno())) == fingerprint(before)
                and fingerprint(os.stat(name, dir_fd=directory, follow_symlinks=False)) == fingerprint(before)
                and total == before.st_size, "storage changed during observation")
    if checks is not None:
        checks.append(lambda: require(fingerprint(os.stat(name, dir_fd=directory, follow_symlinks=False)) == fingerprint(before),
                                      "storage file changed during snapshot"))
    return {"dev": before.st_dev, "ino": before.st_ino, "uid": before.st_uid,
            "bytes": total, "sha256": result.hexdigest()}, bytes(contents)


def captures_storage(case):
    # These roots alone belong to the frozen durable-interruption chains.
    return case_record(case)["root"] in {"recovery-empty", "recovery-parity", "recovery-selection", PROVISIONING_ROOT}


def capture_storage(root, nonce, case):
    """Parent-only opaque observation after a stopped child; no key or SQL read."""
    record = case_record(case)
    manifest = read_record(root / "manifest.json")
    require(manifest["nonce"] == nonce and manifest["casePlanSha256"] == PLAN_SHA256,
            "storage run mismatch")
    handles, proofs, checks = [], [], []
    def directory(parent, name, expected=None):
        fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
        handles.append(fd)
        value = os.fstat(fd)
        require(stat.S_ISDIR(value.st_mode) and value.st_uid == os.getuid()
                and stat.S_IMODE(value.st_mode) == 0o700, "unsafe storage observation directory")
        proof = {"dev": value.st_dev, "ino": value.st_ino, "uid": value.st_uid}
        require(expected is None or proof == expected, "storage directory replaced")
        proofs.append((parent, name, fd, proof))
        return fd
    try:
        run = directory(None, root, manifest["directory"])
        anchor = directory(run, record["root"], manifest["roots"][record["root"]])
        source = directory(anchor, "source-workspace")
        original, _ = private_file(source, "workspace.sqlite3", 64 * 1024 * 1024, checks=checks)
        journal = directory(anchor, "workspace-adoption")
        names = set(os.listdir(journal))
        require("adoption.lock" in names and names <= set(JOURNAL_FILES) | {"adoption.lock"},
                "unexpected adoption storage")
        control, _ = private_file(journal, "adoption.lock", 0, checks=checks)
        records = {name: private_file(journal, name, 4096, checks=checks)[0] for name in JOURNAL_FILES if name in names}
        require(names == set(os.listdir(journal)), "journal changed during observation")
        slot_name = "slot-" + record["root"] + ".json"
        _, raw = private_file(run, slot_name, 16384, retain=True, checks=checks)
        slot = json_exact(raw)
        require(set(slot) == {"schema", "nonce", "root", "workspace_id"}
                and slot["schema"] == "rangoon.e1c4.slot.v1" and slot["nonce"] == nonce
                and slot["root"] == record["root"] and isinstance(slot["workspace_id"], str)
                and re.fullmatch(r"[0-9a-f]{64}", slot["workspace_id"]), "invalid storage slot registry")
        managed = directory(anchor, "managed-workspace")
        parent = directory(managed, "encrypted-workspaces")
        workspace = directory(parent, slot["workspace_id"])
        provisioning = record["root"] == PROVISIONING_ROOT
        extra, name_checks = {}, []
        if provisioning:
            managed_names, workspace_names = set(os.listdir(managed)), set(os.listdir(workspace))
            expected_managed = set(OWNER_FILES[:3]) | {"encrypted-workspaces"}
            require(managed_names in (expected_managed, expected_managed | {OWNER_FILES[3]})
                    and set(os.listdir(parent)) == {slot["workspace_id"]}
                    and workspace_names in (set(), {"workspace.sqlite3"}), "unexpected provisioning storage")
            owner_records = {name: private_file(managed, name, 0 if name == OWNER_FILES[0] else 4096,
                                               checks=checks)[0] for name in OWNER_FILES if name in managed_names}
            require(all(value["bytes"] > 0 for name, value in owner_records.items() if name != OWNER_FILES[0]),
                    "empty provisioning metadata")
            extra = {"ownerRecords": owner_records, "managedChildren": sorted(managed_names),
                     "workspaceChildren": sorted(workspace_names)}
            name_checks = [(managed, managed_names), (parent, {slot["workspace_id"]}), (workspace, workspace_names)]
        ciphertext = None
        if not provisioning or "workspace.sqlite3" in workspace_names:
            ciphertext, _ = private_file(workspace, "workspace.sqlite3", 64 * 1024 * 1024, checks=checks)
            require(ciphertext["bytes"] > 0, "empty storage observation database")
        require(original["bytes"] > 0, "empty original observation database")
        for check in checks:
            check()
        require(names == set(os.listdir(journal)), "journal changed during snapshot")
        require(all(set(os.listdir(fd)) == children for fd, children in name_checks),
                "provisioning children changed during snapshot")
        directories = []
        for parent_fd, name, fd, proof in proofs:
            value = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            held = os.fstat(fd)
            require(stat.S_ISDIR(value.st_mode) and stat.S_IMODE(value.st_mode) == 0o700
                    and {"dev": value.st_dev, "ino": value.st_ino, "uid": value.st_uid} == proof
                    and (held.st_dev, held.st_ino, held.st_uid) == (value.st_dev, value.st_ino, value.st_uid),
                    "storage directory changed during observation")
            directories.append(proof)
        return {"original": original, "ciphertext": ciphertext, "records": records,
                "control": control, "directories": directories, "workspaceId": slot["workspace_id"], **extra}
    finally:
        for fd in reversed(handles):
            os.close(fd)


def verify_storage(case, events, before, after):
    if not captures_storage(case):
        require(before is None and after is None, "unexpected parent storage observation")
        return
    provisioning = case_record(case)["root"] == PROVISIONING_ROOT
    keys = {"original", "ciphertext", "records", "control", "directories", "workspaceId"}
    if provisioning:
        keys |= {"ownerRecords", "managedChildren", "workspaceChildren"}
    require(isinstance(after, dict) and set(after) == keys,
            "missing parent storage observation")
    native_original = [e["fields"] for e in events if e["event"] == "original_before"]
    require(len(native_original) == 1 and after["original"]["sha256"] == native_original[0]["databaseSha256"]
            and after["original"]["bytes"] == native_original[0]["databaseBytes"], "original bytes changed after interruption")
    expectation = case_record(case)["expectation"]
    counts = {"crash_empty_target": (None, 2), "crash_matching_target": (None, 2),
              "crash_reopened": (3, 4), "recovery_inspect_cancel": (2, 2),
              "recovery_record_cancel": (2, 2), "recovery_selection_cancel": (4, 4),
              "recovery_restore": (2, 3), "recovery_record_copied": (2, 3),
              "recovery_complete_selection": (4, 5), "legacy_copy": (None, 3),
              "legacy_verify": (3, 5), "legacy_relaunch": (5, 5)}
    counts.update(interrupted_provisioning=(None, 2), recovery_initialize_empty_cancel=(2, 2),
                  recovery_initialize_empty_restore=(2, 3))
    require(expectation in counts, "unsupported storage oracle")
    old_count, new_count = counts[expectation]
    require(set(after["records"]) == set(JOURNAL_FILES[:new_count]), "wrong durable journal boundary")
    if provisioning:
        absent = expectation in {"interrupted_provisioning", "recovery_inspect_cancel", "recovery_initialize_empty_cancel"}
        owner_names = set(OWNER_FILES[:3 if absent else 4])
        require(set(after["ownerRecords"]) == owner_names
                and after["managedChildren"] == sorted(owner_names | {"encrypted-workspaces"})
                and after["workspaceChildren"] == ([] if absent else ["workspace.sqlite3"]),
                "wrong provisioning child boundary")
        require(after["ciphertext"] is None if absent else
                isinstance(after["ciphertext"], dict) and type(after["ciphertext"].get("bytes")) is int
                and after["ciphertext"]["bytes"] > 0, "wrong provisioning database boundary")
        if expectation == "interrupted_provisioning":
            writes = [e["fields"] for e in events if e["event"] == "slot_write"]
            require(len(writes) == 1 and writes[0].get("workspaceId") == after["workspaceId"],
                    "provisioning used another key slot")
    if old_count is None:
        require(before is None, "initial storage oracle reused prior state")
        return
    require(isinstance(before, dict) and set(before["records"]) == set(JOURNAL_FILES[:old_count]),
            "missing prior durable boundary")
    require(all(before[key] == after[key] for key in ("original", "control", "directories", "workspaceId"))
            and all(after["records"].get(name) == value for name, value in before["records"].items()),
            "recovery rewrote retained storage")
    if provisioning:
        require(set(before) == keys and before["ownerRecords"][OWNER_FILES[0]] == after["ownerRecords"][OWNER_FILES[0]],
                "provisioning control changed")
        if expectation == "recovery_initialize_empty_restore":
            require(before["ciphertext"] is None and set(before["ownerRecords"]) == set(OWNER_FILES[:3])
                    and before["workspaceChildren"] == []
                    and before["managedChildren"] == sorted(set(OWNER_FILES[:3]) | {"encrypted-workspaces"}),
                    "initialization did not start from absent database")
        elif expectation in {"recovery_inspect_cancel", "recovery_initialize_empty_cancel"}:
            require(before == after, "cancelled provisioning changed storage")
        else:
            require(all(before[key] == after[key] for key in ("ownerRecords", "managedChildren", "workspaceChildren")),
                    "read-only provisioning changed metadata")
        if expectation == "recovery_initialize_empty_restore":
            return
    if expectation == "recovery_restore":
        require(before["ciphertext"]["sha256"] != after["ciphertext"]["sha256"], "empty destination was not restored")
    else:
        require(before["ciphertext"] == after["ciphertext"], "read-only recovery changed ciphertext")


def source_digest():
    # Include all local Rust dependencies and embedded assets, not only a commit
    # name in this deliberately dirty qualification worktree.
    paths = []
    for prefix in ("apps/desktop/src", "apps/desktop/native-encrypted-qualification",
                   "apps/desktop/capabilities", "apps/desktop/permissions", "crates", "preview", "fixtures"):
        paths.extend(p for p in (REPO / prefix).rglob("*") if p.is_file()
                     and "target" not in p.parts and "__pycache__" not in p.parts)
    paths.extend(REPO / p for p in ("apps/desktop/Cargo.toml", "apps/desktop/Cargo.lock",
                                  "apps/desktop/build.rs", "apps/desktop/tauri.conf.json",
                                  "scripts/qualify-native-encrypted.py", "Cargo.toml", "Cargo.lock"))
    result = hashlib.sha256()
    for path in sorted(set(paths)):
        require(not path.is_symlink(), "source link not accepted")
        result.update(str(path.relative_to(REPO)).encode() + b"\0" + digest(path).encode() + b"\n")
    return result.hexdigest()


def safe_environment():
    # Native OS custody needs the user's login session. No model/provider
    # credentials or Tauri configuration overrides enter the child environment.
    keys = ("PATH", "HOME", "USER", "LOGNAME", "TMPDIR", "LANG", "LC_ALL",
            "__CF_USER_TEXT_ENCODING", "XPC_FLAGS", "XPC_SERVICE_NAME")
    return {key: os.environ[key] for key in keys if key in os.environ}


def init():
    before = source_digest()
    cmd = ["cargo", "test", "--manifest-path", "apps/desktop/Cargo.toml", "--test",
           "native-encrypted-qualification", "--features", "native-encrypted-qualification,encrypted-workspace",
           "--locked", "--offline", "--no-run", "--message-format=json"]
    built = subprocess.run(cmd, cwd=REPO, env=safe_environment(), stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, timeout=600, check=False)
    require(built.returncode == 0, "qualification build failed; run the named cargo command for diagnostics")
    require(before == source_digest(), "source changed during build")
    artifacts = []
    for line in built.stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-artifact" and item.get("executable") and item["target"]["name"] == "native-encrypted-qualification":
            artifacts.append(Path(item["executable"]).resolve())
    require(len(artifacts) == 1, "expected exactly one qualification executable")
    executable = artifacts[0]
    require(executable.is_relative_to(REPO / "apps/desktop/target"), "unexpected executable location")
    nonce = os.urandom(16).hex()
    root = run_path(nonce)
    root.mkdir(mode=0o700)
    for name in ROOTS:
        (root / name).mkdir(mode=0o700)
    # Accessibility tools require an application bundle to address this exact
    # running native process. This disposable wrapper contains the unchanged
    # custom-test executable; it is not an installer or a product package.
    bundle_directories = (root / BUNDLE, root / BUNDLE / "Contents", root / BUNDLE / "Contents/MacOS")
    for directory in bundle_directories:
        directory.mkdir(mode=0o700)
    wrapped = bundle_directories[-1] / "native-encrypted-qualification"
    shutil.copyfile(executable, wrapped)
    wrapped.chmod(0o700)
    info = bundle_directories[1] / "Info.plist"
    info.write_bytes(bundle_info())
    info.chmod(0o600)
    require(digest(wrapped) == digest(executable), "wrapper changed executable bytes")
    write_new(root / "manifest.json", {"schema": "rangoon.e1c4.run.v2", "nonce": nonce,
              "casePlanSha256": PLAN_SHA256,
              "directory": identity(root), "roots": {name: identity(root / name) for name in ROOTS}})
    write_new(root / "artifact.json", {"sourceSha256": before, "executable": str(wrapped),
              "casePlanSha256": PLAN_SHA256,
              "sourceExecutable": str(executable), "wrapperDirectories": {
                  str(d.relative_to(root)): identity(d) for d in bundle_directories},
              "infoPlistSha256": digest(info),
              "executableSha256": digest(executable), "architecture": subprocess.check_output(["file", "-b", str(executable)], text=True).strip(),
              "os": subprocess.check_output(["sw_vers", "-productVersion"], text=True).strip()})
    print(json.dumps({"nonce": nonce, "directory": str(root), "status": "created", "nativeCasesPassed": 0}), flush=True)


def alive(pid):
    require(isinstance(pid, int) and pid > 1, "invalid child PID")
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def prior_receipt(root, case):
    receipt = read_record(root / ("receipt-" + case + ".json"))
    require(receipt["case"] == case and not alive(receipt["pid"]), "prior child still exists")
    require(receipt["parentFailure"] is None, "prior runner failed")
    artifact = read_record(root / "artifact.json")
    require(receipt.get("phaseExitVerified") is True and receipt.get("casePlanSha256") == PLAN_SHA256
            and all(receipt.get(key) == artifact[key] for key in ("sourceSha256", "executableSha256"))
            and receipt.get("artifactRecordSha256") == digest(root / "artifact.json"),
            "prior phase artifact mismatch")
    expected = ADOPTION_CHECKPOINTS.get(case_record(case)["expectation"])
    if expected:
        require(receipt.get("killedAtAdoptionBarrier") is True
                and receipt.get("killCheckpoint") == list(expected)
                and receipt.get("killedAtProvisioningBarrier") is False
                and receipt["returncode"] == -signal.SIGKILL,
                "missing parent-observed adoption termination")
        verify_exit(case, receipt["returncode"], receipt["events"], True,
                    adoption_checkpoint=expected, storage_before=receipt.get("storageBefore"),
                    storage_after=receipt.get("storageAfter"))
    elif case == "interrupted-setup" or case_record(case)["expectation"] == "interrupted_provisioning":
        require(receipt.get("killedAtProvisioningBarrier") is True
                and receipt.get("killedAtAdoptionBarrier") is False and receipt.get("killCheckpoint") is None
                and receipt["returncode"] == -signal.SIGKILL,
                "missing parent-observed provisioning termination")
        verify_exit(case, receipt["returncode"], receipt["events"], True,
                    storage_before=receipt.get("storageBefore"), storage_after=receipt.get("storageAfter"))
    else:
        require(receipt["returncode"] == 0, "prior phase did not exit successfully")
        require(any(e["event"] == "cleanup_complete" or
                    (e["event"] == "exit" and e["fields"].get("clean") is True)
                    for e in receipt["events"]), "prior phase lacks terminal evidence")
    return receipt


def permit(root, nonce, case):
    require(case in CASES, "invalid case")
    record = case_record(case)
    prior = prior_marker(case)
    if record["prerequisite"]["kind"] == "running":
        running = record["prerequisite"]["case"]
        claim = read_record(root / ("claimed-" + running + ".json"))
        require(alive(claim["pid"]) and not (root / ("receipt-" + running + ".json")).exists(), "prior child not running")
    elif prior == "children-stopped":
        for claim in root.glob("claimed-*.json"):
            require(not alive(read_record(claim)["pid"]), "cleanup refused while a claimed child exists")
    elif prior:
        prior_receipt(root, prior)
    else:
        require(not any((root / record["root"]).iterdir()), "initial case root is not empty")
    write_new(root / ("permit-" + case + ".json"), {"schema": "rangoon.e1c4.permit.v1",
              "nonce": nonce, "case": case, "prior": prior})


def verify_workflow(case, events):
    """Assert native workflow observations, independently of manual UI notes."""
    record = case_record(case)
    expectation = record["expectation"]
    fields = lambda name: [e["fields"] for e in events if e["event"] == name]
    startup = [e for e in events if e["event"] == "startup"]
    observations = (fields("state") + fields("observed")
                    + [e["fields"]["observation"] for e in startup]
                    + [e["state"] for e in fields("ready") + fields("exit")])
    consents = fields("consent")
    require(observations and all(isinstance(s, dict) and type(s.get("clientPresent")) is bool
                                and all(type(s.get(key)) is int and s[key] >= 0
                                        for key in ("vaultOpens", "vaultReads", "vaultWrites"))
                                for s in observations), "invalid native custody observation")
    require(all(set(c) == {"purpose", "outcome"} for c in consents), "invalid consent observation")
    def exact_consent(purpose, outcome):
        require(consents == [{"purpose": purpose, "outcome": outcome}], "wrong or missing native consent")
    def ordered(*stages):
        selected, after = [], 0
        for stage in stages:
            match = next((e for e in startup if e["seq"] > after and e["fields"]["state"] == stage), None)
            require(match is not None, "missing ordered startup stage: " + stage)
            selected.append(match)
            after = match["seq"]
        return selected
    def no_client():
        require(observations and all(s["clientPresent"] is False for s in observations),
                "source-free phase published a client")
    def no_vault():
        require(all(s["vaultOpens"] == s["vaultReads"] == s["vaultWrites"] == 0 for s in observations),
                "unconfirmed phase reached vault custody")
    if expectation in {"cold_cancel", "cold_relaunch"}:
        ordered("fresh", "handoff")
        exact_consent("create_empty", "cancelled")
        no_client()
        no_vault()
    if expectation in {"authoring", "relaunch", "export"}:
        required = "create_empty" if expectation == "authoring" else "unlock"
        require({"purpose": required, "outcome": "approved"} in consents,
                "authoring phase lacks actual native confirmation")
        require(any(s["close"]["state"] == "open" and s["clientPresent"] is True for s in observations),
                "authoring phase never opened a real client")
        # Export bytes/content still require their separate UI and artifact review.
    if seed_for_root(record["root"]) == "none":
        return
    before, after = fields("original_before"), fields("original_after")
    interrupted = expectation in ADOPTION_CHECKPOINTS or expectation == "interrupted_provisioning"
    require(len(before) == 1 and ((not after and interrupted) or (len(after) == 1 and before == after and not interrupted)),
            "original preservation missing or changed")
    original = before[0]
    inventory_keys = {"sources", "capabilities", "revisions", "reviews", "recipes", "applications",
                      "derivations", "workflows", "workflowRevisions"}
    require(set(original) == {"schema", "inventory", "archiveSha256", "identity", "databaseSha256", "databaseBytes"}
            and isinstance(original["inventory"], dict) and set(original["inventory"]) == inventory_keys
            and all(type(n) is int and n >= 0 for n in original["inventory"].values())
            and all(isinstance(original[k], str) and re.fullmatch(r"[0-9a-f]{64}", original[k])
                    for k in ("archiveSha256", "identity", "databaseSha256"))
            and type(original["databaseBytes"]) is int and 0 < original["databaseBytes"] <= 64 * 1024 * 1024,
            "invalid original observation")
    empty = seed_for_root(record["root"]) == "legacy_empty"
    expected_inventory = dict(zip(["sources", "capabilities", "revisions", "reviews", "recipes", "applications",
                                   "derivations", "workflows", "workflowRevisions"],
                                  [0] * 9 if empty else [1, 3, 4, 2, 1, 1, 2, 1, 2]))
    require(type(original["schema"]) is int and original["schema"] == (0 if empty else 4)
            and original["inventory"] == expected_inventory, "wrong original fixture inventory")
    if interrupted or expectation in RECOVERY_CONSENTS:
        require(all(s["vaultOpens"] <= 64 and s["vaultReads"] <= 64 and s["vaultWrites"] <= 1
                    for s in observations), "recovery custody bounds exceeded")
        no_client()
        first_approval = next((e["seq"] for e in events if e["event"] == "consent"
                               and e["fields"].get("outcome") == "approved"), float("inf"))
        for event in events:
            observed = (event["fields"] if event["event"] in {"state", "observed"}
                        else event["fields"]["observation"] if event["event"] == "startup"
                        else event["fields"]["state"] if event["event"] in {"ready", "exit"} else None)
            if observed is not None and event["seq"] < first_approval:
                require(all(observed[key] == 0 for key in ("vaultOpens", "vaultReads", "vaultWrites")),
                        "recovery reached vault before native approval")
        if interrupted:
            copying = expectation != "crash_reopened"
            stages = ordered("legacy_found", "conversion_review") if copying else ordered("verify_required")
            purpose = "copy_existing" if copying else "verify_copy"
            exact_consent(purpose, "approved")
            barrier_name = "provisioning_barrier" if expectation == "interrupted_provisioning" else "adoption_barrier"
            barrier = [e for e in events if e["event"] == barrier_name]
            require(len(barrier) == 1, "missing interruption checkpoint")
            consent = next(e for e in events if e["event"] == "consent")
            require(stages[-1]["seq"] < consent["seq"] < barrier[0]["seq"], "checkpoint preceded confirmation")
            custody = barrier[0]["fields"]
            require(all(type(custody.get(key)) is int and 1 <= custody[key] <= 64
                        for key in ("vaultOpens", "vaultReads"))
                    and type(custody.get("vaultWrites")) is int and custody["vaultWrites"] == (1 if copying else 0),
                    "checkpoint lacks real key custody")
            writes = fields("slot_write")
            require((len(writes) == 1 and writes[0].get("succeeded") is True) if copying else not writes,
                    "interruption key write mismatch")
            if copying:
                write = next(e for e in events if e["event"] == "slot_write")
                require(consent["seq"] < write["seq"] < barrier[0]["seq"], "key write preceded native approval")
            require(not any(e["fields"]["state"] in {"handoff", "restart_required"} for e in startup),
                    "interruption followed a completed startup outcome")
            require(not any(e["seq"] > barrier[0]["seq"] for e in events), "native activity continued past checkpoint")
            return
        expected = [{"purpose": purpose, "outcome": outcome} for purpose, outcome in RECOVERY_CONSENTS[expectation]]
        require(consents == expected, "wrong native recovery consent sequence")
        require(not fields("slot_write") and all(s["vaultWrites"] == 0 for s in observations), "recovery wrote a key")
        first = ordered("recovery_required")[0]
        consent_events = [e for e in events if e["event"] == "consent"]
        require(first["seq"] < consent_events[0]["seq"], "recovery preceded offered action")
        cancelled = expected[-1]["outcome"] == "cancelled"
        terminal_stage = "recovery_required" if cancelled else ("handoff" if expectation == "recovery_complete_selection" else "restart_required")
        require(any(e["seq"] > consent_events[-1]["seq"] and e["fields"]["state"] == terminal_stage for e in startup),
                "recovery lacks fresh outcome stage")
        require(all(e["fields"]["state"] != "handoff" or (expectation == "recovery_complete_selection"
                    and e["seq"] > consent_events[-1]["seq"]) for e in startup),
                "recovery published premature handoff")
        if expectation in {"recovery_inspect_cancel", "recovery_selection_cancel"}:
            no_vault()
        else:
            require(any(s["vaultReads"] > 0 and s["vaultOpens"] > 0 for s in observations), "recovery lacks real existing-key read")
        return
    if expectation == "legacy_copy":
        stages = ordered("legacy_found", "conversion_review", "restart_required")
        exact_consent("copy_existing", "approved")
        require(all(s["fields"]["retainedOriginal"] == "present_unencrypted" for s in stages[1:]),
                "copy lost retained-original disclosure")
        require(fields("slot_write") and len(fields("slot_write")) == 1
                and fields("slot_write")[0].get("succeeded") is True
                and any(s["vaultWrites"] == 1 for s in observations)
                and all(s["vaultWrites"] <= 1 for s in observations), "copy lacks exact real key write")
        no_client()
    else:
        require(not fields("slot_write") and all(s["vaultWrites"] == 0 for s in observations),
                "reopen/cancel phase wrote a key")
        if expectation == "legacy_verify":
            ordered("verify_required", "handoff")
            exact_consent("verify_copy", "approved")
            require(any(s["vaultReads"] > 0 for s in observations), "verification lacks real existing-key read")
            no_client()
        elif expectation == "legacy_verify_cancel":
            exact_consent("verify_copy", "cancelled")
            consent_seq = next(e["seq"] for e in events if e["event"] == "consent")
            require(any(s["seq"] < consent_seq and s["fields"]["state"] == "verify_required" for s in startup)
                    and any(s["seq"] > consent_seq and s["fields"]["state"] == "verify_required" for s in startup),
                    "cancel lacks fresh verification choice")
            require(not any(s["fields"]["state"] == "handoff" for s in startup), "cancel selected a copy")
            no_vault()
            no_client()
        elif expectation == "legacy_relaunch":
            ordered("handoff")
            exact_consent("unlock", "approved")
            require(any(s["close"]["state"] == "open" and s["clientPresent"] is True
                        and s["vaultReads"] > 0 for s in observations), "selected copy did not unlock")
        else:
            stages = ordered("legacy_found", "conversion_review")
            if expectation in {"legacy_review_cancel", "legacy_copy_cancel"}:
                stages = ordered("legacy_found", "conversion_review", "legacy_found")
            if expectation == "legacy_copy_cancel":
                exact_consent("copy_existing", "cancelled")
                consent_seq = next(e["seq"] for e in events if e["event"] == "consent")
                require(stages[1]["seq"] < consent_seq < stages[2]["seq"], "copy Cancel lacks returned choice")
            else:
                require(not consents, "review-only phase opened a native mutation prompt")
            no_vault()
            no_client()


def verify_recovery_snapshots(case, events):
    require(not any(e["event"] == "recovery_snapshot_failed" for e in events), "recovery snapshot failed")
    snapshots = [e["fields"] for e in events if e["event"] == "recovery_snapshot"]
    require(len(snapshots) <= 1 and (not snapshots or case in {"authoring", "relaunch"}), "invalid recovery snapshot scope/count")
    for fields in snapshots:
        require(isinstance(fields, dict) and set(fields) == {"backupId", "byteLength"}
                and isinstance(fields["backupId"], str)
                and re.fullmatch(r"backup:[0-9a-f]{64}", fields["backupId"]) is not None
                and type(fields["byteLength"]) is int and 0 < fields["byteLength"] <= 68 * 1024 * 1024,
                "invalid recovery snapshot receipt")


def verify_exit(case, code, events, killed, require_headers=False, adoption_checkpoint=None, storage_before=None, storage_after=None):
    """Enforce phase exit evidence; this does not assert the manual UI journey."""
    verify_recovery_snapshots(case, events)
    require(not require_headers or case == "authoring", "header probe requires authoring case")
    by_name = lambda name: [e["fields"] for e in events if e["event"] == name]
    require(len(by_name("started")) == 1, "missing native start")
    require(by_name("started")[0].get("casePlanSha256") == PLAN_SHA256, "native case plan mismatch")
    if case in ("cleanup", "lose-key"):
        require(code == 0 and len(by_name("cleanup_complete")) == 1
                and not by_name("slot_cleanup_failed"), "exact slot cleanup did not complete")
        if case == "lose-key":
            require(len(by_name("slot_absent")) == 1 and by_name("slot_absent")[0]["root"] == "authoring",
                    "missing exact authoring key-loss receipt")
        return
    require(len(by_name("context_validated")) == 1, "native isolation context not validated")
    require(len(by_name("ready")) == 1, "missing native Ready")
    expected = ADOPTION_CHECKPOINTS.get(case_record(case)["expectation"])
    barriers = by_name("adoption_barrier")
    require(not by_name("adoption_barrier_timeout") and not by_name("provisioning_barrier_timeout"),
            "interruption checkpoint timed out")
    if expected:
        require(killed and code == -signal.SIGKILL and adoption_checkpoint == expected
                and len(barriers) == 1 and set(barriers[0]) == {"stage", "boundary", "vaultOpens", "vaultReads", "vaultWrites"}
                and (barriers[0]["stage"], barriers[0]["boundary"]) == expected
                and not by_name("exit") and not by_name("provisioning_barrier"), "wrong adoption interruption")
        verify_workflow(case, events)
        verify_storage(case, events, storage_before, storage_after)
        return
    require(not barriers and adoption_checkpoint is None, "unexpected adoption checkpoint")
    provisioning = case_record(case)["expectation"] == "interrupted_provisioning"
    if case == "interrupted-setup" or provisioning:
        checkpoint = by_name("provisioning_barrier")
        require(killed and code == -signal.SIGKILL and len(by_name("provisioning_barrier")) == 1
                and len(by_name("slot_write")) == 1 and by_name("slot_write")[0]["succeeded"] is True
                and checkpoint[0].get("afterRealKeyWrite") is True
                and not by_name("exit"), "interruption did not follow real provisioning write")
        if provisioning:
            require(set(checkpoint[0]) == {"afterRealKeyWrite", "vaultOpens", "vaultReads", "vaultWrites"},
                    "wrong provisioning checkpoint fields")
            verify_workflow(case, events)
            verify_storage(case, events, storage_before, storage_after)
        return
    require(not by_name("provisioning_barrier") and not killed and len(by_name("exit")) == 1,
            "missing actual native Exit")
    terminal = by_name("exit")[0]
    if case == "busy-owner":
        require(code == (0 if terminal["clean"] else 4), "unexpected busy-owner exit status")
        require(any(s.get("state") == "unavailable" for s in by_name("startup")),
                "busy Owner lacks an actual native startup refusal")
        states = (by_name("state") + by_name("observed")
                  + [s["observation"] for s in by_name("startup")]
                  + [by_name("ready")[0]["state"], terminal["state"]])
        require(all(s["vaultOpens"] == 0 and s["vaultReads"] == 0 and s["vaultWrites"] == 0
                    and not s["clientPresent"] for s in states),
                "busy Owner reached a vault or content client")
        return
    headers = [e for e in events if e["event"].startswith("header_")]
    require(not require_headers or bool(headers), "required header probe did not run")
    if headers:
        require(case == "authoring" and [e["event"] for e in headers] == [
            "header_client_captured", "header_probe_started", "header_probe_missing",
            "header_probe_stale", "header_probe_current"], "incomplete or failed header probe")
        require(headers[0]["fields"] == {"open": True} and headers[0]["fields"]["open"] is True
                and headers[1]["fields"] == {"differentClient": True} and headers[1]["fields"]["differentClient"] is True, "invalid header generation evidence")
        require(any(headers[0]["seq"] < e["seq"] < headers[1]["seq"]
                    and e["event"] in ("state", "observed")
                    and e["fields"]["close"]["state"] == "locked"
                    and e["fields"]["clientPresent"] is False for e in events),
                "header probe lacks intervening Locked evidence")
        for negative in headers[2:4]:
            require(negative["fields"] == {"matched": True, "vaultReadDelta": 0, "vaultWriteDelta": 0}
                    and negative["fields"]["matched"] is True
                    and type(negative["fields"]["vaultReadDelta"]) is int
                    and type(negative["fields"]["vaultWriteDelta"]) is int,
                    "header refusal reached vault or failed its exact oracle")
        positive = headers[4]["fields"]
        require(set(positive) == {"matched", "vaultReadDelta", "vaultWriteDelta"}
                and positive["matched"] is True and type(positive["vaultReadDelta"]) is int
                and positive["vaultReadDelta"] > 0 and type(positive["vaultWriteDelta"]) is int
                and positive["vaultWriteDelta"] == 0,
                "header positive control lacks real read")
    barriers = [e for e in events if e["event"].startswith("read_barrier_")]
    if barriers:
        require(case == "authoring" and [e["event"] for e in barriers] == [
            "read_barrier_armed", "read_barrier_held", "read_barrier_released", "read_barrier_read_returned"],
            "incomplete or failed read barrier")
        armed, held, released, returned = [e["fields"] for e in barriers]
        require(armed["close"]["state"] == "open" and armed["clientPresent"] is True
                and held.get("beforeOsRead") is True
                and released["close"]["state"] == "closing" and released["clientPresent"] is False
                and released["requestState"] == "unavailable" and released["failedClosed"] is False
                and returned.get("present") is True,
                "read barrier did not cross real closed admission")
        require(any(e["seq"] > barriers[-1]["seq"] and e["event"] in ("state", "observed")
                    and e["fields"]["close"]["state"] == "locked" and e["fields"]["clientPresent"] is False
                    for e in events), "read barrier lacks post-completion Locked evidence")
    if case in ("anchor-failure", "cleanup-failure", "window-failure"):
        ready = by_name("ready")[0]
        require(code == 4 and terminal["clean"] is False and terminal["failedClosed"] is True
                and ready["windowPresented"] is False and ready["state"]["clientPresent"] is False
                and all(s["vaultOpens"] == 0 and s["vaultReads"] == 0 and s["vaultWrites"] == 0
                        and not s["clientPresent"] for s in (ready["state"], terminal["state"])),
                "startup failure did not stay fenced")
    elif case == "missing-key":
        require(code == (0 if terminal["clean"] else 4), "unexpected missing-key exit status")
        states = by_name("state") + by_name("observed") + [terminal["state"]]
        require(any(s["vaultReads"] > 0 for s in states)
                and all(s["vaultWrites"] == 0 and not s["clientPresent"] for s in states),
                "missing-key case lacks closed read-only custody evidence")
    else:
        require(code == 0 and terminal["clean"] is True and terminal["failedClosed"] is False,
                "phase did not exit cleanly")
        if case in ("cold-cancel", "cold-relaunch"):
            states = by_name("state") + by_name("observed") + [by_name("ready")[0]["state"], terminal["state"]]
            require(any(s["close"]["state"] == "locked" for s in states)
                    and not by_name("slot_write")
                    and all(s["vaultOpens"] == 0 and s["vaultReads"] == 0 and s["vaultWrites"] == 0
                            and not s["clientPresent"] for s in states),
                    "cold-cancel must remain locked without a vault/client")
    verify_workflow(case, events)
    verify_storage(case, events, storage_before, storage_after)


def controls_for_case(case):
    controls = ("observe", "quit")
    if case == "authoring":
        controls += ("arm-read", "release-read", "capture-client", "check-headers")
    if case in {"authoring", "relaunch"}:
        controls += ("recovery-snapshot",)
    return controls


def run(nonce, case, require_headers=False):
    root = validate_run(nonce)
    artifact = read_record(root / "artifact.json")
    require(artifact.get("casePlanSha256") == PLAN_SHA256, "artifact case plan mismatch")
    executable = Path(artifact["executable"])
    require(executable == root / BUNDLE / "Contents/MacOS/native-encrypted-qualification"
            and not executable.is_symlink(), "invalid executable")
    for name in (BUNDLE, BUNDLE + "/Contents", BUNDLE + "/Contents/MacOS"):
        require(identity(root / name) == artifact["wrapperDirectories"][name], "application wrapper replaced")
    info = root / BUNDLE / "Contents/Info.plist"
    require(not info.is_symlink() and info.read_bytes() == bundle_info()
            and digest(info) == artifact["infoPlistSha256"], "application metadata changed")
    require(digest(executable) == artifact["executableSha256"] and source_digest() == artifact["sourceSha256"],
            "artifact/source changed; create a new run")
    permit(root, nonce, case)
    events, stderr, pending, total = [], bytearray(), bytearray(), 0
    recovery_snapshot_requested = False
    killed, failure, kill_checkpoint = False, None, None
    storage_before = storage_after = None
    if captures_storage(case) and case_record(case)["prerequisite"]["kind"] != "initial":
        storage_before = capture_storage(root, nonce, case)
        previous = prior_receipt(root, case_record(case)["prerequisite"]["case"])
        require(storage_before == previous.get("storageAfter"), "storage changed between phases")
    child = subprocess.Popen([str(executable), nonce, case], cwd=REPO, env=safe_environment(),
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    print(json.dumps({"childPid": child.pid, "case": case, "controls": list(controls_for_case(case))}), flush=True)
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ, "out")
    selector.register(child.stderr, selectors.EVENT_READ, "err")
    if sys.stdin.isatty():
        selector.register(sys.stdin, selectors.EVENT_READ, "in")
    deadline = time.monotonic() + 1200
    try:
        prerequisite = case_record(case)["prerequisite"]
        if prerequisite["kind"] == "completed":
            previous = read_record(root / ("receipt-" + prerequisite["case"] + ".json"))
            require(child.pid != previous["pid"], "phase reused prior process identity")
        while child.poll() is None or any(k.data in ("out", "err") for k in selector.get_map().values()):
            require(time.monotonic() < deadline, "case exceeded 20-minute native UI limit")
            for key, _ in selector.select(0.25):
                if key.data == "in":
                    line = sys.stdin.readline(65)
                    allowed = tuple(control + "\n" for control in controls_for_case(case))
                    require(line in allowed, "only exact phase controls allowed")
                    if line == "recovery-snapshot\n":
                        require(not recovery_snapshot_requested, "recovery snapshot is one-shot")
                        recovery_snapshot_requested = True
                    if child.poll() is None:
                        child.stdin.write(line.encode())
                        child.stdin.flush()
                    continue
                chunk = os.read(key.fileobj.fileno(), 8192)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                total += len(chunk)
                require(total <= LIMIT, "child output exceeded bound")
                if key.data == "err":
                    stderr.extend(chunk)
                    continue
                pending.extend(chunk)
                require(len(pending) <= 16384, "event line exceeded bound")
                while b"\n" in pending:
                    line, _, tail = pending.partition(b"\n")
                    pending = bytearray(tail)
                    event = json_exact(line)
                    require(event.get("schema") == "rangoon.e1c4.event.v1" and event.get("pid") == child.pid
                            and event.get("case") == case and event.get("seq") == len(events) + 1,
                            "invalid native event identity/sequence")
                    events.append(event)
                    require(len(events) <= 512, "event count exceeded bound")
                    print(line.decode(), flush=True)
                    if event["event"] == "provisioning_barrier":
                        require(not killed and (case == "interrupted-setup" or
                                case_record(case)["expectation"] == "interrupted_provisioning")
                                and event["fields"].get("afterRealKeyWrite") is True,
                                "unexpected provisioning barrier")
                        child.kill()
                        killed = True
                    elif event["event"] == "adoption_barrier":
                        expected = ADOPTION_CHECKPOINTS.get(case_record(case)["expectation"])
                        require(not killed and expected is not None
                                and (event["fields"].get("stage"), event["fields"].get("boundary")) == expected,
                                "unexpected adoption barrier")
                        child.kill()
                        killed, kill_checkpoint = True, expected
            if child.poll() is not None:
                deadline = min(deadline, time.monotonic() + 5)
        require(not pending, "truncated native event")
        code = child.wait(timeout=5)
        if captures_storage(case):
            storage_after = capture_storage(root, nonce, case)
        verify_exit(case, code, events, killed, require_headers, kill_checkpoint, storage_before, storage_after)
        require(digest(executable) == artifact["executableSha256"]
                and source_digest() == artifact["sourceSha256"], "artifact/source changed during phase")
    except (Exception, KeyboardInterrupt) as error:
        failure = type(error).__name__ + ": " + str(error)
        if child.poll() is None:
            child.kill()
    finally:
        child.wait(timeout=10)
        selector.close()
        for stream in (child.stdin, child.stdout, child.stderr):
            stream.close()
        # Fixed native diagnostics only; unexpected stderr is bounded but retained
        # locally so an OS failure cannot disappear from the qualification record.
        receipt = {"schema": "rangoon.e1c4.receipt.v1", "case": case, "pid": child.pid,
                   "returncode": child.returncode, "events": events, "stderr": stderr.decode(errors="replace"),
                   "killedAtProvisioningBarrier": killed and kill_checkpoint is None,
                   "killedAtAdoptionBarrier": killed and kill_checkpoint is not None,
                   "killCheckpoint": kill_checkpoint, "storageBefore": storage_before, "storageAfter": storage_after,
                   "parentFailure": failure,
                   "phaseExitVerified": failure is None,
                   "requiredHeaderProbe": require_headers,
                   "headerProbeVerified": failure is None and any(e["event"] == "header_probe_current" for e in events),
                   "executableSha256": artifact["executableSha256"], "sourceSha256": artifact["sourceSha256"],
                   "casePlanSha256": PLAN_SHA256,
                   "architecture": artifact["architecture"], "os": artifact["os"],
                   "artifactRecordSha256": digest(root / "artifact.json"),
                   "wrapperInfoPlistSha256": artifact["infoPlistSha256"],
                   "nativeCaseVerdict": "unassessed; inspect UI evidence and case-specific assertions"}
        write_new(root / ("receipt-" + case + ".json"), receipt)
    print(json.dumps({"case": case, "returncode": child.returncode, "parentFailure": failure,
                      "receipt": str(root / ("receipt-" + case + ".json"))}), flush=True)
    if failure:
        fail(failure)


def main():
    require(__debug__, "Python optimization is forbidden")
    require(sys.platform == "darwin", "macOS qualification only")
    args = sys.argv[1:]
    if args == ["init"]:
        init()
    elif len(args) == 4 and args[0] == "run" and args[2:] == ["authoring", "--require-headers"]:
        run(args[1], args[2], require_headers=True)
    elif len(args) == 3 and args[0] == "run":
        run(args[1], args[2])
    else:
        fail("usage: qualify-native-encrypted.py init | run NONCE CASE")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print("e1c4 runner: " + str(error), file=sys.stderr)
        sys.exit(64)
