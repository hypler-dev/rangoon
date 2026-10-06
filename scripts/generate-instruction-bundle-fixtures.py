#!/usr/bin/env python3
"""Independent portable instruction-bundle framing and identity vectors.

Uses the independently generated compiler vectors; never calls Rangoon Rust.
The maximum-content case stores compact input and expected digests, not a second
large copy of instruction text. All inputs are synthetic and unauthenticated.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "fixtures/compilation/instruction-v1.json"
OUTPUT = ROOT / "fixtures/compilation/instruction-bundle-v1.json"
MAGIC = b"RANGOON-INSTRUCTIONS-V1\n"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def compact(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def vector(name: str, report: dict, include_hex: bool = True) -> dict:
    assert report["readiness"] == "candidate"
    manifest = compact(report["candidate"]["manifest"])
    artifact = report["artifact"]["content"].encode("utf-8")
    prefix = MAGIC + struct.pack(">QQ", len(manifest), len(artifact)) + manifest + artifact
    bundle = prefix + digest(prefix).encode("ascii")
    identity = b"rangoon.instruction-bundle.v1\0" + struct.pack(">Q", len(bundle)) + bundle
    result = {
        "name": name,
        "manifestByteLength": len(manifest),
        "artifactByteLength": len(artifact),
        "byteLength": len(bundle),
        "sha256": digest(bundle),
        "bundleId": "instruction-bundle:" + digest(identity),
        "candidateId": report["candidate"]["candidateId"],
        "manifestSha256": report["candidate"]["manifestSha256"],
        "compilationId": report["compilationId"],
    }
    if include_hex:
        result["bundleHex"] = bundle.hex()
    return result


def main() -> None:
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    cases = [vector(case["name"], case["expected"])
             for case in source["cases"] if case["expected"]["readiness"] == "candidate"]
    spec = importlib.util.spec_from_file_location(
        "independent_compiler_vectors", ROOT / "scripts/generate-compilation-fixtures.py")
    assert spec is not None and spec.loader is not None
    sys.dont_write_bytecode = True
    compiler = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(compiler)
    maximum = compiler.case("maximum_content", compiler.ordinary("x" * 262144),
                            "agents_md_v1", "candidate")
    boundary = vector(maximum["name"], maximum["expected"], include_hex=False)
    boundary["input"] = {
        "capabilityId": compiler.CAPABILITY_ID,
        "title": maximum["revision"]["title"],
        "repeat": "x",
        "repeatCount": 262144,
        "createdAtMs": 10,
        "reviewedAtMs": 20,
        "profile": "agents_md_v1",
    }
    output = {
        "schemaVersion": "rangoon.instruction-bundle-vectors.v1",
        "compilerVectorsSha256": digest(SOURCE.read_bytes()),
        "cases": cases,
        "maximumContent": boundary,
    }
    OUTPUT.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Generated {len(cases)} bundle vectors and one maximum-content vector: {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
