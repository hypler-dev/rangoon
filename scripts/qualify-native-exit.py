#!/usr/bin/env python3
"""Build and qualify only the explicit NX-06 test executable on macOS."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]
CASES = ("idle-clean", "active-cancel", "duplicate-timeout-retry", "cleanup-failed",
         "anchor-unavailable", "anchor-install-failed")
LIMIT = 65536
OWNED_INPUTS = (
    "apps/desktop/Cargo.toml", "apps/desktop/Cargo.lock", "Cargo.lock",
    "apps/desktop/src/main.rs", "apps/desktop/src/cloud_models.rs",
    "apps/desktop/src/openshell_host.rs", "apps/desktop/src/native_exit_qualification.rs",
    "apps/desktop/native-exit-qualification/tauri.conf.json",
    "scripts/qualify-native-exit.py",
)


def source_identity():
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    paths = sorted(set(p for p in tracked if p) | set(OWNED_INPUTS))
    hashes = {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest()
              for p in paths if (ROOT / p).is_file()}
    return {
        "files": hashes,
        "status": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True),
        "unstagedDiffSha256": hashlib.sha256(subprocess.check_output(["git", "diff", "--binary"], cwd=ROOT)).hexdigest(),
        "stagedDiffSha256": hashlib.sha256(subprocess.check_output(["git", "diff", "--cached", "--binary"], cwd=ROOT)).hexdigest(),
    }



def capture(executable, args):
    """Bound each pipe; stop/join only the child owned by this invocation."""
    child = subprocess.Popen([str(executable), *args], cwd=ROOT, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE)
    buffers = [bytearray(), bytearray()]
    overflow = threading.Event()

    def read(stream, target):
        try:
            while True:
                chunk = stream.read(4096)
                if not chunk:
                    return
                remaining = LIMIT - len(target)
                target.extend(chunk[:remaining])
                if len(chunk) > remaining:
                    overflow.set()
                    child.kill()
                    return
        finally:
            stream.close()

    readers = [threading.Thread(target=read, args=(pipe, target), daemon=True)
               for pipe, target in zip((child.stdout, child.stderr), buffers)]
    for reader in readers:
        reader.start()
    timed_out = False
    try:
        child.wait(timeout=45)
    except subprocess.TimeoutExpired:
        timed_out = True
        child.kill()
        child.wait()
    for reader in readers:
        reader.join(timeout=2)
    return {"code": child.returncode, "timeout": timed_out,
            "overflow": overflow.is_set(), "readerAlive": any(t.is_alive() for t in readers),
            "stdout": bytes(buffers[0]).decode("utf-8", errors="strict"),
            "stderr": bytes(buffers[1]).decode("utf-8", errors="strict")}


def verify(case, result):
    assert not any(result[k] for k in ("timeout", "overflow", "readerAlive")), "incomplete capture"
    expected = 4 if case == "cleanup-failed" else 37 if case.startswith("anchor-") else 0
    assert result["code"] == expected, f"exit {result['code']} expected {expected}"
    assert result["stderr"] == ("nx06: test panic\n" if case == "cleanup-failed" else ""), "unexpected stderr"
    rows = [json.loads(line) for line in result["stdout"].splitlines()]
    assert 4 <= len(rows) <= 128
    allowed = {"context_validated", "ready", "production_unavailable", "peer_stopped", "request_exit",
               "exit_request", "exit", "watch_started", "client_returned", "held_roundtrip",
               "duplicates_roundtrip", "waiter_started", "waiter_finished", "timeout_roundtrip",
               "release_callback", "callback_drained", "retry_exit"}
    for seq, row in enumerate(rows, 1):
        assert set(row) == {"seq", "case", "event", "fields"}
        assert row["seq"] == seq and row["case"] == case
        assert row["event"] in allowed and isinstance(row["fields"], dict)
    events = [r["event"] for r in rows]

    def one(name):
        found = [r for r in rows if r["event"] == name]
        assert len(found) == 1, f"expected exactly one {name}"
        return found[0]

    def order(*names):
        positions = [one(n)["seq"] for n in names]
        assert positions == sorted(positions), f"wrong event order: {names}"

    assert events[0] == "context_validated" and events[-1] == "exit"
    assert one("context_validated")["fields"] == {"emptyAssets": True, "effectiveGrants": 0, "negativeChecks": 9}
    ready = one("ready")["fields"]
    assert ready["anchor"] == (not case.startswith("anchor-"))
    assert ready["failedClosed"] == (case == "anchor-install-failed")
    assert one("production_unavailable")["fields"]["rpcs"] == 0
    assert one("request_exit")["fields"]["code"] == 37
    order("context_validated", "ready", "production_unavailable", "request_exit", "exit")
    exits = [r for r in rows if r["event"] == "exit_request"]
    assert exits and exits[0]["fields"]["code"] == 37
    assert exits[-1]["fields"]["code"] == expected
    assert exits[-1]["fields"]["action"] == "pass"
    if case.startswith("anchor-"):
        assert len(exits) == 1 and not exits[0]["fields"]["clean"]
        assert one("production_unavailable")["fields"]["connections"] == 0
        assert not set(events) & {"watch_started", "waiter_started", "release_callback"}
    else:
        assert all(r["fields"]["admissionClosed"] for r in exits)
        assert exits[-1]["fields"]["clean"] == (expected == 0)
        starts = [r for r in exits if r["fields"]["action"] == "start"]
        assert len(starts) == 1 and not starts[0]["fields"]["clean"]
        assert starts[0]["seq"] < exits[-1]["seq"]
    if case in ("active-cancel", "duplicate-timeout-retry"):
        order("watch_started", "request_exit", "client_returned", "held_roundtrip", "peer_stopped", "release_callback", "exit")
        assert one("watch_started")["fields"]["rpcs"] == 3
        assert one("client_returned")["fields"]["cancelled"] is True
        assert one("held_roundtrip")["fields"] == {"admissionClosed": True, "callbackHeld": True}
        assert exits[0]["fields"]["action"] == "wait"
        assert starts[0]["seq"] > one("release_callback")["seq"]
        began = one("waiter_started")
        finished = one("waiter_finished")
        assert began["seq"] < finished["seq"] and began["fields"]["active"] == 1
        assert finished["fields"]["active"] == 0
        if case == "active-cancel":
            assert finished["fields"]["drained"] is True
            assert finished["seq"] > one("release_callback")["seq"]
        else:
            assert finished["fields"]["checks"] == 200 and finished["fields"]["drained"] is False
            assert len([r for r in exits if r["fields"]["action"] == "wait"]) == 4
            order("held_roundtrip", "duplicates_roundtrip", "waiter_finished", "timeout_roundtrip", "release_callback", "callback_drained", "retry_exit", "exit")
            assert one("timeout_roundtrip")["fields"]["callbackHeld"] is True
    else:
        assert "client_returned" not in events
        assert one("production_unavailable")["fields"]["connections"] == 0
    return {"case": case, "events": len(rows), "code": result["code"], "passed": True}


def main():
    if not __debug__:
        raise SystemExit("NX-06 requires Python assertion checks; optimized mode is refused")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("NX-06 native qualification is macOS-only")
    evidence = args.evidence or Path(tempfile.mkdtemp(prefix="rangoon-nx06-"))
    evidence.mkdir(parents=True, exist_ok=True)
    # Refuse overwrite so every failed native run remains reviewable.
    if (evidence / "build.jsonl").exists():
        parser.error("use a fresh evidence directory")
    command = ["cargo", "test", "--manifest-path", "apps/desktop/Cargo.toml", "--test",
               "native-exit-qualification", "--features", "native-exit-qualification,encrypted-workspace",
               "--locked", "--offline", "--no-run", "--message-format=json"]
    inputs = source_identity()
    (evidence / "source-before-build.json").write_text(json.dumps(inputs, indent=2) + "\n")
    with (evidence / "build.jsonl").open("wb") as out, (evidence / "build.stderr").open("wb") as err:
        build = subprocess.run(command, cwd=ROOT, stdout=out, stderr=err, check=False)
    assert build.returncode == 0, "custom target build failed"
    artifacts = []
    for line in (evidence / "build.jsonl").read_text().splitlines():
        row = json.loads(line)
        if (row.get("reason") == "compiler-artifact" and row["target"]["name"] == "native-exit-qualification"
                and row["target"]["kind"] == ["test"] and row.get("executable")):
            artifacts.append(Path(row["executable"]))
    assert len(artifacts) == 1, "exact custom target artifact missing/ambiguous"
    executable = artifacts[0]
    assert source_identity() == inputs, "source changed during build"
    metadata = {"source": inputs, "platform": platform.platform(), "machine": platform.machine(), "command": command,
                "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip(),
                "executableFormat": subprocess.check_output(["file", str(executable)], text=True).strip(),
                "executable": str(executable), "sha256": hashlib.sha256(executable.read_bytes()).hexdigest()}
    (evidence / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    for i, invalid in enumerate(([], ["invalid"], ["idle-clean", "extra"])):
        result = capture(executable, invalid)
        (evidence / f"invalid-{i}.json").write_text(json.dumps(result, indent=2) + "\n")
        assert result == {"code": 64, "timeout": False, "overflow": False, "readerAlive": False,
                          "stdout": "", "stderr": "nx06: one exact case required\n"}
    results = []
    for i, case in enumerate((*CASES, "active-cancel", "duplicate-timeout-retry")):
        result = capture(executable, [case])
        (evidence / f"{i:02}-{case}.json").write_text(json.dumps(result, indent=2) + "\n")
        results.append(verify(case, result))
        print(f"PASS {case}: exit {result['code']}", flush=True)
    assert source_identity() == inputs, "source changed during native qualification"
    (evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(f"Evidence: {evidence}")


if __name__ == "__main__":
    main()
