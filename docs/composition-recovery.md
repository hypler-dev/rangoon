# Composition recovery format and transactions

Status: implementation refinement under the reviewed [destination contract](composition-destinations.md); qualification pending
Authority: [accepted version 1.0 intent](intent.md)
Owner: Jeff; primary controller owns storage, schema, integration and qualification

## Purpose

Preserve composed skill histories, application destinations and live provenance through backup, additive restore and explicit deletion. Recovery does not recreate a deleted output merely because an immutable application still names it. Source and ordinary revision identities remain unchanged. This format is plaintext with unkeyed consistency hashes; it does not authenticate authorship, protect confidentiality or establish execution authority.

## Version 2 archive

The exact byte sequence is ASCII `RANGOON-BACKUP-V2` followed by LF; unsigned 32-bit big-endian manifest byte length; compact canonical UTF-8 JSON manifest; raw payload segments; and a 64-byte lowercase ASCII SHA-256 trailer. The trailer hashes every preceding byte, including magic and length. No BOM, separator, padding or trailing bytes are accepted. The archive identity is `backup:` plus SHA-256 of the complete archive, including its trailer.

The manifest is at most 2 MiB, the complete archive at most 68 MiB. Before copying payload or creating an in-memory database, decoding checks descriptor counts, strictly increasing descriptor keys, individual lengths, checked aggregate length and exact body-end equality. Sources and revisions are at most 256 KiB each; recipes are at most 8 MiB each. Aggregate raw payload cannot exceed 64 MiB. Complete reconstructed SQLite allocation, including indexes and pages, must fit 64 MiB at the canonical 4 KiB page size. An archive can fit the raw-byte limit and still fail the database allocation limit.

Object fields appear in the following order. Every object is closed, required nullable fields remain explicit, and the stored manifest bytes must equal re-encoding with these field orders and Serde JSON string escaping. Descriptor lists sort lexicographically by their exact identity string, without duplicates. Application target order and recipe input/output/piece order retain their semantic order.

| Object | Fields in order | Descriptor ordering |
| --- | --- | --- |
| Manifest | `schemaVersion` exactly `rangoon.backup.v2`, `sources`, `owners`, `revisions`, `recipes`, `applications` | Not applicable |
| Source | `sourceId`, `displayName`, `sha256`, `byteLength`, `savedAtMs` | `sourceId` |
| Owner | `id`, `birth`, `latestRevisionId` | `id` |
| Source birth | `kind` exactly `source`, `sourceId`, `fragmentId` | Not applicable |
| Composition birth | `kind` exactly `composition`, `compositionId`, `outputIndex` | Not applicable |
| Revision descriptor | `capabilityId`, `revision`, `byteLength` | Nested `revision.id` |
| Revision summary | `id`, `parentRevisionId`, `title`, `sha256`, `createdAtMs`, `review`, `provenance` | Not applicable |
| Review | `reviewer` exactly `local_operator`, `reviewedAtMs` | Embedded in its revision |
| Ordinary provenance | `kind` exactly `ordinary` | Not applicable |
| Composition provenance | `kind` exactly `composition`, `applicationId`, `compositionId`, `outputIndex` | Not applicable |
| Recipe descriptor | `id`, `transformationVersion` exactly the qualified `0.1.0`, `byteLength`, `sha256`, `createdAtMs` | `id` |
| Application descriptor | `id`, `compositionId`, `targets`, `createdAtMs` | `id` |

Targets retain the exact closed new/append objects in the destination contract. Raw payload segments follow all source descriptors in order, then all revision descriptors in order, then all recipe descriptors in order. Each segment has exactly its declared length. Recipe segments are the normalized compact draft JSON, not the identity envelope. Their SHA-256, transformation version, recomputed composition ID and exact normalization must all match. Application identities and revision materialization are recomputed using the complete retained dependency closure.

The manifest represents the existing source, owner, revision and review tables plus all four schema 3 tables without losing live membership. Owner birth selects the legacy or derived owner table. Revision provenance supplies the exact derivation row; embedded review supplies the review row. Application acknowledgment reconstructs only the fixed unauthenticated `local_operator` label. Missing destination membership is never inferred from targets.

Version 1 archives retain their original decoder and meaning. The versioned recovery path may project validated V1 source/skill records into ordinary provenance. Existing V1 native APIs remain unchanged until the newer native data and capability contracts are integrated; they continue to reject schema 3. No automatic database migration follows reading, previewing or decoding a backup.

## Additive restore

Plan against the final union: preserve every local record, import all absent backup sources, and import each absent capability with its complete history and reviews. Never append missing history to an existing capability or replace its head or review. Retain only recipes and applications needed by final live derivations and origins. A pinned input missing from that final union blocks the operation explicitly, even when the archive contains an older history of an existing capability that the additive policy keeps unchanged.

Same-ID source bytes, name and digest must agree; preserve local save time. Same-ID owner birth must agree; preserve local head/history. Same-ID recipes must have identical normalized draft bytes and transformation version. Same-ID applications must have identical composition ID and ordered targets. Preserve local recipe/application timestamps; imported timestamps remain historical metadata. These comparisons establish consistency only.

The confirmation contains the exact decoded archive ID, byte length, current full workspace-state digest and all proposed addition/retention counts. The host retains the selected decoded backup and issued confirmation. Commit recomputes and compares the complete confirmation inside an immediate transaction, before inserting rows. A changed archive, state, count or dependency fails; no automatic retry or partial restore follows. Validate the final union and canonical allocation before writes; the actual destination's allocation ceiling still applies transactionally. Empty/no-addition restores do not promote an existing schema. Required migration and additions commit atomically.

## Deletion

Compute surviving revisions before checking dependencies. Other surviving capabilities referencing a selected capability's revisions block removal; references within the selected capability's own soon-deleted history do not. Source deletion is blocked by surviving source origins or recipe inputs. Confirmations bind kind, identity, current state, dependent skills and exact removal counts for sources, capabilities, revisions, reviews, recipes, applications and derivations.

Remove only selected records and their owned history/reviews/derivations. Garbage-collect applications with no live derivations, then recipes with no application or derived-origin reference. Never remove input sources or another capability's history through garbage collection. Compare the complete current confirmation before mutation, validate the retained records afterward, and commit atomically. Stale confirmation fails before missing-record/idempotence handling. Existing schema 1/2 deletion does not migrate the store.

## Qualification

Require independent byte vectors, V1 compatibility, exact V2 round trips, hostile manifest/length/order/identity tests, partial-output preservation, additive restore into newer/diverged local histories, missing pinned history, self/external dependency deletion, stale backup/state/count rejection, and canonical SQLite capacity checks. Interrupted migration, restore, deletion and disk-full rollback must preserve prior rows and heads. No composition writer is ready for publication before these recovery paths are qualified. Native integration and three-OS GUI evidence remain separate required work.
