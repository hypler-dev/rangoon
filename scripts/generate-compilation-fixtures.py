#!/usr/bin/env python3
"""Generate compiler wire vectors with independent standard-library framing.

This does not import or execute Rangoon code. Cases prescribe diagnostics and
readiness explicitly; this is a wire-format reference, not another Markdown
analyzer or a provenance authenticator. All content and identities are synthetic.
"""

from __future__ import annotations

import copy
import hashlib
import json
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "fixtures" / "compilation" / "instruction-v1.json"
CAPABILITY_ID = "capability:" + "a" * 64


def compact(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def frame(domain: str, *fields: bytes) -> str:
    value = bytearray(domain.encode("ascii") + b"\0")
    for field in fields:
        value.extend(struct.pack(">Q", len(field)))
        value.extend(field)
    return digest(bytes(value))


def ordinary(content: str, review_time: int | None = 20) -> dict:
    title = "Review instructions"
    revision_id = "revision:" + frame(
        "rangoon.revision.v0",
        CAPABILITY_ID.encode(), b"", title.encode(), content.encode(),
    )
    return {
        "id": revision_id,
        "parentRevisionId": None,
        "title": title,
        "content": content,
        "sha256": digest(content.encode()),
        "createdAtMs": 10,
        "review": None if review_time is None else {
            "reviewer": "local_operator", "reviewedAtMs": review_time,
        },
        "provenance": {"kind": "ordinary"},
    }


def profile_descriptor(profile: str) -> dict:
    artifact, url = {
        "agents_md_v1": (
            "AGENTS.md",
            "https://learn.chatgpt.com/docs/agent-configuration/agents-md",
        ),
        "claude_md_v1": ("CLAUDE.md", "https://code.claude.com/docs/en/memory"),
    }[profile]
    return {
        "id": profile,
        "version": 1,
        "artifactPath": artifact,
        "documentationUrl": url,
        "documentationRetrievedOn": "2026-10-06",
        "runtimeQualification": "untested",
        "targetBudget": "unknown",
        "semanticEquivalence": "unverified",
    }


def case(name: str, revision: dict, profile: str, readiness: str,
         errors: list[tuple[str, str | None]] | None = None,
         requirements: list[str] | None = None) -> dict:
    descriptor = profile_descriptor(profile)
    profile_digest = digest(compact(descriptor))
    selected = {
        "capabilityId": CAPABILITY_ID,
        "revisionId": revision["id"],
        "parentRevisionId": revision["parentRevisionId"],
        "title": revision["title"],
        "sha256": revision["sha256"],
        "provenance": revision["provenance"],
    }
    metadata = {
        "path": descriptor["artifactPath"],
        "sha256": revision["sha256"],
        "byteLength": len(revision["content"].encode()),
    }
    diagnostics = [
        {"code": "instruction_semantics_unverified", "severity": "warning", "detail": None},
        {"code": "target_environment_unqualified", "severity": "warning", "detail": None},
    ] + [{"code": code, "severity": "error", "detail": detail}
         for code, detail in errors or []]
    envelope = {
        "schemaVersion": "rangoon.compilation-identity.v1",
        "profileDigest": profile_digest,
        "selectedRevision": selected,
        "artifact": metadata,
        "diagnostics": diagnostics,
        "authority": "none",
    }
    compilation_id = "compilation:" + frame("rangoon.compilation.v1", compact(envelope))
    candidate = None
    if readiness == "candidate":
        manifest = {
            "schemaVersion": "rangoon.instruction-candidate.v1",
            "compilationId": compilation_id,
            "profileDigest": profile_digest,
            "selectedRevision": selected,
            "artifact": metadata,
            "reviewObservation": revision["review"],
            "diagnostics": diagnostics,
            "authority": "none",
        }
        manifest_digest = digest(compact(manifest))
        candidate = {
            "manifest": manifest,
            "manifestSha256": manifest_digest,
            "candidateId": "candidate:" + frame(
                "rangoon.instruction-candidate.v1",
                compilation_id.encode(), manifest_digest.encode(),
            ),
        }
    expected = {
        "schemaVersion": "rangoon.compilation.v1",
        "compilationId": compilation_id,
        "profile": descriptor,
        "profileDigest": profile_digest,
        "selectedRevision": selected,
        "artifact": {
            "path": metadata["path"],
            "content": revision["content"],
            "sha256": metadata["sha256"],
            "byteLength": metadata["byteLength"],
        },
        "diagnostics": diagnostics,
        "reviewObservation": revision["review"],
        "readiness": readiness,
        "candidate": candidate,
        "authority": "none",
    }
    return {
        "name": name,
        "capabilityId": CAPABILITY_ID,
        "revision": revision,
        "profile": profile,
        "requirements": requirements or [],
        "expected": expected,
        "expectedCanonicalJson": compact(expected).decode(),
    }


def main() -> None:
    content = "\ufeff# Review\r\nKeep originals 🦀.\r\nNo final newline"
    active_syntax = "# Review\nContact team@example.test.\n<!-- Keep this constraint. -->\n"
    composed = copy.deepcopy(ordinary("# Composed\nKeep both constraints.\n"))
    composed["id"] = "revision:" + "d" * 64
    composed["provenance"] = {
        "kind": "composition",
        "applicationId": "composition-application:" + "b" * 64,
        "compositionId": "composition:" + "c" * 64,
        "outputIndex": 15,
    }
    cases = [
        case("agents_exact_bom_crlf_unicode", ordinary(content), "agents_md_v1", "candidate"),
        case("claude_exact_bom_crlf_unicode", ordinary(content), "claude_md_v1", "candidate"),
        case("unreviewed_same_compilation", ordinary(content, None), "agents_md_v1", "review_required"),
        case("different_review_same_compilation", ordinary(content, 30), "agents_md_v1", "candidate"),
        case("clock_rollback_is_not_identity", ordinary(content, 5), "agents_md_v1", "candidate"),
        case("claude_conservative_blocks", ordinary(active_syntax), "claude_md_v1", "blocked", [
            ("unsupported_requirement", "tool_permissions"),
            ("unsupported_requirement", "future_requirement"),
            ("possible_include_syntax", None),
            ("possible_comment_elision", None),
        ], ["tool_permissions", "future_requirement"]),
        case("blocked_wins_over_missing_review", ordinary(active_syntax, None), "claude_md_v1", "blocked", [
            ("possible_include_syntax", None), ("possible_comment_elision", None),
        ]),
        case("agents_preserves_marker_bytes", ordinary(active_syntax), "agents_md_v1", "candidate"),
        case("synthetic_composition_reference_shape_only", composed, "agents_md_v1", "candidate"),
        case("json_escaping", ordinary('# Quote\n"path\\name"\t\u0001\n'), "agents_md_v1", "candidate"),
    ]
    fixture = {"schemaVersion": "rangoon.compilation-vectors.v1", "cases": cases}
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(fixture, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Generated {len(cases)} independent compiler vectors: {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
