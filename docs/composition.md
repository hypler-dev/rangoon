<!-- intent-driven-delivery:spec:v1 -->
# Specification: Source-preserving composition

Status: experimental pure-core implementation under the accepted version 1.0 objective; persistence and native delivery remain incomplete; no completed R3 or runtime claim
Intent: [intent.md](intent.md), version 1.0 build continuation
Owner: Jeff; primary controller owns architecture, storage, native integration and release judgment
Last updated: 2026-10-05

## Outcome and product behavior

Make Decompose and Merge & Split operate on actual saved content. A user can turn a saved instruction file into several reusable skills, combine selected skill revisions into one skill, or divide a selected skill revision into several skills. A preview must show exactly which input bytes each output retains, duplicates, replaces or excludes. Saving produces new capabilities in one transaction, retains every input, and never transfers local review to an output.

These are manual and deterministic content transformations. Heading-based decomposition is a useful starting proposal, not an assertion of semantic understanding. The user can refine boundaries and wording. No confidence percentage, automatic policy-conflict resolution, provider call, execution permission or LNSAT connection is implied. Conflicting instructions remain visible until the user records a resolution; the application cannot prove that arbitrary prose preserves every constraint.

The illustrated graph is an editor over these real records. It is not a separate synthetic data model. Sources and selected revision text stay inspectable beside the preview. Show the next unresolved input span and offer Assign, Duplicate deliberately, Replace with explanation, or Exclude with explanation. Use one primary Save composition action after complete coverage and explicit content review of the transformation. This acknowledgment is not the existing per-revision review badge and is not authenticated approval.

## Operations and pinned inputs

- **Decompose:** one complete saved source snapshot, one or more outputs. Propose boundaries from the existing analyzer's complete span partition, including preamble and whitespace. The user may regroup spans before saving.
- **Merge:** two to sixteen distinct pinned capability revisions, exactly one output. Initial proposal concatenates their exact content in the displayed input order with any introduced separators explicitly represented as authored text.
- **Split:** one pinned capability revision, two to sixteen outputs. Start with an editable complete partition; never silently omit leading context, separators or trailing text.

Source references bind the exact source ID and original-byte digest. Revision references bind capability ID, exact revision ID and exact content digest. Historical revisions may be chosen explicitly; they are labeled historical and never silently replaced by current heads. The native host resolves the references from the validated workspace. The renderer cannot supply purported original bytes.

Inputs are whole records in R3: a source's complete original content or a revision's complete saved content. Span selections address those records by zero-based, half-open UTF-8 byte offsets. Every endpoint must be a character boundary. Display one-based line context alongside byte spans, but never derive persisted byte positions from JavaScript UTF-16 indices without conversion. Preserve BOM, CRLF, Unicode and terminal newlines exactly.

## Composition draft and output recipes

The pure domain contract accepts a closed, versioned draft plus resolved immutable inputs. A draft contains operation, ordered input references, ordered output recipes, explicit exclusions, deliberate-duplication acknowledgments and conflict declarations with their resolution state. The host validates identity and bounds before passing content to the domain layer.

An output recipe has a validated title and an ordered list of pieces:

1. **Copy:** input index plus exact start/end byte offsets. Output contains those bytes unchanged.
2. **Authored:** exact UTF-8 text and a nonempty explanation. Separators are authored pieces too. New prose has no inherited source span or review.
3. **Replace:** one exact input range, replacement text and a nonempty explanation. Coverage records those input bytes as replaced, not preserved. The output retains a provenance link to the original range.

An exclusion identifies one exact input range and a nonempty explanation. Exclusions do not create output bytes. A duplication acknowledgment identifies an exact input range copied to more than one output or repeated within an output and records why. The preview computes usage from actual recipes; a renderer-provided coverage count or acknowledgment cannot invent a copy edge.

Each conflict declaration is a closed object with `id`, `title`, `ranges`, `context` and `resolution`. The ID is a positive draft-local unsigned 32-bit integer, stable across edits and unique within the draft; it is not a content identity or authority token. The title follows the output-title bound. There must be two to sixteen distinct, nonempty input ranges and a nonempty bounded context explanation. `resolution` is required and nullable: `null` means unresolved and blocks saving; a nonempty bounded explanation means the operator recorded a resolution. A missing field, duplicate conflict ID, duplicate range or unsupported resolution shape is rejected. Resolution is embedded in its declaration, so a detached/orphan resolution is never accepted. Canonical identity includes each declaration, its exact ranges/context and its nullable resolution, ordered by conflict ID. These records document operator-observed conflicts; they do not certify complete conflict detection. Source wording is shown with each declaration and the resolution is stored with the composition. No automatic rule ranks one source's prohibition below another's instruction.

The domain preview computes output content, output digests, provenance edges and a coverage ledger. For each input, split at all piece/exclusion/acknowledgment/conflict endpoints into a complete ordered partition. Each nonempty segment is classified as copied once, deliberately copied multiple times, replaced, excluded, or unassigned. Mixed copy/replacement/exclusion classifications are conflicts and block saving. Repeated copies require an acknowledgment covering the exact duplicated region; acknowledgments covering nonduplicated bytes are rejected. Overlapping replacements or exclusions are rejected. Unassigned bytes block saving. Empty source input has no byte gaps but cannot create an empty output.

Any unresolved declared conflict, invalid range, unknown reference, unacknowledged duplicate, mixed disposition, unassigned input, invalid output or exceeded bound keeps the draft editable and blocks commit. Error messages identify bounded input/output indices and spans without echoing arbitrary source text into logs. The UI shows both why a proposal is blocked and how to fix it. Copying bytes proves byte preservation only; it does not prove safe behavior or policy equivalence.

The final transformation acknowledgment binds the exact latest preview ID. Editing any recipe, input, explanation or conflict invalidates it. The native commit requires that exact host-issued preview, current workspace state and an explicit acknowledgment; a generic checked box cannot acknowledge a later draft. Store this as an unauthenticated local transformation acknowledgment, separate from per-revision content reviews. New output revisions remain unreviewed.

## Bounds and identity

Retain existing title and output-content limits: trimmed single-line title of at most 160 UTF-8 bytes; nonblank output of at most 256 KiB without NUL. Bound a composition to sixteen inputs, sixteen outputs, 256 total output pieces, 256 exclusions, 256 duplication acknowledgments and 128 declared conflicts. Each explanation is trimmed, nonempty, at most 1,024 UTF-8 bytes without NUL. Bound total resolved input bytes and total materialized output bytes separately to 4 MiB. Bound serialized draft bytes before native deserialization; the precise native request ceiling is defined with its wire command, not inferred from a per-record limit.

Every public structure rejects unknown fields and unsupported schema/operation/piece variants. Integer offsets are unsigned byte offsets; values exceeding input length or machine-safe UI integer bounds are rejected. Validate count and byte limits before allocation or traversal. Bound the complete coverage partition to 4,096 segments. The preview is deterministic for the same draft and resolved inputs.

New composition identities use a dedicated versioned, length-framed SHA-256 domain. Bind operation, ordered exact input identities/digests, ordered output titles and piece recipes, normalized exclusions/duplication acknowledgments, conflict declarations/resolutions and the transformation version. List normalization and field framing must be specified with independent golden vectors before persistence. No identity depends on timestamps, locale, incidental JSON key order or randomized map order. Existing source, legacy capability and revision identity algorithms remain unchanged.

A derived capability ID binds composition ID and output index under a new identity domain; it must not masquerade as a legacy single-source capability. Its root revision uses the existing revision identity function with that new capability ID, no parent and the exact materialized title/content. Subsequent ordinary edits keep the same immutable parent-chain behavior. The provenance of the initial composition remains available after edits; an edited successor is not automatically considered to preserve its ancestor's bytes.

IDs, digests and operator annotations remain unauthenticated content records. The separate [local workspace boundary](local-workspace.md) remains plaintext until a separately qualified cryptographic design is adopted.

### Pure-core wire and identity contract

Implement a separate `rangoon-compose` crate depending only on the existing domain types, Serde and SHA-256 through the domain helper. It owns no files, database, clock, provider or network. Expose bounded JSON draft decoding, deterministic preview over host-resolved inputs, and derived identity helpers. The core verifies that resolved references exactly match the ordered draft references and recomputes every input digest; the store remains responsible for verifying that those identities are real saved records.

All JSON fields below are required, including arrays when empty. Field order shown is the canonical encoder order. All objects reject duplicate or unknown keys; all enums reject unknown variants. A bounded JSON entry point rejects payloads over 8 MiB before parsing and bounds sequences during deserialization; validation also enforces the same bounds for directly constructed Rust values.

| Object | Canonical fields |
| --- | --- |
| Draft | `schemaVersion` exactly `rangoon.composition-draft.v0`, `operation` (`decompose`, `merge`, `split`), `inputs`, `outputs`, `exclusions`, `duplications`, `conflicts` |
| Source input | `kind: source`, `sourceId`, `sha256` |
| Revision input | `kind: revision`, `capabilityId`, `revisionId`, `sha256` |
| Range | `inputIndex` unsigned 32-bit integer, `startByte` unsigned 64-bit integer, `endByte` unsigned 64-bit integer |
| Output | `title`, `pieces` |
| Copy piece | `kind: copy`, `range` |
| Authored piece | `kind: authored`, `content`, `reason` |
| Replace piece | `kind: replace`, `range`, `content`, `reason` |
| Exclusion or duplication acknowledgment | `range`, `reason` |
| Conflict | `id`, `title`, `ranges`, `context`, `resolution` (required string or null) |

Copy, replace and annotation ranges must be nonempty. Authored and replacement content may contain whitespace or be empty while editing, but no NUL and at most 256 KiB per piece; final output must satisfy the existing nonblank content limit to be saveable. Empty outputs or invalid output titles are blocking preview diagnostics, so incomplete drafts can still show coverage. An invalid input reference, invalid range, unsupported schema, malformed explanation or resource-limit violation is a fixed core error. A source input may be empty; no output may be committed empty. Source inputs are valid only for Decompose; revision inputs are valid for Merge/Split. Reject duplicate input references.

Normalize exclusions and duplication acknowledgments by `(inputIndex, startByte, endByte, reason)` using unsigned numeric order and UTF-8 byte string order; reject overlapping annotations of the same class. Sort a conflict's distinct ranges by `(inputIndex, startByte, endByte)` and conflicts by their unique numeric IDs. Preserve input, output and piece order. This normalization changes list presentation only; it does not trim, normalize Unicode/newlines, infer reasons or alter source text.

Encode the normalized identity envelope as compact UTF-8 JSON with fields `transformationVersion` exactly `0.1.0`, then `draft`. Use the field order above, Serde JSON string escaping, no BOM, no extra whitespace and no trailing newline. Composition digest input is raw ASCII `rangoon.composition.v0` followed by one zero byte, the envelope byte length as unsigned 64-bit big-endian, then the exact envelope bytes. ID is `composition:` plus lowercase SHA-256 hex. This is an explicitly specified encoding, not incidental object/map iteration order.

Derived-capability digest input is raw ASCII `rangoon.composed-capability.v0`, one zero byte, the composition-ID UTF-8 length as unsigned 64-bit big-endian, the composition-ID bytes, then output index as unsigned 32-bit big-endian. ID is `capability:` plus lowercase SHA-256 hex. Its initial revision uses the unchanged domain `revision_id` with no parent and materialized title/content. Publish independent golden envelope bytes and expected identity digests alongside tests before storage consumes this format.

Pure result schema `rangoon.composition-core-preview.v0` reports composition ID, saveability, proposed output content/digests/IDs, ordered piece-to-output byte mappings, per-input coverage segments, bounded structured diagnostics and `authority: none`. It does not issue a native confirmation ID or claim workspace state. Every mapping identifies output/piece index, output byte interval, piece kind and its input range where applicable. A replacement mapping associates whole before/after ranges; it must not claim byte-for-byte alignment. Coverage segments expose their exact range, disposition and output/piece references; copied duplicates also expose whether a valid acknowledgment covers the segment. Declared unresolved conflicts are separate diagnostics linked by conflict ID and exact input ranges. They are not silently treated as an automatic semantic classifier.

Input content and output materialization remain bounded to 4 MiB each. After validation, computing partitions and mappings must use bounded segment/count checks before allocating results. All fixed errors/diagnostic codes avoid interpolated source content. Human-readable content appears only in explicit content fields, never as presumed executable instructions.

## Storage and compatibility

### Output destinations still to qualify

The supplied Merge & Split reference includes both **Create new skill** and **Update existing skill**. Both remain part of the intended product. The initial pure-core contract below materializes new capabilities only; that is an implementation stage, not a decision to remove the existing-skill destination.

Updating an existing skill must append an unreviewed revision at an explicitly pinned expected head while retaining its earlier revisions, reviews and original provenance. It must also retain the new composition's exact recipe and input references at the resulting revision. A capability-level origin alone is insufficient: an originally source-derived skill can later receive a composition-derived revision. Before implementing the schema 3 migration, resolve and independently review revision-level derivation ownership, deterministic identity binding to the target and parent, target/input overlap, output-target uniqueness, stale confirmation, backup closure, and deletion dependencies. No existing revision may be overwritten or silently reclassified. The schema proposal that follows describes the new-capability destination and is not yet the complete persistence contract for both destinations.

Preserve all schema 1/2 tables and identity algorithms. Schema 3 adds two tables: `compositions(composition_id, draft_json, transformation_version, created_at_ms)` and `derived_capabilities(capability_id, composition_id, output_index, latest_revision_id)`. Their exact DDL is qualified with migration tests. The existing `capabilities` table remains exclusively the legacy source/fragment origin and is neither rebuilt nor populated with invented source/fragment values. A capability has exactly one owner row across the disjoint union of legacy `capabilities` and `derived_capabilities`. A duplicate ID across those tables is corruption, even if other fields match. Each live derived row references one composition and a unique output index within that composition.

Schema 3 changes capability lookup and validation, not just origin metadata. List reads the disjoint union sorted by capability ID; all capability/revision quotas apply to that union. Open resolves the owner first, validates the shared revision chain and head, then validates the appropriate origin. Legacy root content and identity still match the original source fragment. A derived root identity/content instead match materialization of the retained composition's indexed output. Every revision has exactly one owner in the union; reviews still bind revisions. Revise and Review use the same expected-head rules for both kinds; Revise updates only the owning table's head. Delete selects the owner, checks composition dependencies, then removes only that owner's revisions/reviews and owner row. An unrecognized or ambiguous origin never falls back to legacy handling.

The new `rangoon.capability.v1` detail retains common ID, head, revision, history and `authority: none`, but replaces the flat single-source fields with a required discriminated origin. A `source` origin contains source ID, fragment ID, source name, exact span and original text. A `composition` origin contains composition ID, operation, output index and ordered pinned input references, with a separate detail read for the complete recipe and coverage. It has no fictitious single `originalText` or source span. Versioned summaries carry common title/review/count fields and either a source reference or composition reference. The workspace inventory DTO must carry these versioned summaries too. All native commands, strict renderer validators, Skills provenance, Workspace dependency views and test fixtures change together. The new application reads schema 1/2 through the same v1 projections without migration; old binaries reject schema 3 instead of rewriting it.

The immutable composition record holds the validated draft, transformation version, ordered input references, initial output recipes/digests and created time. A separate derived-capability record links a live capability to its composition/output index and current head. Source and revision inputs reference the actual saved records; no duplicate text is treated as a substitute for provenance. Reads revalidate derivations and reference closure. Bound retained compositions to 128 and provenance depth to 128; use cycle detection and memoized validation over bounded record identities rather than repeatedly expanding nested text graphs. Cyclic, missing, ambiguous or inconsistent provenance is rejected. No read or preview implicitly creates/migrates a store.

An explicit save validates all inputs, the workspace state token, coverage/conflicts, limits and all output identities in one immediate transaction. Migration, composition record, every output capability and its root revision commit together or not at all. Current input records are not modified. New roots are unreviewed with `authority: none`. Reopening the same exact committed composition may be recognized read-only; stale mutation confirmation still fails first, matching R2c. No automatic write retry follows an uncertain response.

Deleting a capability referenced by a retained composition is blocked and names its dependents. Source deletion includes direct composition input references as well as legacy capability references. Deleting one derived output requires the ordinary explicit R2c confirmation and removes only that output's history/reviews; it does not remove sibling outputs. The immutable composition recipe may therefore describe outputs no longer present. The live derived-capability records define current membership, and reads never recreate missing outputs. After the last unreferenced output is explicitly removed, its otherwise unused composition record may be removed in that same transaction. This garbage collection must not remove source/revision inputs or other compositions. Preview counts and state identity include these effects.

Saved composition drafts need an explicit recovery design before the UI claims draft persistence. Initial UI may retain an unsaved draft in memory and warn before discarding it; do not silently label it saved. The accepted full-v1 project/revision recovery work remains outstanding until durable draft handling is built and qualified.

## Backup and restore integration

Schema 3 cannot ship without backup, restore and deletion support for its new records. Introduce a versioned portable format that carries compositions, derived-capability membership and all referenced input records. Preserve version 1 decoding and validate its existing semantics. The exact new manifest is specified with the storage implementation and independent format vectors; no existing archive is silently reinterpreted as a composition.

Additive restore must account for dependency closure. Keeping an existing capability unchanged can mean a required pinned revision from the archive is absent locally. In that case, restoring a dependent composition is blocked with an explicit conflict; do not append archived history to the existing capability or invent an input revision. Matching identities must resolve to matching content, and derived origins must retain their exact composition. Restore preview and mutation share the same plan and stale-state checks. Capacity validation includes recipes, metadata, indexes and all new tables, not just text bodies.

Do not restore recipe outputs whose live membership is absent in the archive. Atomic restoration and migration preserve previously committed data on invalid provenance, insufficient space, unsupported format and interruption. Archive review annotations remain historical unauthenticated metadata.

## Native and UI delivery

Keep pure transformation logic independent of Tauri, SQLite, providers and network. A narrow native preview resolves references and returns computed output/coverage diagnostics; a separate confirmation saves the host-validated composition against expected state. The native result wraps the pure result with its own schema version, host-issued opaque `previewId` and `expectedStateId`. The host retains at most one issued preview: normalized draft, resolved input identities/digests, exact core result and expected workspace state. A fresh successful preview replaces the previous slot and gets a new process-independent random identifier; failure does not fabricate an issued preview. IDs are local selection handles, not authorization credentials.

Commit accepts only the matching retained `previewId`, matching `expectedStateId` and explicit acknowledgment of that exact preview. It never accepts replacement recipes, input bytes or a renderer-supplied core result alongside confirmation. The host rejects missing, replaced, consumed or previous-process handles, rechecks workspace state and input digests inside the save transaction, and consumes the slot only after a verified commit. A lost response still requires explicit readback/new preview; it does not authorize automatic retry. The deterministic composition ID is not substituted for this host-owned preview handle. Exact command names, closed wrapper DTOs, secure random-ID generation and native payload-size enforcement are defined and tested before exposing these commands. The renderer cannot assert a composition is valid, forge an original span or bypass host validation.

Provide real Decompose and Merge & Split navigation alongside Import, Skills and Workspace. Allow selection of saved input revisions, output naming and ordering, span assignment, deliberate duplication, reasoned replacement/exclusion, declared conflict resolution and preview inspection. Show source-backed versus authored text distinctly. Keep graph/list/editor selection synchronized. A failed save preserves the draft and displays the stale or unavailable state; successful save opens the actual new skills. Theme changes and keyboard navigation preserve the draft. Loading, empty, unavailable, malformed, conflicted, incomplete, ready, saving and saved states need explicit behavior.

Keep the existing sample dashboards clearly identified until replaced by real routes. Do not show sample counts or invented engine readiness in real composition. Layouts must support dark/light, reduced motion, keyboard use, announced validation and narrow screens. Preserve the chosen Rangoon OS name/logo.

## Delivery stages and acceptance

R3 is complete only when the full native lifecycle, storage/recovery and image-derived UI work. Staging the implementation does not reduce that outcome:

1. **Pure composition core:** closed types, exact materialization, coverage/conflict validation, deterministic identities and independent golden/negative fixtures. Test UTF-8 boundaries, BOM/CRLF, omitted separators, reordered pieces, deliberate duplicates, exclusions, replacements, declared conflicts and every resource bound.
2. **Durable lineage:** schema 3, versioned origin DTOs, atomic multi-output commit, immutable edits, reference-closure validation, dependency deletion, backup/restore migration, stale state and interrupted/disk-full recovery. Prove old schema 1/2 data and version 1 backups still work.
3. **Usable native workbench:** actual Decompose, Merge and Split through typed IPC, synchronized preview/provenance, draft preservation, keyboard/responsive dark/light evidence and native restart/reopen. No synthetic fixture counts in real screens.
4. **Qualification and publication:** independent architecture/storage/UI reviews, named tests and builds, exact-head CI, reviewed main publication and per-OS GUI evidence. Source CI remains distinct from installer, security and release qualification.

No engine activation, provider transmission, filesystem scanning, policy enforcement, encryption implementation, installer publication, license adoption, grant submission or production mutation belongs to this composition packet. Those remain separate work under the broader product objective and its established gates.
