#!/usr/bin/env python3
"""Generate independent deterministic Rangoon V2 backup archive vectors.

This script uses only Python standard-library framing, JSON and SHA-256. It
does not invoke Rangoon code or a database.
"""

from __future__ import annotations

import base64
import hashlib
import json
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "fixtures" / "composition" / "backup-v2.json"
DECOMPOSE = ROOT / "fixtures" / "composition" / "decompose-v0.json"
APPLICATION = ROOT / "fixtures" / "composition" / "application-v0.json"

MAGIC = b"RANGOON-BACKUP-V2\n"


def compact(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def framed(domain: bytes, *fields: str) -> str:
    body = bytearray(domain + b"\0")
    for field in fields:
        encoded = field.encode("utf-8")
        body.extend(struct.pack(">Q", len(encoded)))
        body.extend(encoded)
    return sha256(bytes(body))


def source_id(display_name: str, content: str) -> str:
    return "source:" + framed(b"rangoon.source.v0", display_name, content)


def composition_id(draft: dict[str, object]) -> tuple[str, bytes]:
    envelope = compact({"transformationVersion": "0.1.0", "draft": draft})
    body = b"rangoon.composition.v0\0" + struct.pack(">Q", len(envelope)) + envelope
    return "composition:" + sha256(body), envelope


def composed_capability_id(recipe_id: str, output_index: int) -> str:
    encoded = recipe_id.encode("utf-8")
    body = b"rangoon.composed-capability.v0\0" + struct.pack(">Q", len(encoded)) + encoded
    body += struct.pack(">I", output_index)
    return "capability:" + sha256(body)


def revision_id(capability_id: str, parent: str | None, title: str, content: str) -> str:
    return "revision:" + framed(
        b"rangoon.revision.v0", capability_id, parent or "", title, content
    )


def application_id(recipe_id: str, targets: list[dict[str, str]]) -> tuple[str, bytes]:
    envelope = compact(
        {
            "schemaVersion": "rangoon.composition-application.v0",
            "compositionId": recipe_id,
            "targets": targets,
        }
    )
    body = (
        b"rangoon.composition-application.v0\0"
        + struct.pack(">Q", len(envelope))
        + envelope
    )
    return "composition-application:" + sha256(body), envelope


def archive(manifest: dict[str, object], payload: list[bytes]) -> tuple[bytes, bytes, str]:
    manifest_bytes = compact(manifest)
    body = MAGIC + struct.pack(">I", len(manifest_bytes)) + manifest_bytes + b"".join(payload)
    trailer = sha256(body).encode("ascii")
    return body + trailer, manifest_bytes, trailer.decode("ascii")


def vector(name: str, manifest: dict[str, object], payload: list[bytes]) -> dict[str, object]:
    archive_bytes, manifest_bytes, trailer = archive(manifest, payload)
    complete_digest = sha256(archive_bytes)
    return {
        "name": name,
        "manifest": manifest_bytes.decode("utf-8"),
        "archiveBase64": base64.b64encode(archive_bytes).decode("ascii"),
        "archiveHex": archive_bytes.hex(),
        "archiveByteLength": len(archive_bytes),
        "trailerSha256": trailer,
        "archiveDigest": complete_digest,
        "archiveId": "backup:" + complete_digest,
        "expectedRecordCounts": {
            "sources": len(manifest["sources"]),
            "owners": len(manifest["owners"]),
            "revisions": len(manifest["revisions"]),
            "recipes": len(manifest["recipes"]),
            "applications": len(manifest["applications"]),
        },
    }


def main() -> None:
    decompose = json.loads(DECOMPOSE.read_text(encoding="utf-8"))
    application = json.loads(APPLICATION.read_text(encoding="utf-8"))
    source_content = decompose["resolvedInputs"][0]["content"]
    display_name = "AGENTS.md"
    draft = {
        "schemaVersion": "rangoon.composition-draft.v0",
        "operation": "decompose",
        "inputs": [
            {
                "kind": "source",
                "sourceId": source_id(display_name, source_content),
                "sha256": sha256(source_content.encode("utf-8")),
            }
        ],
        "outputs": [
            {"title": "Rules", "pieces": [{"kind": "copy", "range": {"inputIndex": 0, "startByte": 0, "endByte": 29}}]},
            {"title": "Tests", "pieces": [{"kind": "copy", "range": {"inputIndex": 0, "startByte": 29, "endByte": 52}}]},
        ],
        "exclusions": [],
        "duplications": [],
        "conflicts": [],
    }
    recipe_id, _ = composition_id(draft)
    targets = [{"kind": "new"}, {"kind": "new"}]
    app_id, _ = application_id(recipe_id, targets)
    rules = source_content.encode("utf-8")[:29].decode("utf-8")
    tests = source_content.encode("utf-8")[29:].decode("utf-8")
    capability = composed_capability_id(recipe_id, 0)
    root_revision = revision_id(capability, None, "Rules", rules)

    # Evidence fixtures are checked, never used as identity inputs.
    assert draft == decompose["draft"]
    assert recipe_id == decompose["expectedCompositionId"] == application["expectedCompositionId"]
    assert app_id == application["cases"][0]["expectedApplicationId"]
    assert capability == application["cases"][0]["expectedAppliedOutputs"][0]["capabilityId"]
    assert root_revision == application["cases"][0]["expectedAppliedOutputs"][0]["revisionId"]
    assert tests == decompose["expectedOutputs"][1]["content"]

    empty_manifest = {
        "schemaVersion": "rangoon.backup.v2",
        "sources": [],
        "owners": [],
        "revisions": [],
        "recipes": [],
        "applications": [],
    }
    draft_bytes = compact(draft)
    revision_summary = {
        "id": root_revision,
        "parentRevisionId": None,
        "title": "Rules",
        "sha256": sha256(rules.encode("utf-8")),
        "createdAtMs": 1,
        "review": None,
        "provenance": {
            "kind": "composition",
            "applicationId": app_id,
            "compositionId": recipe_id,
            "outputIndex": 0,
        },
    }
    partial_manifest = {
        "schemaVersion": "rangoon.backup.v2",
        "sources": [
            {
                "sourceId": source_id(display_name, source_content),
                "displayName": display_name,
                "sha256": sha256(source_content.encode("utf-8")),
                "byteLength": len(source_content.encode("utf-8")),
                "savedAtMs": 1,
            }
        ],
        "owners": [
            {
                "id": capability,
                "birth": {"kind": "composition", "compositionId": recipe_id, "outputIndex": 0},
                "latestRevisionId": root_revision,
            }
        ],
        "revisions": [
            {"capabilityId": capability, "revision": revision_summary, "byteLength": len(rules.encode("utf-8"))}
        ],
        "recipes": [
            {
                "id": recipe_id,
                "transformationVersion": "0.1.0",
                "byteLength": len(draft_bytes),
                "sha256": sha256(draft_bytes),
                "createdAtMs": 1,
            }
        ],
        "applications": [
            {"id": app_id, "compositionId": recipe_id, "targets": targets, "createdAtMs": 1}
        ],
    }
    result = {
        "evidence": "Independently constructed with Python hashlib, struct and compact JSON. No Rangoon Rust code is invoked. Synthetic BOM and CRLF content is checked against the composition vectors.",
        "format": "rangoon.backup.v2",
        "vectors": [
            vector("empty", empty_manifest, []),
            vector("composed_partial_live_output", partial_manifest, [source_content.encode("utf-8"), rules.encode("utf-8"), draft_bytes]),
        ],
    }
    OUTPUT.write_bytes(compact(result) + b"\n")


if __name__ == "__main__":
    main()
