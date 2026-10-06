# Composition destinations and revision provenance

Status: reviewed destination contract under the accepted version 1.0 objective; bounded pure application layer implemented; independent source review passed; persistence qualification pending
Authority: [composition specification](composition.md), under [intent.md](intent.md)
Owner: Jeff; primary controller owns architecture, schema, integration and release judgment

## Outcome

Every composition output has an explicit destination: create a new skill, or append a revision to an existing skill at a pinned expected head. All outputs are applied atomically. Earlier source bytes, revisions and reviews remain unchanged; every new revision is unreviewed with `authority: none`.

The operation's recipe and its application are separate records. The recipe describes which bytes were copied, replaced, authored or excluded. The application binds that recipe to an ordered list of destinations. A skill's original origin remains stable; each revision also exposes whether it came from an ordinary edit or a particular composition application.

This refinement preserves the existing composition v0 encoding and all existing source, capability and ordinary revision identities. It does not reinterpret the pure core's proposed new-capability IDs as IDs for existing destinations.

## Closed target contract

A target is one of these closed JSON objects:

- `{"kind":"new"}`
- `{"kind":"append","capabilityId":"capability:<digest>","expectedRevisionId":"revision:<digest>"}`

The ordered targets array has exactly one item per output, at most sixteen. All append IDs use the existing lowercase SHA-256 identity syntax. Reject two append targets for the same capability even when they name different expected revisions. After deriving new-capability IDs, reject any duplicate resulting capability across all destinations too. A single application cannot advance one capability twice.

The host resolves every append destination from the actual workspace. The chosen expected revision must still be its current head at preview and commit. A historic input remains a permitted input; it must not silently substitute for an append destination's current head. It is valid for a destination to also be an input, including the same capability's current or earlier revision. The operation creates a successor and never mutates the pinned input.

A new destination requires the derived capability ID to be absent. An existing derived capability must be chosen explicitly as an append destination. Readback and restore never recreate an output absent from live membership. After explicit deletion, a fresh user-confirmed application may deliberately create that output again with the same deterministic identity; this design keeps no permanent deletion tombstones and makes no permanent non-recreation guarantee. This requires a new host-issued preview against the current workspace, and the confirmation names every output being created. A previously retained application does not authorize filling missing outputs during readback. A stale mutation confirmation fails before idempotent-readback handling and never triggers an automatic write retry.

## Pure application preview

Add a pure application layer on top of `rangoon-compose`'s existing recipe preview. Its request contains required fields `schemaVersion` (`rangoon.composition-application-request.v0`), `draft`, and `targets` in that order. The draft retains its current closed v0 contract. The application request rejects unknown/duplicate fields and unsupported variants. Bound the complete request to `8 MiB + 16 KiB` before parsing, targets to sixteen before typed allocation, and the compact encoded nested draft to the existing 8 MiB bound. Retain all existing semantic sequence, byte, coverage and output limits.

Inputs are resolved by the host exactly as for recipe preview. A separate ordered internal list of `TargetHead { capabilityId, revisionId }` contains exactly one record for each append target, in append-target order, with no extra records. The pure layer validates these identities and exact expected-head matches. The store remains responsible for proving that the resolved records exist. New-destination absence and workspace quotas are store checks.

The result uses `rangoon.composition-application-preview.v0` and includes the existing core preview, an application ID, and one applied-output identity per output: `outputIndex`, `kind`, `capabilityId`, nullable `parentRevisionId`, and `revisionId`. Existing-destination revision IDs replace the core's provisional new-capability IDs only in this explicit applied-output collection. The core result remains unchanged and is never treated as a persisted result for an append destination. Saveability requires the core to be saveable and all destination checks to pass. Invalid/missing/mismatched target references are fixed errors with an output index and no source text; blocked core coverage remains an editable preview.

Public application-identity helpers accept the draft plus targets, validate their exact count and distinct derived destinations, and keep raw envelope encoding private. They do not prove saved-record existence, current heads, coverage or saveability; those require the complete preview and store validation.

The pure layer has no filesystem, database, clock, randomness, network or authority. Host-issued confirmation still wraps its exact result with the retained preview ID and expected workspace state. Changing any destination invalidates that confirmation just as changing a recipe does.

## Application and revision identity

The application envelope is compact UTF-8 JSON with fields `schemaVersion` exactly `rangoon.composition-application.v0`, `compositionId`, then `targets`. Target objects use the field order above; target order is output order. Use Serde JSON string escaping, no BOM or trailing newline. The recipe's composition ID is the unchanged v0 identity.

Application digest bytes are ASCII `rangoon.composition-application.v0`, one zero byte, the envelope length as unsigned 64-bit big-endian, then the exact envelope bytes. The ID is `composition-application:` plus lowercase SHA-256 hex. No timestamp or unauthenticated operator label participates.

For a new destination, use the existing composed-capability ID and its ordinary root revision ID from the pure core. Its origin already binds the recipe and output index. For an append destination, create a distinct composition revision ID so different derivations producing identical text cannot collapse into one ordinary revision. Its digest bytes are ASCII `rangoon.composition-revision.v0`, one zero byte, then these fields in order:

1. Capability ID: unsigned 64-bit big-endian UTF-8 length, then bytes.
2. Expected parent revision ID: the same length framing.
3. Application ID: the same length framing.
4. Output index: unsigned 32-bit big-endian.
5. Exact output title: unsigned 64-bit big-endian UTF-8 length, then bytes.
6. Exact output content: the same length framing.

The resulting ID retains the `revision:` prefix. Ordinary edits still use the unchanged ordinary revision algorithm, even when their parent is a composition revision. Store validation chooses the algorithm from a validated revision-derivation record; it never tries algorithms until one happens to pass. A missing or contradictory derivation fails closed. Independent golden application-envelope and revision vectors must precede storage use.

## Schema 3 refinement

Preserve schema 1/2 tables and existing data. The two-table new-capability proposal in the parent specification expands to four added tables; exact DDL is qualified with migration tests:

| Table | Fields and role |
| --- | --- |
| `compositions` | `composition_id`, `draft_json`, `transformation_version`, `created_at_ms`; immutable complete recipe |
| `composition_applications` | `application_id`, `composition_id`, `targets_json`, `acknowledged_by`, `created_at_ms`; immutable destination binding and explicit local transformation acknowledgment |
| `derived_capabilities` | `capability_id`, `composition_id`, `output_index`, `latest_revision_id`; birth origin and mutable head of newly created skills |
| `revision_derivations` | `revision_id`, `application_id`, `output_index`; exact application/output that produced a retained revision |

`acknowledged_by` is the fixed unauthenticated label `local_operator`. Recording it requires the exact current host-issued preview acknowledgment. It does not identify an authenticated person or grant execution authority.

Capability ownership remains the disjoint union of legacy `capabilities` and `derived_capabilities`. Shared revision/review tables cover both. Every new composition root and every composition-appended revision requires one derivation row. Ordinary legacy roots and ordinary edits have no derivation row. A derivation's output index must exist, and its target must match the revision's capability and parent. New targets require a matching derived owner and a root revision; append targets require the exact expected parent and a successor revision.

Before reads, backup, restore or mutation use application metadata, bounded closed decoding must validate `targets_json`, re-encode it in the canonical target order and require exact stored-byte equality. Recompute the referenced composition identity from its normalized recipe and the application identity from that composition ID plus canonical targets. Require both stored identities to match. Each live derivation must match this validated application, its output index, the recomputed recipe materialization and the appropriate revision-identity algorithm. Enforce unique `(application_id, output_index)` membership as well as one derivation per revision. Recomputing only the content or revision digest is insufficient. These checks prove internal consistency, not authenticated authorship: an actor able to replace the entire plaintext database can also recompute its unkeyed identities.

An application may retain only some of its original output revisions after explicit capability deletion. The immutable targets list describes the application as originally performed; the surviving `revision_derivations` rows define live membership. Validate destination existence/parent identity for live derivations only. Never recreate a missing destination merely because an immutable recipe or targets list mentions it. All recipe inputs remain required while any live derivation depends on that recipe.

Every capability still has exactly one connected linear parent chain; no disconnected revision or multiple head is accepted. Provenance traversal follows exact revision identities and recipe inputs, not capability IDs. A later revision of A may depend on an earlier revision of B while a later B depends on an earlier A; this is valid when the revision graph is acyclic. Reject actual revision-level cycles, missing references, duplicate owners, mismatched materialization and unexpected schemas. Use bounded memoized traversal. Retain limits of 128 capabilities, 32 revisions per capability, 1,024 total revisions, 128 recipes, 128 applications, provenance depth 128, and a 64 MiB database allocation ceiling.

Read-only schema 1/2 projections remain available without migration. The first successful composition commit upgrades to schema 3 atomically with the recipe, application, all output revisions, new owner rows and head updates. Input validation, destination-head checks, expected workspace state, resource limits and explicit acknowledgment are checked before mutation in the same immediate transaction. Existing head/review metadata and every prior revision survive failure. Reads and previews never initialize or migrate the store.

## Revision and UI contract

Capability v1 keeps its discriminated birth origin from the parent specification. Revision details and summaries additionally have required discriminated `provenance`:

- `{"kind":"ordinary"}` for a legacy root or ordinary content edit; the capability's birth origin and parent chain remain inspectable.
- `{"kind":"composition","applicationId":"...","compositionId":"...","outputIndex":0}` for a composition root or successor.

The Skills workbench shows the selected revision's provenance as well as the skill's origin. An ordinary edit after a composition still links to its parent, but never claims to preserve the earlier composition's byte mapping. Local review binds only the selected exact revision, with no inheritance to a successor.

Each output card offers Create new skill or Update existing skill. Selecting an existing target shows its current title/head and a before/after preview without exposing editable hash fields. The save confirmation names which skills will be created and which will receive revisions. A stale target preserves the draft and offers explicit refresh/review. Successful save opens the actual created or updated skills with unreviewed new revisions. These remain proposed native UI behaviors until implemented and qualified.

## Dependency deletion and recovery

Deletion computes the proposed surviving revision set first. A capability cannot be deleted if any surviving revision owned by another capability depends on one of its revisions. Its own composition-derived revisions are removed with it and must not block its deletion merely for referencing their own earlier history. Source deletion remains blocked by any surviving legacy origin or recipe input.

Remove a capability's own revisions, reviews and derivation rows together. Remove an application only when no derivation references it; remove a recipe only when no application or derived origin references it. Never garbage-collect source snapshots or another capability's revisions. Preview counts and workspace state IDs include all affected rows, and the existing explicit confirmation and stale-state rules remain in force.

The next portable backup version carries all four tables and the complete live dependency closure. Canonical reconstruction validates both ordinary and composition revision identities, exact recipe materialization, live memberships, ordering, quotas and SQLite allocation overhead. Version 1 archives retain their old interpretation.

Additive restore still never appends history to or changes an existing capability. Compute the proposed final union of preserved local records and whole absent imported capabilities before validating dependency closure. Import only recipes/applications required by live derivations in that union. Existing same-ID recipe/application records must match all immutable identity-bearing fields; timestamps remain local historical metadata. Missing pinned input revisions block the dependent import explicitly. A deleted output remains deleted when absent from the archive's live membership. Validate every surviving derivation against its actual parent; dead targets mentioned in an immutable application do not require restoring those destinations.

Backup preparation, save, restore and deletion must all agree on the same schema 3 validation. No schema 3 persistence is publishable before backup, additive restore, deletion, stale-state and interrupted/disk-full recovery tests cover these paths.

## Required evidence

Before native integration, prove pure new/append/mixed output identities with independent vectors; reject duplicate targets, forged/missing heads and oversized requests; show changing destination, parent or recipe changes the application/append identity. Prove same output text under a different recipe cannot collapse into an ordinary edit.

Storage tests must cover legacy skill append, derived skill append, ordinary editing after composition, target-as-input including historical input, mixed new/append atomic commits, exact stale-head conflict, capacity rollback, migration interruption, self-dependency deletion, surviving external dependency blocking, partially deleted output membership, old backup compatibility, and additive restore with missing required history. Prove that readback and restore do not fill missing outputs, while a fresh confirmed create can deliberately recreate a deleted output; stale confirmation cannot do so. Reject altered targets/application IDs, noncanonical stored target bytes, duplicate application/output memberships and mismatched derivation materialization even when an individual content hash remains valid. Native tests and all-three-OS GUI qualification must demonstrate the corresponding usable flows before declaring R3 complete.
