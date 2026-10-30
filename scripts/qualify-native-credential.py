#!/usr/bin/env python3
"""Build and qualify bounded C1-M native credential cases.

Usage: python3 scripts/qualify-native-credential.py --repo /absolute/rangoon
  --toolchain-bin /absolute/rust/bin --evidence /absolute/fresh-evidence
  [--case CASE] [--timeout SECONDS]
"""
import argparse
import hashlib
import json
import os
import plistlib
from pathlib import Path
import queue
import re
import secrets
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone

SCHEMA = "rangoon.native-credential-qualification.v2"
PLAN_SCHEMA = "rangoon.native-credential-cases.v1"
CASES = ("accept-read", "cancel-review", "stop-queued", "stop-open", "expire-open", "parent-close")
COMMANDS = frozenset(("stop", "hold-main", "release-main", "close-parent"))
LINE_LIMIT, EVENT_LIMIT, OUTPUT_LIMIT, STDERR_LIMIT, MAX_TIMEOUT = 8192, 128, 1024 * 1024, 1024 * 1024, 200
CONFIG = "apps/desktop/native-credential-qualification/tauri.conf.json"
CASE_PLAN = "apps/desktop/native-credential-qualification/cases.v1.json"
RUNNER = "scripts/qualify-native-credential.py"
TARGET = "native-credential-qualification"
EXECUTABLE_RE = re.compile(r"native_credential_qualification-[0-9a-f]+$")
BUNDLE_NAME = "Rangoon C1-M.app"
BUNDLE_EXECUTABLE = "native-credential-qualification"
BUNDLE_IDENTIFIER = "ai.rangoon.native-credential-qualification"
TOOL_KEYS = ("cargo", "rustc", "rustdoc", "cargo_fmt", "rustfmt", "cargo_clippy", "clippy_driver")
NODE_HELPER = r'''import { readFileSync } from 'node:fs'; import { pathToFileURL } from 'node:url';
const [helperPath, repo, toolchainBin] = process.argv.slice(2); const helper = await import(pathToFileURL(helperPath).href);
const config = helper.validateConfiguration({toolchainToml:readFileSync(`${repo}/rust-toolchain.toml`,'utf8'),workspaceToml:readFileSync(`${repo}/Cargo.toml`,'utf8'),desktopToml:readFileSync(`${repo}/apps/desktop/Cargo.toml`,'utf8'),nativeInputToml:readFileSync(`${repo}/apps/desktop/native-input/Cargo.toml`,'utf8'),ciRustVersion:process.env.RANGOON_RUST_VERSION});
const resolved=helper.resolveToolchain({pin:config.pin,toolchainBin,cwd:repo}); const childEnv=helper.prepareChildEnv({pin:config.pin,bin:resolved.bin,tools:resolved.tools}); const identities=[]; const log=console.log; console.log=value=>identities.push(String(value)); helper.verifyToolIdentities({pin:config.pin,clippyVersion:config.clippyVersion,tools:resolved.tools,cwd:repo,env:childEnv}); console.log=log;
const env=Object.fromEntries(['PATH','RUSTUP_TOOLCHAIN','RUSTUP_AUTO_INSTALL','CARGO','RUSTC','RUSTDOC','RUSTFMT','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'].map(key=>[key,childEnv[key]??''])); process.stdout.write(JSON.stringify({pin:config.pin,bin:resolved.bin,tools:resolved.tools,env,identities})+'\n');'''

class QualificationError(RuntimeError): pass
class DuplicateKeyError(ValueError): pass

def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(65536), b""): digest.update(block)
    return digest.hexdigest()

def write_json_new(path, value):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True); output.write("\n")

def no_duplicate_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise DuplicateKeyError()
        result[key] = value
    return result

def exact_root(repo):
    supplied = Path(os.path.abspath(repo))
    if not supplied.is_dir() or supplied.is_symlink() or supplied.resolve() != supplied or not (supplied / ".git").exists(): raise QualificationError("explicit non-symlink repository root required")
    top = Path(subprocess.check_output(("git", "rev-parse", "--show-toplevel"), cwd=supplied, text=True).strip())
    if top.resolve() != supplied: raise QualificationError("repository root must be explicit")
    return supplied

def repo_files(repo):
    result = subprocess.run(("git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"), cwd=repo, check=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    return sorted(set(item for item in result.stdout.decode("utf-8").split("\0") if item) | {CONFIG, CASE_PLAN, RUNNER})

def source_manifest(repo):
    repo, files = exact_root(repo), {}
    for relative in repo_files(repo):
        candidate = repo / relative
        if not candidate.is_file() or candidate.is_symlink() or candidate.resolve() != candidate: raise QualificationError("source manifest requires regular non-symlink files")
        files[relative] = sha256_file(candidate)
    return {"schema":"rangoon.native-credential-source.v1","repo":str(repo),"files":files}

def verify_plan(path):
    try: plan = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_object)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, DuplicateKeyError) as error: raise QualificationError("invalid case plan") from error
    if set(plan) != {"schema","cases"} or plan["schema"] != PLAN_SCHEMA or not isinstance(plan["cases"],list) or len(plan["cases"]) != 6: raise QualificationError("invalid case plan")
    ids = [item.get("id") if isinstance(item,dict) and set(item)=={"id"} else None for item in plan["cases"]]
    if tuple(ids) != CASES: raise QualificationError("invalid case plan")
    return tuple(ids)

def strictly_equal(actual, expected):
    if type(actual) is not type(expected): return False
    if isinstance(expected,dict): return set(actual)==set(expected) and all(strictly_equal(actual[key],value) for key,value in expected.items())
    return actual == expected
def is_int(value): return type(value) is int
def exact_fields(fields, expected): return isinstance(fields,dict) and strictly_equal(fields,expected)

def toolchain_record(repo, toolchain_bin, node=None):
    if not Path(toolchain_bin).is_absolute(): raise QualificationError("toolchain bin must be absolute")
    node_path = node or shutil.which("node")
    if not node_path or not Path(node_path).is_absolute(): raise QualificationError("absolute Node executable required")
    command = (str(Path(node_path).resolve()),"--input-type=module","--eval",NODE_HELPER,"rangoon-native-credential-preflight",str(repo / "scripts/check-source-rust.mjs"),str(repo),str(Path(toolchain_bin)))
    result = subprocess.run(command,cwd=repo,env=os.environ.copy(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=False)
    if result.returncode != 0: raise QualificationError("pinned toolchain identity or environment refused")
    try: payload=json.loads(result.stdout.decode("utf-8"),object_pairs_hook=no_duplicate_object)
    except (UnicodeDecodeError,json.JSONDecodeError,DuplicateKeyError) as error: raise QualificationError("invalid pinned toolchain response") from error
    if not isinstance(payload,dict) or set(payload)!={"pin","bin","tools","env","identities"} or payload["pin"]!="1.99.1" or not Path(payload["bin"]).is_absolute() or not isinstance(payload["tools"],dict) or set(payload["tools"])!=set(TOOL_KEYS) or not isinstance(payload["env"],dict) or not isinstance(payload["identities"],list) or len(payload["identities"])!=7: raise QualificationError("invalid pinned toolchain response")
    if any(not isinstance(value,str) or not Path(value).is_absolute() for value in payload["tools"].values()): raise QualificationError("invalid pinned toolchain response")
    env=os.environ.copy()
    for key in list(env):
        if key.lower()=="path": del env[key]
    env.update(payload["env"])
    if env.get("CARGO")!=payload["tools"]["cargo"] or env.get("RUSTC_WRAPPER")!="" or env.get("RUSTC_WORKSPACE_WRAPPER")!="": raise QualificationError("pinned toolchain environment refused")
    return {"timestamp":datetime.now(timezone.utc).isoformat(),"node":str(Path(node_path).resolve()),"pin":payload["pin"],"bin":payload["bin"],"tools":payload["tools"],"identities":payload["identities"],"env":env}

def build_command(toolchain):
    return (toolchain["tools"]["cargo"],"test","--manifest-path","apps/desktop/Cargo.toml","--test",TARGET,"--features",f"{TARGET},encrypted-workspace","--locked","--offline","--no-run","--message-format=json")

def select_artifact(repo,records):
    matches=[]; allowed=(repo/"apps/desktop/target/debug/deps").resolve()
    for record in records:
        if not isinstance(record,dict): continue
        target, executable=record.get("target"),record.get("executable")
        if record.get("reason")!="compiler-artifact" or not isinstance(target,dict) or target.get("name")!=TARGET or target.get("kind")!=["test"] or not isinstance(executable,str): continue
        path=Path(executable)
        if not path.is_absolute() or path.is_symlink() or path.resolve() != path: raise QualificationError("artifact executable must be absolute regular file")
        path=path.resolve(strict=True)
        try: path.relative_to(allowed)
        except ValueError as error: raise QualificationError("artifact executable outside debug deps") from error
        if not EXECUTABLE_RE.fullmatch(path.name): raise QualificationError("artifact executable name refused")
        matches.append(path)
    if len(matches)!=1: raise QualificationError("exact custom test artifact required")
    return matches[0]

def build(repo,evidence,manifest,toolchain):
    command=build_command(toolchain); build_path,stderr_path=evidence/"build.jsonl",evidence/"build.stderr"
    with build_path.open("xb") as output,stderr_path.open("xb") as stderr: result=subprocess.run(command,cwd=repo,env=toolchain["env"],stdout=output,stderr=stderr,check=False)
    if result.returncode!=0: raise QualificationError("locked offline custom target build failed")
    try: records=[json.loads(line,object_pairs_hook=no_duplicate_object) for line in build_path.read_text(encoding="utf-8").splitlines()]
    except (UnicodeDecodeError,json.JSONDecodeError,DuplicateKeyError) as error: raise QualificationError("invalid cargo JSON output") from error
    executable=select_artifact(repo,records)
    if source_manifest(repo)!=manifest: raise QualificationError("source changed during build")
    artifact={"schema":"rangoon.native-credential-artifact.v1","target":{"name":TARGET,"kind":["test"]},"executable":str(executable),"sha256":sha256_file(executable),"command":list(command),"timestamp":datetime.now(timezone.utc).isoformat(),"toolchainPin":toolchain["pin"]}
    write_json_new(evidence/"artifact.json",artifact); return executable,artifact

def bundle_info():
    return plistlib.dumps({"CFBundleIdentifier":BUNDLE_IDENTIFIER,
        "CFBundleName":"Rangoon C1-M","CFBundleDisplayName":"Rangoon C1-M",
        "CFBundleExecutable":BUNDLE_EXECUTABLE,"CFBundlePackageType":"APPL",
        "CFBundleVersion":"1","NSHighResolutionCapable":True})

def verify_native_bundle(executable, artifact, evidence):
    try:
        expected = evidence / BUNDLE_NAME / "Contents" / "MacOS" / BUNDLE_EXECUTABLE
        source_value, digest = artifact.get("sourceExecutable"), artifact.get("sha256")
        if (executable != expected or artifact.get("executable") != str(expected)
                or artifact.get("launchScope") != "disposable-app-wrapper"
                or not isinstance(source_value,str) or not isinstance(digest,str)
                or not re.fullmatch(r"[0-9a-f]{64}",digest)):
            raise QualificationError("invalid native bundle identity")
        source, info = Path(source_value), expected.parent.parent / "Info.plist"
        directories = (evidence, evidence / BUNDLE_NAME, expected.parent.parent, expected.parent)
        if (any(not p.is_absolute() or not p.is_dir() or p.is_symlink() or p.resolve() != p for p in directories)
                or any(not p.is_absolute() or not p.is_file() or p.is_symlink() or p.resolve() != p for p in (source,executable,info))):
            raise QualificationError("native bundle requires exact regular paths")
        if info.read_bytes() != bundle_info() or artifact.get("infoPlistSha256") != sha256_file(info):
            raise QualificationError("native bundle metadata changed")
        if sha256_file(source) != digest: raise QualificationError("compiled artifact changed")
        if sha256_file(executable) != digest: raise QualificationError("native bundle executable changed")
    except OSError as error:
        raise QualificationError("native bundle inspection failed") from error

def prepare_native_bundle(source, artifact, evidence):
    try:
        # As in E1c4, this is an addressable disposable test parent, not an installer
        # or product package. Both original and copied bytes remain artifact-bound.
        if (not source.is_absolute() or not source.is_file() or source.is_symlink()
                or source.resolve() != source or artifact.get("executable") != str(source)
                or sha256_file(source) != artifact.get("sha256")):
            raise QualificationError("compiled artifact changed")
        if not evidence.is_absolute() or not evidence.is_dir() or evidence.is_symlink() or evidence.resolve() != evidence:
            raise QualificationError("native bundle requires exact regular paths")
        directories = (evidence / BUNDLE_NAME, evidence / BUNDLE_NAME / "Contents", evidence / BUNDLE_NAME / "Contents/MacOS")
        executable, info = directories[-1] / BUNDLE_EXECUTABLE, directories[1] / "Info.plist"
        try:
            for directory in directories: directory.mkdir(mode=0o700)
            with source.open("rb") as original, os.fdopen(os.open(executable,os.O_WRONLY | os.O_CREAT | os.O_EXCL,0o700),"wb") as copied:
                shutil.copyfileobj(original,copied,65536)
            with os.fdopen(os.open(info,os.O_WRONLY | os.O_CREAT | os.O_EXCL,0o600),"wb") as output:
                output.write(bundle_info())
        except OSError as error:
            raise QualificationError("disposable native bundle creation failed") from error
        launch = {**artifact,"sourceExecutable":str(source),"executable":str(executable),
            "launchScope":"disposable-app-wrapper","infoPlistSha256":sha256_file(info)}
        verify_native_bundle(executable,launch,evidence)
        write_json_new(evidence / "launch-artifact.json",launch)
        return executable, launch
    except OSError as error:
        raise QualificationError("disposable native bundle creation failed") from error

def validate_event(row,case,nonce,child_pid):
    if not isinstance(row,dict) or set(row)!={"schema","case","nonce","sequence","event","fields"}: raise QualificationError("invalid event envelope")
    if row["schema"]!=SCHEMA or row["case"]!=case or row["nonce"]!=nonce or not is_int(row["sequence"]): raise QualificationError("event identity mismatch")
    event,fields=row["event"],row["fields"]
    if not isinstance(event,str): raise QualificationError("invalid event kind")
    if event=="ready":
        if not isinstance(fields,dict) or set(fields)!={"parentLabel","pid","context"} or fields["parentLabel"]!="c1-native-parent" or not is_int(fields["pid"]) or fields["pid"]!=child_pid or fields["context"]!=case_context(case,nonce): raise QualificationError("invalid ready event")
    elif event=="phase":
        if not isinstance(fields,dict) or set(fields)!={"phase"} or not isinstance(fields["phase"],str) or fields["phase"] not in {"native_review_queued","native_review_accepted","native_input_queued","native_input_presented","native_input_accepted","read_returned"}: raise QualificationError("invalid phase event")
    elif event=="command_received":
        if not isinstance(fields,dict) or set(fields)!={"command"} or not isinstance(fields["command"],str) or fields["command"] not in COMMANDS: raise QualificationError("invalid command event")
    elif event in {"main_held","main_released"}:
        if not exact_fields(fields,{}): raise QualificationError("invalid main event")
    elif event=="stopping":
        if not exact_fields(fields,{"status":"closing","admitted":True}): raise QualificationError("invalid stopping event")
    elif event=="result":
        counts=fields.get("rpcCounts") if isinstance(fields,dict) else None
        if not isinstance(fields,dict) or set(fields)!={"status","code","observation","rpcCounts","authority","execution","outcome"} or fields.get("status") not in ("completed","cancelled","error") or fields.get("code") not in (None,"cancelled","credential_lease_expired") or type(fields.get("observation")) is not bool or not isinstance(counts,dict) or set(counts)!={"health","policyStatus","logs"} or not all(is_int(value) and value>=0 for value in counts.values()) or fields.get("authority")!="none" or fields.get("execution")!="unavailable" or fields.get("outcome")!="unknown": raise QualificationError("invalid result event")
    elif event=="exit":
        if not exact_fields(fields,{"code":0,"clean":True}): raise QualificationError("invalid exit event")
    else: raise QualificationError("unknown event")

def event_index(rows,event):
    values=[number for number,row in enumerate(rows) if row["event"]==event]
    return values[0] if len(values)==1 else None

def phase_index(rows, phase):
    values=[number for number,row in enumerate(rows) if row["event"]=="phase" and row["fields"]["phase"]==phase]
    return values[0] if len(values)==1 else None

def command_indexes(rows, commands):
    values=[(number,row["fields"]["command"]) for number,row in enumerate(rows) if row["event"]=="command_received"]
    if [command for _,command in values] != list(commands): raise QualificationError("control command evidence invalid")
    return [number for number,_ in values]

def verify_events(case,nonce,child_pid,rows,elapsed=None):
    if not 1<=len(rows)<=EVENT_LIMIT: raise QualificationError("event count out of bounds")
    terminal=False
    for sequence,row in enumerate(rows,1):
        validate_event(row,case,nonce,child_pid)
        if row["sequence"]!=sequence: raise QualificationError("noncontiguous event sequence")
        if terminal and row["event"] != "exit": raise QualificationError("event follows terminal result")
        if row["event"]=="result": terminal=True
    if rows[0]["event"]!="ready" or rows[-1]["event"]!="exit" or event_index(rows,"ready")!=0: raise QualificationError("missing clean event bounds")
    results,exits=[row for row in rows if row["event"]=="result"],[row for row in rows if row["event"]=="exit"]
    if len(results)!=1 or len(exits)!=1 or rows[-2]["event"]!="result": raise QualificationError("terminal records invalid")
    phases=[row["fields"]["phase"] for row in rows if row["event"]=="phase"]; ordered=("native_review_queued","native_review_accepted","native_input_queued","native_input_presented","native_input_accepted","read_returned")
    if phases!=list(ordered[:len(phases)]): raise QualificationError("phase order invalid")
    fields=results[0]["fields"]
    expected={"status":"completed","code":None,"observation":True,"rpcCounts":{"health":1,"policyStatus":1,"logs":1}} if case=="accept-read" else {"status":"error","code":"credential_lease_expired","observation":False,"rpcCounts":{"health":0,"policyStatus":0,"logs":0}} if case=="expire-open" else {"status":"cancelled","code":"cancelled","observation":False,"rpcCounts":{"health":0,"policyStatus":0,"logs":0}}
    if any(not strictly_equal(fields[key],value) for key,value in expected.items()): raise QualificationError("case result invalid")
    if case=="accept-read" and phases!=list(ordered): raise QualificationError("accept-read phase evidence incomplete")
    if case=="cancel-review" and phases!=["native_review_queued"]: raise QualificationError("cancel-review phases invalid")
    if case=="stop-queued" and phases!=["native_review_queued"]: raise QualificationError("stop-queued phases invalid")
    if case in {"stop-open","parent-close","expire-open"} and phases!=list(ordered[:4]): raise QualificationError("native input phase evidence incomplete")
    if case=="stop-queued":
        held,stopping,released=event_index(rows,"main_held"),event_index(rows,"stopping"),event_index(rows,"main_released")
        queued=phase_index(rows,"native_review_queued"); result=event_index(rows,"result")
        stop,released_command=command_indexes(rows,("stop","release-main"))
        if any(value is None for value in (held,queued,stopping,released,result)) or not held<queued<stop<stopping<released_command<released<result: raise QualificationError("stop custody evidence invalid")
    if case=="stop-open":
        held,stopping,released=event_index(rows,"main_held"),event_index(rows,"stopping"),event_index(rows,"main_released")
        input_presented,result=phase_index(rows,"native_input_presented"),event_index(rows,"result")
        hold,stop,released_command=command_indexes(rows,("hold-main","stop","release-main"))
        if any(value is None for value in (held,stopping,released,input_presented,result)) or not input_presented<hold<held<stop<stopping<released_command<released<result: raise QualificationError("stop custody evidence invalid")
    if case=="parent-close":
        presented,close=phase_index(rows,"native_input_presented"),command_indexes(rows,("close-parent",))[0]
        if presented is None or not presented<close: raise QualificationError("parent close command missing")
    if case=="expire-open" and (elapsed is None or elapsed<119): raise QualificationError("expiry evidence invalid")
    return rows

def case_context(case,nonce):
    return f"c1-{case}-{nonce}"

def validate_controller_record(record):
    if not isinstance(record,dict) or not isinstance(record.get("case"),str) or record["case"] not in CASES or not isinstance(record.get("nonce"),str) or not re.fullmatch(r"[0-9a-f]{32}",record["nonce"]): raise QualificationError("invalid controller record")
    if set(record)=={"case","nonce","command"}:
        if not isinstance(record["command"],str) or record["command"] not in COMMANDS: raise QualificationError("invalid controller record")
        return
    kind=record.get("kind")
    base={"case","nonce","kind"}
    if kind=="start" and set(record)==base|{"challenge"}:
        valid=isinstance(record["challenge"],str) and re.fullmatch(r"[0-9a-f]{32}",record["challenge"])
    elif kind=="intent" and set(record)==base|{"id","action","pid","artifactSha256","context"}:
        valid=isinstance(record["id"],str) and re.fullmatch(r"[0-9a-f]{32}",record["id"]) and isinstance(record["action"],str) and record["action"] in {"review-continue","review-cancel","input-fill","input-submit"} and is_int(record["pid"]) and record["pid"]>0 and isinstance(record["artifactSha256"],str) and re.fullmatch(r"[0-9a-f]{64}",record["artifactSha256"]) and record["context"]==case_context(record["case"],record["nonce"])
    elif kind=="return" and set(record)==base|{"id","outcome"}:
        valid=isinstance(record["id"],str) and re.fullmatch(r"[0-9a-f]{32}",record["id"]) and isinstance(record["outcome"],str) and record["outcome"] in {"dispatched","failed","interference"}
    elif kind=="handoff" and set(record)==base|{"pid","artifactSha256","terminalId"}:
        valid=is_int(record["pid"]) and record["pid"]>0 and isinstance(record["artifactSha256"],str) and re.fullmatch(r"[0-9a-f]{64}",record["artifactSha256"]) and isinstance(record["terminalId"],str) and re.fullmatch(r"[0-9a-f]{32}",record["terminalId"])
    else: valid=False
    if not valid: raise QualificationError("invalid controller record")

class CommandPump:
    def __init__(self,stream): self.stream,self.commands,self.failure,self.eof=stream,queue.Queue(maxsize=8),None,False; self.thread=threading.Thread(target=self._read,daemon=True); self.thread.start()
    def _read(self):
        while True:
            line=self.stream.readline(1025)
            if not line: self.eof=True; return
            if len(line)>1024 or not line.endswith("\n"): self.failure="invalid command input"; return
            try:
                record=json.loads(line,object_pairs_hook=no_duplicate_object); validate_controller_record(record)
            except (json.JSONDecodeError,DuplicateKeyError,QualificationError): self.failure="invalid command input"; return
            try: self.commands.put(record,timeout=1)
            except queue.Full: self.failure="command queue overflow"; return
    def discard(self):
        while True:
            try: self.commands.get_nowait()
            except queue.Empty: return

class CaseController:
    """Private test correlation only; cannot supply native consent or input.

    The lock serializes native observations, controller assertions and durable
    journal writes. Only fixed action names and already-validated metadata enter
    the journal. No field value, raw UI dump or free-form failure is accepted.
    """
    def __init__(self,case,nonce,artifact,evidence,pump):
        if pump is None: raise QualificationError("controller required before launch")
        self.case,self.nonce,self.sha,self.evidence,self.pump=case,nonce,artifact["sha256"],evidence,pump
        self.lock=threading.RLock(); self.pid=None; self.phases=[]; self.actions={}; self.pending=None
        self.controls=[]; self.control_seen=0; self.main_held=False; self.stopping=False
        self.result=False; self.exited=False; self.failed=False; self.records=0; self.native_count=0
        self.path=evidence/f"{case}.controller.jsonl"
        self.output=os.fdopen(os.open(self.path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),"w",encoding="utf-8")

    def close(self):
        with self.lock: self.output.close()

    def append(self,kind,**fields):
        # Caller holds the single state/journal lock.
        if self.records>=256: raise QualificationError("controller journal bound")
        row={"sequence":self.records+1,"kind":kind,"case":self.case,"nonce":self.nonce,**fields}
        try:
            self.output.write(json.dumps(row,sort_keys=True,separators=(",",":"))+"\n"); self.output.flush(); os.fsync(self.output.fileno()); self.records+=1
        except OSError as error: raise QualificationError("controller journal write failed") from error

    def identity(self,record):
        validate_controller_record(record)
        if record["case"]!=self.case or record["nonce"]!=self.nonce: raise QualificationError("stale controller identity")

    def next_record(self,timeout):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            if self.pump.failure: raise QualificationError("controller input failed")
            try: return self.pump.commands.get(timeout=.05)
            except queue.Empty:
                if self.pump.eof: raise QualificationError("controller input closed")
        raise QualificationError("controller response timed out")

    def start(self):
        challenge=secrets.token_hex(16)
        with self.lock:
            self.append("start-request",challenge=challenge,artifactSha256=self.sha,context=case_context(self.case,self.nonce))
            write_json_new(self.evidence/f"{self.case}.start.json",{"case":self.case,"nonce":self.nonce,"kind":"start","challenge":challenge,"artifactSha256":self.sha,"context":case_context(self.case,self.nonce)})
        record=self.next_record(60); self.identity(record)
        if record!={"case":self.case,"nonce":self.nonce,"kind":"start","challenge":challenge}: raise QualificationError("start challenge mismatch")
        with self.lock: self.append("start-accepted")

    def attach(self,pid):
        with self.lock:
            if self.pid is not None: raise QualificationError("controller child already bound")
            self.pid=pid; self.append("case-start",pid=pid,artifactSha256=self.sha,context=case_context(self.case,self.nonce))

    def receive(self,record):
        with self.lock:
            self.identity(record)
            if self.failed or self.exited and record.get("kind")!="return": raise QualificationError("controller action after terminal")
            kind=record.get("kind")
            if kind=="return":
                if self.pending is None or self.actions[self.pending]["id"]!=record["id"]: raise QualificationError("controller return mismatch")
                self.append("action-return",id=record["id"],outcome=record["outcome"])
                if record["outcome"]!="dispatched": raise QualificationError("controller action refused")
                self.actions[self.pending]["returned"]=True; self.pending=None; return None
            if self.result or self.pending is not None: raise QualificationError("controller action out of order")
            if kind=="intent":
                action=record["action"]
                if record["pid"]!=self.pid or record["artifactSha256"]!=self.sha or action in self.actions or any(value["id"]==record["id"] for value in self.actions.values()): raise QualificationError("controller action binding mismatch")
                phase=self.phases[-1] if self.phases else None
                valid=(action=="review-continue" and self.case not in {"cancel-review","stop-queued"} and phase=="native_review_queued") or (action=="review-cancel" and self.case=="cancel-review" and phase=="native_review_queued") or (action=="input-fill" and phase=="native_input_presented" and "review-continue" in self.actions) or (action=="input-submit" and self.case=="accept-read" and phase=="native_input_presented" and "input-fill" in self.actions)
                if not valid: raise QualificationError("controller action phase mismatch")
                self.append("action-intent",id=record["id"],action=action,pid=self.pid,artifactSha256=self.sha,context=record["context"])
                self.actions[action]={"id":record["id"],"returned":False}; self.pending=action; return None
            if "command" in record:
                expected={"stop-queued":("stop","release-main"),"stop-open":("hold-main","stop","release-main"),"parent-close":("close-parent",)}.get(self.case,())
                command=record["command"]
                if len(self.controls)>=len(expected) or command!=expected[len(self.controls)]: raise QualificationError("controller control out of order")
                if command in {"hold-main","close-parent"} and (not self.phases or self.phases[-1]!="native_input_presented" or not self.actions.get("input-fill",{}).get("returned")): raise QualificationError("controller control before input")
                if command=="stop" and not self.main_held or command=="release-main" and not self.stopping: raise QualificationError("controller control before custody")
                self.append("control-intent",command=command); self.controls.append(command); return command
            raise QualificationError("unexpected controller record")

    def native(self,row):
        with self.lock:
            if self.failed or self.exited: raise QualificationError("native event after terminal")
            validate_event(row,self.case,self.nonce,self.pid)
            if row["sequence"]!=self.native_count+1 or self.native_count==0 and row["event"]!="ready" or self.native_count>0 and row["event"]=="ready": raise QualificationError("native sequence mismatch")
            event,fields=row["event"],row["fields"]
            if self.result and event!="exit": raise QualificationError("native event after result")
            if event=="phase":
                phase=fields["phase"]; ordered=("native_review_queued","native_review_accepted","native_input_queued","native_input_presented","native_input_accepted","read_returned")
                if len(self.phases)>=len(ordered) or phase!=ordered[len(self.phases)]: raise QualificationError("native phase out of order")
                if phase in ordered[1:4] and "review-continue" not in self.actions or phase in ordered[4:] and "input-submit" not in self.actions: raise QualificationError("unattributed native transition")
                self.phases.append(phase)
            elif event=="command_received":
                if self.control_seen>=len(self.controls) or fields["command"]!=self.controls[self.control_seen]: raise QualificationError("unattributed native control")
                self.control_seen+=1
            elif event=="main_held":
                if self.main_held or self.case!="stop-queued" and "hold-main" not in self.controls: raise QualificationError("unattributed native hold")
                self.main_held=True
            elif event=="main_released":
                if not self.main_held or "release-main" not in self.controls: raise QualificationError("unattributed native release")
                self.main_held=False
            elif event=="stopping":
                if self.stopping or "stop" not in self.controls: raise QualificationError("unattributed native stop")
                self.stopping=True
            elif event=="result":
                needed={"accept-read":"input-submit","cancel-review":"review-cancel"}.get(self.case,"review-continue")
                if self.case!="stop-queued" and needed not in self.actions: raise QualificationError("unattributed native result")
                if self.case in {"stop-queued","stop-open"} and (not self.stopping or self.main_held) or self.case=="parent-close" and "close-parent" not in self.controls: raise QualificationError("unattributed native result")
                self.result=True
            elif event=="exit":
                if not self.result: raise QualificationError("native exit before result")
                self.exited=True
            self.append("native",event=row); self.native_count+=1

    def refuse(self):
        with self.lock:
            if not self.failed:
                self.failed=True; self.append("refused",code="controller-correlation-failed")

    def settle_returns(self,timeout=60):
        # The child has exited. This only bounds controller transport latency;
        # it cannot extend the native credential deadline or admit a new action.
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            with self.lock:
                if self.failed: raise QualificationError("controller correlation failed")
                if self.pending is None: return
            time.sleep(.01)
        raise QualificationError("controller return timed out")

    def handoff(self):
        with self.lock:
            if self.failed or not self.result or not self.exited or self.pending is not None: raise QualificationError("unclean controller terminal")
            terminal=secrets.token_hex(16)
            expected={"case":self.case,"nonce":self.nonce,"kind":"handoff","pid":self.pid,"artifactSha256":self.sha,"terminalId":terminal}
            self.append("terminal-request",pid=self.pid,artifactSha256=self.sha,terminalId=terminal)
            write_json_new(self.evidence/f"{self.case}.terminal.json",expected)
        record=self.next_record(60); self.identity(record)
        if record!=expected: raise QualificationError("terminal handoff mismatch")
        with self.lock:
            self.append("handoff-accepted",pid=self.pid,artifactSha256=self.sha,terminalId=terminal)

def capture_child(executable,artifact,case,nonce,timeout,evidence,pump=None,controller=None):
    child=subprocess.Popen((str(executable),"--case",case,"--nonce",nonce),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=executable.parent); started=time.monotonic()
    rows,stderr=[],bytearray(); flags={"stdoutOverflow":False,"stderrOverflow":False,"stdoutEof":False,"stderrEof":False,"cleanupFailure":False,"timedOut":False,"eventFailure":False,"commandFailure":False,"attributionFailure":False,"ioFailure":False}; event_path=evidence/f"{case}.events.jsonl"
    kill_lock=threading.Lock(); kill_requested=False
    def stop_owned_child():
        nonlocal kill_requested
        with kill_lock:
            if kill_requested: return
            kill_requested=True
            try: child.kill()  # Only this owned disposable child; never a passing outcome.
            except OSError: flags["cleanupFailure"]=True
    def refuse_attribution():
        flags["attributionFailure"]=True
        try:
            if controller: controller.refuse()
        except QualificationError: pass  # Failed journal durability also refuses capture.
        finally: stop_owned_child()

    try:
        write_json_new(evidence/f"{case}.launch.json",{"case":case,"nonce":nonce,"pid":child.pid,"artifactSha256":artifact["sha256"],"timestamp":datetime.now(timezone.utc).isoformat()})
        if controller: controller.attach(child.pid)
    except (OSError,QualificationError) as error:
        flags["ioFailure"]=isinstance(error,OSError); refuse_attribution()
        try: child.wait(timeout=2)
        except (OSError,subprocess.TimeoutExpired): flags["cleanupFailure"]=True
        for pipe in (child.stdin,child.stdout,child.stderr):
            try: pipe.close()
            except OSError: flags["cleanupFailure"]=True
        capture={"pid":child.pid,"code":child.returncode,"elapsed":time.monotonic()-started,"stderrPresent":False,**flags}
        try: failure_receipt(evidence,case,nonce,capture,"owned-child-setup-failed")
        except OSError: pass  # Unwritable evidence is not a successful cleanup receipt.
        raise QualificationError("owned child setup failed") from error
    def stdout_reader():
        total=0
        with event_path.open("x",encoding="utf-8") as safe:
            while True:
                line=child.stdout.readline(LINE_LIMIT+1)
                if not line: flags["stdoutEof"]=True; return
                total+=len(line)
                if total>OUTPUT_LIMIT or len(line)>LINE_LIMIT or not line.endswith(b"\n"):
                    flags["stdoutOverflow"]=True; refuse_attribution(); return
                try:
                    row=json.loads(line.decode("utf-8"),object_pairs_hook=no_duplicate_object); validate_event(row,case,nonce,child.pid)
                    if row["sequence"]!=len(rows)+1: raise QualificationError("noncontiguous event sequence")
                    rows.append(row); safe.write(json.dumps(row,sort_keys=True,separators=(",",":"))+"\n"); safe.flush()
                    if controller: controller.native(row)
                except (UnicodeDecodeError,json.JSONDecodeError,DuplicateKeyError,QualificationError):
                    flags["eventFailure"]=True
                    refuse_attribution()
                    return
    def stderr_reader():
        while True:
            part=child.stderr.read(4096)
            if not part: flags["stderrEof"]=True; return
            if len(stderr)+len(part)>STDERR_LIMIT:
                flags["stderrOverflow"]=True; refuse_attribution(); return
            stderr.extend(part)
    done=threading.Event()
    def relay():
        while not done.is_set():
            if pump and pump.failure:
                flags["commandFailure"]=True
                refuse_attribution()
                return
            try: command=pump.commands.get(timeout=.05) if pump else None
            except queue.Empty: continue
            if command:
                try:
                    value=controller.receive(command) if controller else command.get("command") if command["case"]==case and command["nonce"]==nonce else None
                    if value and child.poll() is None: child.stdin.write((value+"\n").encode()); child.stdin.flush()
                except QualificationError:
                    flags["commandFailure"]=True; refuse_attribution(); return
                except (BrokenPipeError,OSError):
                    if controller: refuse_attribution()
                    return
    def guarded_reader(read):
        try: read()
        except OSError:
            flags["ioFailure"]=True; refuse_attribution()
    readers=[threading.Thread(target=guarded_reader,args=(read,),daemon=True) for read in (stdout_reader,stderr_reader)]
    for thread in readers: thread.start()
    relay_thread=threading.Thread(target=relay,daemon=True); relay_thread.start()
    try: child.wait(timeout=timeout)
    except (OSError,subprocess.TimeoutExpired) as error:
        flags["timedOut"]=isinstance(error,subprocess.TimeoutExpired)
        flags["ioFailure"]=isinstance(error,OSError)
        try: stop_owned_child(); child.wait(timeout=2)
        except (OSError,subprocess.TimeoutExpired): flags["cleanupFailure"]=True
    finally:
        try: child.stdin.close()
        except OSError: flags["cleanupFailure"]=True
    for thread in readers:
        thread.join(timeout=2)
        if thread.is_alive(): flags["cleanupFailure"]=True
    if controller and child.returncode==0 and not any(flags[key] for key in ("timedOut","cleanupFailure","eventFailure","commandFailure","attributionFailure","ioFailure")):
        try: controller.settle_returns()
        except QualificationError: refuse_attribution()
    done.set(); relay_thread.join(timeout=.2)
    if relay_thread.is_alive(): flags["cleanupFailure"]=True
    return {"pid":child.pid,"code":child.returncode,"rows":rows,"stderrPresent":bool(stderr),"elapsed":time.monotonic()-started,**flags}

def failure_receipt(evidence,case,nonce,capture,code):
    path=evidence/f"{case}.failure.json"
    if not path.exists(): write_json_new(path,{"case":case,"nonce":nonce,"pid":capture.get("pid"),"code":code,"childReturnCode":capture.get("code"),"elapsedSeconds":capture.get("elapsed"),"stdoutEof":bool(capture.get("stdoutEof")),"stderrEof":bool(capture.get("stderrEof")),"stderrPresent":bool(capture.get("stderrPresent")),"flags":{key:bool(capture.get(key)) for key in ("timedOut","stdoutOverflow","stderrOverflow","cleanupFailure","eventFailure","commandFailure","attributionFailure","ioFailure")}})

def execute_case(repo,executable,artifact,manifest,evidence,case,timeout,pump=None):
    verify_native_bundle(executable,artifact,evidence)
    nonce=secrets.token_hex(16)
    controller=CaseController(case,nonce,artifact,evidence,pump)
    capture={}
    try:
        controller.start()
        # The challenge can be held for a minute. Recheck bytes immediately
        # before spawning, not merely before waiting for controller readiness.
        verify_native_bundle(executable,artifact,evidence)
        if source_manifest(repo)!=manifest: raise QualificationError("source changed before launch")
        capture=capture_child(executable,artifact,case,nonce,timeout,evidence,pump,controller)
        if capture["code"]!=0 or any(capture.get(key) for key in ("timedOut","stdoutOverflow","stderrOverflow","cleanupFailure","eventFailure","commandFailure","attributionFailure","ioFailure")) or not capture["stdoutEof"] or not capture["stderrEof"]: raise QualificationError("child capture failed")
        rows=verify_events(case,nonce,capture["pid"],capture["rows"],capture["elapsed"])
        verify_native_bundle(executable,artifact,evidence)
        if source_manifest(repo)!=manifest: raise QualificationError("source changed during qualification")
        controller.handoff()
    except QualificationError:
        failure_receipt(evidence,case,nonce,capture,"qualification-failed"); raise
    finally: controller.close()
    result={"case":case,"nonce":nonce,"pid":capture["pid"],"artifactSha256":artifact["sha256"],"events":len(rows),"elapsedSeconds":capture["elapsed"],"stderrPresent":capture["stderrPresent"],"parentOraclePassed":True,"controllerCorrelationComplete":True,"parentCloseMethod":"programmatic-owned-parent" if case=="parent-close" else None}
    write_json_new(evidence/f"{case}.result.json",result); return result

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--repo",type=Path,required=True); parser.add_argument("--toolchain-bin",type=Path,required=True); parser.add_argument("--evidence",type=Path,required=True); parser.add_argument("--case",choices=CASES); parser.add_argument("--timeout",type=int,default=200)
    args=parser.parse_args(argv)
    if not 1<=args.timeout<=MAX_TIMEOUT: parser.error("timeout must be 1..200 seconds")
    repo,evidence=exact_root(args.repo),args.evidence.resolve()
    if evidence.exists(): parser.error("evidence directory must be fresh")
    evidence.mkdir(mode=0o700,parents=True); manifest,cases=source_manifest(repo),verify_plan(repo/CASE_PLAN); write_json_new(evidence/"source-manifest.json",manifest)
    toolchain=toolchain_record(repo,args.toolchain_bin); write_json_new(evidence/"toolchain.json",{key:value for key,value in toolchain.items() if key!="env"}); executable,artifact=build(repo,evidence,manifest,toolchain)
    executable,artifact=prepare_native_bundle(executable,artifact,evidence)
    pump,selected=CommandPump(sys.stdin),(args.case,) if args.case else cases; results=[execute_case(repo,executable,artifact,manifest,evidence,case,args.timeout,pump) for case in selected]
    verify_native_bundle(executable,artifact,evidence)
    if source_manifest(repo)!=manifest: raise QualificationError("source changed after qualification")
    complete=args.case is None and tuple(row["case"] for row in results)==CASES and all(row["parentOraclePassed"] and row["controllerCorrelationComplete"] for row in results)
    label="diagnostic-subset" if args.case is not None else "full-matrix"
    write_json_new(evidence/"results.json",{"schema":"rangoon.native-credential-results.v2","label":label,"diagnosticSubset":args.case is not None,"parentControllerMatrixComplete":complete,"qualificationComplete":False,"independentUiReview":"required","results":results}); print(json.dumps({"evidence":str(evidence),"cases":[row["case"] for row in results],"label":label,"parentControllerMatrixComplete":complete,"qualificationComplete":False},sort_keys=True))

if __name__=="__main__":
    try: main()
    except QualificationError as error: print(f"native-credential qualification failed: {error}",file=sys.stderr); raise SystemExit(1)
