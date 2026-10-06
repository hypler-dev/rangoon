# Native composition and versioned workspace integration

Status: locally implemented and independently reviewed; exact-commit CI/publication, complete composition editor and native GUI qualification remain pending
Authority: [accepted intent](intent.md), [composition specification](composition.md), and [recovery contract](composition-recovery.md)

This stage connects the validated store to the native desktop boundary. It also updates the existing Skills and Workspace interfaces together so composed records can be opened, edited, reviewed, backed up, restored and logically deleted without fabricating single-source provenance. It does not complete the Decompose, Merge or Split editor.

## Commands and byte limits

Only the bundled local main window receives the two new commands. Neither command accepts paths, SQL, original input bytes, keys, engine requests or a renderer-computed preview result. The existing native busy guard serializes them with source selection, source saving and other local mutations.

Both commands use a raw Tauri IPC body containing UTF-8 JSON. The renderer must encode JSON with `TextEncoder` and invoke with the resulting `Uint8Array`; ordinary JSON-object IPC is rejected. The host checks raw byte length before composition or confirmation deserialization. Tauri has already allocated the transport buffer at this point: this is a domain parsing bound, not a transport-level allocation guarantee.

| Command | Request | Bound | Success |
| --- | --- | --- | --- |
| `preview_composition` | Closed `rangoon.composition-application-request.v0` from the destination contract, including exact saved input references and destinations | 8 MiB + 16 KiB UTF-8 JSON | `outcome: ready`, `schemaVersion: rangoon.composition-session.v0`, `previewId`, `expectedStateId`, `preview` |
| `commit_composition` | Closed `rangoon.composition-confirmation.v0` with exactly `schemaVersion`, `previewId`, `expectedStateId`, and boolean `acknowledged` | 4,096 bytes UTF-8 JSON | `outcome: committed`, `receipt` |

`preview` is the store's exact `rangoon.composition-application-preview.v0`, including coverage, diagnostics, materialized output text and destination identities. An incomplete preview can be returned for editing, but its `saveable: false` prevents persistence. `receipt` is `rangoon.composition-receipt.v0`, with the actual output-order capability details, composition/application IDs, committed state ID, and `authority: none`. Unknown fields, duplicate fields, unsupported schemas, malformed UTF-8 and unsupported body types are rejected with fixed public errors.

## Retained preview lifecycle

The native session retains at most one prepared store preview, including its private normalized request and exact application result. Each successful preview receives a fresh `preview:` handle containing 32 bytes from the operating system's random source, encoded as 64 lowercase hexadecimal characters. The direct dependency is pinned to `getrandom` 0.3.4, already present in the desktop dependency graph. Its [documented fill API](https://docs.rs/getrandom/0.3.4/getrandom/fn.fill.html) returns an error on random-source failure; no timestamp, counter or predictable fallback is used.

A successful new preview replaces the prior slot. Parsing, validation, workspace or randomness failure does not issue a handle and leaves any earlier slot intact. The renderer must still invalidate its displayed acknowledgment when editing or requesting a replacement; an older preview is never acknowledgment of the edited draft. Handles are transient selections, not authorization credentials, and are not persisted across process restarts.

Confirmation requires `acknowledged: true` and exact equality with the retained handle and state ID. The host atomically marks the exact handle attempted before entering storage, making that confirmation unusable for any second attempt. It passes the retained Rust preview to the store, which rechecks current state, saved inputs, targets and output identity within the write transaction. After a verified commit, the host consumes the matching slot. Missing, replaced, consumed and previous-process selections are rejected. A stale workspace or failed write keeps the prepared data in its attempted slot but rejects further confirmation with that handle. A newly issued preview and acknowledgment are required even when storage appears unchanged. The UI must refresh saved data and request a new preview after an uncertain result; no automatic mutation retry is permitted.

## Existing surfaces move together

- `list_capabilities`, `open_capability`, `create_capability`, `revise_capability` and `review_capability` retain their names and outcome envelopes but return v1 capability DTOs. Source and composition birth origins are distinct. Every revision and history entry includes ordinary or composition provenance.
- `save_analysis` uses the versioned save path, preserving source saving after schema 3 migration. It still accepts only the selected source ID and saves the host's retained analysis. Snapshot listing/opening continue using validated source records.
- Workspace inventory returns `rangoon.workspace-data.v1`: `records` holds source, capability, revision, review, recipe, application and derivation counts; allocation and reusable-byte counts are top-level fields.
- Export uses V2 composition backups. Native restore accepts validated V1 or V2 archives and retains both the selected archive and exact v1 restore plan. Confirmation supplies only backup ID and expected state; it cannot replace the plan's counts or dependency closure.
- Native deletion retains the issued v1 deletion plan. Confirmation must match kind, ID and state; the store validates the whole plan again before removal. Success consumes the slot. A read-only dependency inspection uses the same issued-plan mechanism.
- The Skills renderer validates closed v1 origins and revision provenance, distinguishes stable birth origin from the selected revision's transformation, and does not invent original source text for composed skills. Import coverage counts only actual source-section origins.
- The Workspace renderer validates v1 counts and origins and shows recipe, application and derivation impact in restore/delete previews. Dependent composed skills can block deletion just like other saved dependencies.

These records remain unencrypted and unauthenticated. Random preview handles, content digests, local review labels and transformation acknowledgment grant no execution or human identity authority. No LNSAT integration, encryption, provider access or external operation is activated.

## Evidence and remaining work

Native tests exercise bounded raw decoding, unknown/duplicate/missing confirmation fields, missing/replaced/unacknowledged handles, random-source failure, stale workspace rejection, consumption after commit and schema 3 editing/review/source-save/backup/restore/deletion. Separate session tests bind restore and deletion to exact retained plans. Renderer tests cover closed v1 contracts, malformed provenance, history consistency and existing confirmation/draft behavior. Named command results and publication state belong in the [development ledger](development.md).

The complete composition editor, all three native GUI workflows, raw IPC round-trip qualification on macOS/Windows/Linux, responsive rendered evidence, restart/reopen and installer/release gates remain outstanding. Unit tests of the host command helpers do not establish those GUI or release properties.
