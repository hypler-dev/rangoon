<!-- intent-driven-delivery:spec:v1 -->
# Specification: Reviewed capability revisions

Status: accepted
Intent: [intent.md](intent.md), version 1.0 build continuation
Owner: Jeff; primary controller owns architecture, storage, native integration and release judgment
Last updated: 2026-10-05

## Behavior

R2b adds a real Skills workspace alongside source analysis. Select a section of an explicitly saved source, name it, and create a draft capability. The first revision copies that section's exact bytes. Edit the title and content, compare with the original and previous revisions, save a new immutable revision, and explicitly mark the current revision reviewed. Review means this local operator inspected this content; it is not authenticated authorship, validation, compatibility, publication or execution permission. Every result retains `authority: none`.

Source reports and fragments remain unchanged and unreviewed. Capability revisions have their own review records. Editing a reviewed revision creates an unreviewed successor; old content and its review survive. Unassigned sections remain visible. A provenance link records derivation, not a guarantee that edited content preserves meaning. No source span disappears because its derived capability changed.

## Interfaces and contracts

The host owns the workspace path. Five local main-window commands accept typed bounded content and opaque IDs; no path, SQL, provider or engine request is accepted:

| Command | Arguments | Result |
| --- | --- | --- |
| `list_capabilities` | None | `listed`, `capabilities` summaries |
| `create_capability` | `sourceId`, `fragmentId`, `title` | `opened`, `capability` detail, `alreadyApplied` |
| `open_capability` | `capabilityId`, optional `revisionId` | `opened`, `capability` detail |
| `revise_capability` | `capabilityId`, `expectedRevisionId`, `title`, `content` | `opened`, `capability` detail, `alreadyApplied` |
| `review_capability` | `capabilityId`, `expectedRevisionId` | `opened`, `capability` detail, `alreadyApplied` |

Failures return `failed` with fixed `error.code` and `error.message`. Detail schema is `rangoon.capability.v0`: `id`, `sourceId`, `fragmentId`, `sourceName`, `span`, `originalText`, `latestRevisionId`, `revision`, `history`, `authority`. The revision has `id`, nullable `parentRevisionId`, `title`, `content`, `sha256`, `createdAtMs`, and nullable `review` (`reviewer: local_operator`, `reviewedAtMs`). History entries omit content but retain other revision fields. Summaries contain `id`, `sourceId`, `fragmentId`, `latestRevisionId`, `title`, `reviewed`, `revisionCount`.

Identity uses versioned length-framed SHA-256: capability binds source ID, fragment ID and initial title; revision binds capability, parent ID, title and exact content. Identical creation is an explicit deduplicated operation. An edit retry succeeds only if the exact derived revision is still the head; a conflicting head returns `capability_conflict`. Content-identical saves are no-ops. Reviews bind the exact current revision and are idempotent. IDs/digests are integrity checks, not signatures or authority tokens.

## States and failure handling

The UI distinguishes missing native bridge, empty collection, loading, unavailable list, draft, unreviewed, reviewed, historical revision, pending mutation and stale revision. Failed writes preserve the draft and last successfully opened record. Conflicts require explicit reload before another edit; no automatic overwrite or retry. Historical revisions are inspectable, with a separate return-to-current action. Review is disabled while unsaved edits exist or a historical revision is selected. Inputs render only as escaped text. Theme/navigation must preserve unsaved drafts and keyboard focus.

Limits: 128 capabilities, 32 revisions per capability, 1,024 total revisions, 160 UTF-8 bytes for a nonempty single-line title, 256 KiB UTF-8 content without NUL, and the existing 64 MiB database ceiling. Empty edited content is rejected. No automatic eviction. Local wall-clock timestamps are not trusted evidence.

## Data, privacy, and permissions

Explicit capability creation, revision save and review write unencrypted application-local SQLite data. Source originals stay intact. No browser content storage, telemetry, upload, remote model or imported instruction execution is added. A selected source must already exist in the validated source store; the renderer cannot forge its original bytes or spans. The existing bounded native busy guard covers mutations. The renderer may supply edited capability content solely as inert data.

## Compatibility and migration

Existing schema 1 stores remain readable. Source-only Save keeps schema 1. Only explicit capability creation upgrades a validated schema 1 database to schema 2 within the same immediate transaction as the first capability. Schema 2 adds exact `capabilities`, `revisions` and `reviews` tables while preserving the original snapshots table byte for byte. Read/list operations do not migrate or create stores. Unknown or modified schemas fail closed.

DDL, version update and first capability commit atomically. A failed or interrupted migration rolls back to schema 1 with existing source intact. Later writes also use immediate transactions and expected-head checks. An older Rangoon build rejects schema 2 instead of overwriting it; downgrade requires a compatible binary or a user-managed pre-upgrade backup. This stage does not provide automatic backup/restore or claim power-loss durability on all filesystems. Those remain release work. Recovery tests must cover rollback, interrupted migration, stale concurrent writers, corrupted lineage/review, original-byte retention and quotas.

## Acceptance mapping

1. Exact saved source section becomes a durable draft; original bytes, source ID, span and hash survive restart.
2. Revision edits preserve history, reject stale heads, invalidate current review and support safe exact retries.
3. Local content review cannot alter source analysis or grant execution authority; corrupted records are rejected.
4. Failed migration and interrupted writes preserve committed source data and existing capabilities.
5. Native/renderer contracts, draft retention, review gating, historical comparison, source coverage, keyboard focus and responsive dark/light states receive named tests and rendered evidence.
6. Fresh independent storage/native and UI reviews precede publication; three-OS source checks and manual GUI evidence remain separate from release qualification.

## Non-goals and open questions

No semantic extraction, arbitrary multi-source spans, merge/split conflict solver, dependency semantics, authenticated reviewers, project/tenant identities, signing, compiler, export, engine connection, provider call, deletion or installers. This is the first real capability lifecycle; later packets extend it without treating local content review as authority.
