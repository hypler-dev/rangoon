<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/rangoon-wordmark-dark.svg">
    <img src="docs/assets/rangoon-wordmark-light.svg" alt="Rangoon.ai" width="420">
  </picture>
</h1>

**A local-first workbench for building, understanding, and governing AI capabilities.**

Rangoon brings agent instructions, reusable skills, workflows, connectors, tests, and evidence into one inspectable lifecycle. The immediate problem is practical: useful knowledge is scattered across `AGENTS.md`, `CLAUDE.md`, `SKILL.md`, project rules, and scripts. Teams need to understand that material, preserve its origin, reorganize it without silently losing constraints, and see exactly what will change before publishing it elsewhere.

Core product scope also includes approval management, redacted correlated telemetry and live operations visualization: operators need to see who can approve, which controls mediate an action and what evidence establishes its outcome. These runtime surfaces are planned and are described separately from implemented local content work.

The long-term product is a desktop and cloud control plane. The current implementation is an **experimental desktop source workbench**, with real local persistence, immutable skill revisions, direct composition editing, portable workflow definitions/revision candidates, an actual `#model-assistance` workbench, and a separate synthetic design preview. The native application has no live agent executor, connector execution, or LNSAT transport. Its model paths provide explicitly configured numeric-loopback local sessions and an optional native cloud route, all advisory. Native cloud orchestration now owns ten bounded commands, explicit capability configuration, full-envelope credential custody, final native consent, freshness checks and a fixed-origin TLS adapter; provider compatibility, proposal application, quality, GUI and release qualification remain open. Native OS credential management remains separate and explicit; no existing key or paid/private call is used by source validation.

The first desktop release targets **macOS, Windows, and Linux together**. Rangoon's original source and documentation are licensed under [Apache License 2.0](LICENSE); the initial community product remains free. Released operating-system support is not implied by public source or license adoption.

![Rangoon Command Center design preview](docs/screenshots/command-dark.jpg)

*Command Center design preview: synthetic data showing the intended visual direction. Native features and their evidence are distinguished below.*

## Contents

- [Product and feature status](#product-and-feature-status)
- [Architecture](#architecture)
- [Import and source analysis](#import-and-source-analysis)
- [Workspace and revision model](#workspace-and-revision-model)
- [Composition: Decompose, Merge, and Split](#composition-decompose-merge-and-split)
- [Workflow definitions and revision records](#workflow-definitions-and-revision-records)
- [Instruction compilation](#instruction-compilation)
- [Model assistance](#model-assistance)
- [Backup, restore, and deletion](#backup-restore-and-deletion)
- [Native application boundary](#native-application-boundary)
- [Local database security](#local-database-security)
- [Approval, telemetry and live operations](#approval-telemetry-and-live-operations)
- [LNSAT integration](#lnsat-integration)
- [Run from source](#run-from-source)
- [Validation and platform support](#validation-and-platform-support)
- [Roadmap](#roadmap)
- [License and commercial services](#license-and-commercial-services)
- [Contributing and documentation](#contributing-and-documentation)

## Product and feature status

The first useful experience is deliberately concrete: select a Markdown file, inspect its exact content and section inventory, save a snapshot, derive a skill, edit a new revision, record local review, and reopen that work after restart. Composition extends this into traceable transformations across saved records. The native Compile workbench turns exact saved revisions into inspectable AGENTS.md/CLAUDE.md artifacts and candidate manifests through a pure compiler and a read-only workspace service. Native Export and external bundle inspection are implemented in this source through the native command/UI path described in [instruction-bundle-ui.md](docs/instruction-bundle-ui.md); isolated macOS native QA now covers the recorded import/save/derive/review/Compile, export, external Inspect, and restart flows, while collision/write uncertainty, composition compile, full keyboard coverage, Windows/Linux GUI, and target-loader qualification remain open. Publication evidence belongs in the [development ledger](docs/development.md).

The intended users are developers managing agent configuration, platform teams maintaining shared capabilities, and reviewers who need source lineage and explicit change plans. Cloud administration and government use are later qualification targets, not capabilities established by the current desktop prototype.

| Area | Implemented in this source | Remaining work |
| --- | --- | --- |
| Design preview | Nine synthetic views: Command Center, Import, Decompose, Merge/Split, Skills, Workflows, Connectors, Evidence, and release planning; dark/light themes and responsive layouts | Replace each sample workflow with a real, qualified application service |
| Import & Analyze | Bounded Rust Markdown scanner, stdin CLI, and one explicitly selected native file; exact text, digest, spans, and diagnostics | Explicit directory/repository import, additional formats, optional semantic proposals |
| Local workspace | SQLite source snapshots; explicit save, deduplication, reanalysis on reopen | Broader project/workspace model and platform qualification |
| Skills | Source-section derivation, immutable revisions, history comparison, local content review, versioned provenance | Search/library expansion, compatibility, distribution, and collaboration |
| Decompose/Merge/Split | Real native editor routes and direct composition canvas: drag/drop, click-to-connect, contextual/keyboard actions, bounded undo/redo, exact recipes/coverage, new/append destinations, retained previews, atomic storage, and schema 3 recovery | Complete advanced GUI/failure qualification and Windows/Linux interactive evidence |
| Data controls | Inventory, plaintext portable backup, additive restore, dependency-aware logical deletion; optional keyed backend, source-only native key/session ownership, shared callback drain admission | Current desktop remains plaintext; native encrypted adoption/cleanup/consent, verified migration, encrypted archives and platform qualification remain open |
| Engine integration | Explicit unavailable LNSAT port, native status page, and CLI diagnostics | Qualified transport, authentication, typed operations, and evidence readback |
| Model assistance | M1a pure pack/validation, session-only local custody, native local workbench, and native cloud orchestration are implemented. Cloud exposes ten bounded main/review commands, explicit capability configuration, retained full-envelope credential pairing, source-free GET checks, fixed-origin TLS, final parented OS consent, and pre/post freshness checks; outputs remain inert and advisory | Real provider compatibility, tokenizer/accounting, proposal application, three-OS GUI, security and release qualification remain open; M1b/V1 are incomplete |
| Workflows | Pure typed definitions and structural validation; durable schema-4 history/recovery; six native authoring commands; real saved-library/canvas/inspector source with separate control/data wires, six node types, exact capability pins, draft/validated saves, historical reads and comparison | Native save/restart/history/stale-state GUI qualification across all three OSs; run records and execution remain future work. The separate design preview remains synthetic |
| Agents and templates | Synthetic visual concepts and architecture direction | Real reusable agent records, dependency closure, templates, scheduling and qualified runners |
| Approval, telemetry and operations | Accepted core architecture; portable O1a bounded observations, O1b-1 structural graph/timeline, O1b-2a reported evaluation/approval links and O1b-2b1 reported alias/delegation inspection; outputs remain unverified with no authority or execution | Scoped gateways/rules, human and delegated-agent chains, redacted events, live graph/timelines/replay, operations console and per-operation enforcement adapters; O1–O5 gates remain open |
| Connectors and harnesses | Synthetic catalog and extension direction | Versioned implementations, permissions, compatibility fixtures, and conformance suites |
| Instruction compilation | Two immutable text-format profiles, exact-byte artifacts, deterministic candidate manifests, and native inspection with revision selection, diagnostics and evidence | Native GUI qualification, SKILL/support-file profiles, qualified target versions, and target-loader compatibility |
| Test Lab and export | Development tests, independent golden fixtures, portable instruction-bundle encoder/verifier, and native Export/Inspect commands/UI implemented in this source | User-facing static tests, native GUI qualification, target conformance and evidence bundles |
| Distribution | Three-OS source/test/build CI; selected macOS development-bundle runtime evidence | Windows/Linux GUI checks, installers, signing, updates, rollback, and release qualification |
| Team/cloud/enterprise | Architecture direction | Service implementation, tenancy, identity, operations, and deployment evidence |

The atomic composition store and native integration reached public source through [PR #12](https://github.com/hypler-dev/rangoon/pull/12) and [PR #13](https://github.com/hypler-dev/rangoon/pull/13); [PR #15](https://github.com/hypler-dev/rangoon/pull/15) added the interactive native composition editor. An isolated macOS development bundle completed Decompose, Merge, and Split with new outputs, then restart/reopen of five saved skills. Native append/mixed-destination GUI, the full failure/keyboard matrix, Windows/Linux GUI, and release qualification remain open. Exact implementation, test, and publication receipts belong in the [development ledger](docs/development.md).

## Architecture

### Current implementation

Rangoon uses a small Rust core with a Tauri 2 desktop host and a shared, static HTML/CSS/JavaScript frontend. The browser preview uses a dependency-free Node server. The native application loads bundled frontend assets and does not start that HTTP server.

```text
Bundled native frontend                         Synthetic browser preview
  Import / Skills / Workspace / Engine            sample state and interactions
                 |                                             |
          scoped Tauri IPC                           no native bridge
                 |
          apps/desktop host
       sessions / pickers / busy guards
            /             \
   rangoon-host         rangoon-store  ------ SQLite workspace
 bounded file reads    validated records / transactions
          |                  |
   rangoon-import      rangoon-compose / rangoon-compile
 deterministic spans   recipes / instruction artifacts / identities
            \              /
             rangoon-domain
          typed records / identities

   rangoon-engine: independent, unavailable LNSAT integration port
```

| Path | Responsibility |
| --- | --- |
| `crates/rangoon-domain/` | Source, span, capability, revision, and provenance record types; content identity helpers |
| `crates/rangoon-import/` | Deterministic bounded Markdown section analysis |
| `crates/rangoon-host/` | Selected-file reads, bounded native backup handling, and exclusive instruction-bundle file helpers |
| `crates/rangoon-store/` | SQLite schema validation, snapshots, revisions, local reviews, composition storage, backup/restore, and deletion |
| `crates/rangoon-compose/` | Pure transformation validation, coverage, materialization, destination application, and deterministic identities |
| `crates/rangoon-compile/` | Pure AGENTS.md/CLAUDE.md instruction artifacts, static diagnostics, candidate identities and strict portable-bundle verification |
| `crates/rangoon-workflow/` | Pure typed control/data graphs, structural diagnostics, canonical definition identities, and immutable revision candidates; no storage or execution |
| `crates/rangoon-ops/` | Closed observations, strict bounded decoding, immutable structural graph/timeline projection and exact replay identity; semantic approval relationships, authentication, I/O and authority remain unavailable |
| `crates/rangoon-engine/` | Inert `GovernancePort` and LNSAT unavailable diagnostics |
| `crates/rangoon-model-assistance/` | Pure bounded context packing and advisory-response validation; no transport, provider, credential, or authority behavior |
| `crates/rangoon-model-local/` | Explicit numeric-loopback Ollama-compatible HTTP/1 transport for prepared advisory requests; the desktop host supplies consent and calling orchestration; no credential or authority behavior |
| `crates/rangoon-model-cloud/` | Fixed-origin OpenAI Responses TLS adapter, exact request/credential-revision binding, bounded response validation and synthetic TLS fixtures; native host now supplies consent and custody orchestration |
| `crates/rangoon-model-session/` | Memory-only session profile/request custody, saved-record resolution, exact protected packing, single-use handles, lifecycle leases, and freshness checks; no network or authority behavior |
| `crates/rangoon-cli/` | Stdin source analysis and local engine diagnostics; no workspace editing CLI |
| `apps/desktop/` | Separate Cargo workspace containing the Tauri host, scoped commands, native sessions, assets, and OS icons |
| `preview/` | Synthetic product preview, native workbench views/controllers, splash, icon system, and styles |
| `fixtures/` | Secret-free source examples, contract fixtures, and independent identity vectors |
| `tests/` | Node controller, view, server, and contract tests; Rust tests also live inside individual crates |
| `.github/workflows/source-checks.yml` | Core, frontend, and native source/build checks |
| `docs/` | Accepted intent, specifications, product direction, claim evidence, and validation receipts |

### Separation of responsibilities

The renderer requests operations and displays their outcomes. It does not decide that bytes are trustworthy, compute authoritative saveability, choose arbitrary database paths, or supply replacement originals during commit. Rust resolves saved references, validates contracts, calculates the result, and owns the transaction.

The pure composition crate has no database, filesystem, process, provider, or network behavior. The store resolves records and enforces persistence rules around that core. The native host adds user selection, retained previews, bounded IPC, and lifecycle handling. The engine port remains separate from all local content operations.

The proposed evolution is a modular monolith with reusable domain/application contracts. A later self-hosted service can expose authenticated APIs and job workers without copying transformation or authority semantics into a second implementation. A frontend framework migration is not a prerequisite for this architecture.

## Import and source analysis

### Exact-byte contract

The analyzer accepts caller-supplied UTF-8 Markdown bytes and a display basename. The basename is a label, never a path for the CLI to open. The native host obtains bytes only after the user selects a file through the OS picker.

| Bound | Value |
| --- | --- |
| Source size | 262,144 bytes / 256 KiB |
| Logical lines | 10,000 LF-delimited lines |
| Content bytes per line | 16,384, excluding LF/CRLF terminators |
| Fragments | 256, including any preamble |
| Display name | Nonempty Markdown basename, at most 255 UTF-8 bytes; no path separators, colon, or control characters |
| Encoding | Valid UTF-8; optional BOM preserved; NUL rejected |

Oversized or malformed input is rejected, not silently truncated. `AGENTS.md`, `CLAUDE.md`, and `SKILL.md` receive filename classifications; a classification does not prove harness compatibility. Other valid Markdown names receive a generic classification.

### Parser scope

The parser recognizes ATX headings, preambles, and backtick/tilde fences that suppress headings inside code blocks. It preserves BOM, CRLF, Unicode, separators, and trailing text. Empty input and unclosed fences produce explicit diagnostics. Every report discloses the limited Markdown subset.

It is a section scanner, not a CommonMark parser, semantic skill extractor, instruction sanitizer, or policy evaluator. Setext headings, frontmatter interpretation, nested-container semantics, and HTML-block rules are outside the current parser. Unsupported syntax remains original text.

### Analysis records

`rangoon.source-analysis.v0` contains the analyzer version, source record, ordered fragments, diagnostics, and `authority: none`.

- `source.id` binds the display name and original bytes using versioned, length-framed SHA-256 input.
- `source.sha256` hashes the exact supplied bytes; `content`, `byteLength`, and `lineCount` describe that same input.
- Fragment spans use zero-based, end-exclusive byte offsets and one-based inclusive line coordinates.
- The fragments form a complete ordered partition: concatenating them reconstructs the original source.
- Every proposed fragment is `unreviewed`; analysis never grants review or execution authority.

The report contains original source text and is not redacted telemetry. See the [analysis contract](docs/source-analysis.md) for identity framing, supported syntax, diagnostic codes, and CLI exit behavior.

## Workspace and revision model

### Saved sources

Selection and analysis are in-memory operations. **Save locally** persists a source snapshot explicitly. Identical basename and bytes deduplicate; a changed basename or content produces another source identity. Original selected files are never rewritten.

The desktop stores data beneath its OS app-local directory at `source-workspace/workspace.sqlite3`. The renderer cannot choose that location. Listing a missing workspace does not create it. Reopening a source reanalyzes its bytes and checks stored identity, digest, and length.

### Database confidentiality and integrity

**The current desktop database and portable backups remain plaintext. Native directory creation applies restrictive Unix permissions through a platform-specific builder; warnings-denied Windows build qualification is tracked separately from GUI and real-vault support.** API credential custody does not encrypt workspace records. The optional `rangoon-store/encrypted-sqlite` feature introduces an explicit host API, `Workspace::encrypted(directory, WorkspaceKey::from_bytes(key))`, backed by the pinned Community SQLCipher 4.14.0 source bundled with `rusqlite =0.40.2`. The default build keeps the API callable but returns `encryption_unavailable` before filesystem access. `Workspace::new` remains the legacy plaintext path. Neither constructor silently switches storage modes.

The encrypted path requires the pinned cipher identity and default profile: 4096-byte pages, HMAC enabled, zero plaintext header, SHA-512 HMAC/KDF algorithms and the SQLCipher-4 iteration setting. Caller-supplied 32-byte raw keys bypass passphrase derivation; the trusted host must supply entropy and custody. Only OpenSSL/CommonCrypto provider labels are accepted; actual provider and platform qualification are recorded separately. Secrets have no debug or serialization representation, and owned key/hex buffers zeroize on final drop. This is not an erasure guarantee for caller, SQLite/parser or OS copies. SQLCipher logging is disabled before keying; temporary SQLite storage is restricted to memory.

Encrypted connections reject misaligned nonempty files. Every nonempty encrypted file passes main-file page authentication and readable-schema checks before existing record validation and transactions. A zero-length file exclusively created by the current write has no pages to authenticate and must pass readable-empty-schema checks before initialization. Wrong keys, plaintext inputs, unreadable content and failed page checks return the same closed `encrypted_store_invalid` diagnostic. Existing zero-length files are rejected; only exclusive creation by the current encrypted write may initialize. Existing file bounds, no-follow checks, journal rules, immutable identities and complete-state validation remain. Page-size reads normalize SQLCipher's text getter without changing domain/schema behavior.

The native library implements source-only database-key custody primitives in `rangoon-desktop::workspace_keys`, behind the optional desktop `encrypted-workspace` feature. Native entropy creates workspace identities and independent key/revision bytes; each workspace has its own OS slot, separate from provider credentials. Strict envelope decoding, observed-existing-slot refusal, exact post-write verification and complete retained-record comparison distinguish missing, unavailable, invalid, changed and uncertain states. Unsupported builds fail before vault access. No production command or unlock UI is mounted, and provisioning requires the later consent/exclusive-custody contract. A returned keyed handle is not database authentication or a locked-session guarantee; actual store reads still enforce the backend checks.

The native source-only layer, `rangoon-desktop::workspace_session`, owns a cooperating-file lease, a strict nonsecret workspace binding, prepared create/unlock actions and managed source operations. Creating retains the exact selected source; unlocking requires actual database authentication. Unix directory descriptors stay retained and are rechecked before vault lookup. Keys and store handles stay inside this owner API, and lock cancels pending actions and drops its key/store session. Interrupted writes remain uncertain and block same-owner retries; missing keys never regenerate. Existing trusted key primitives remain separately callable, so this does not establish library-wide ownership. Production command/callback routing, retained-data invalidation, explicit consent UI and real OS-vault qualification must precede native mounting. This packet does not switch the current plaintext workbench or migrate existing data; see its contract and source acceptance evidence in the development ledger.

The E1b-2b1 native admission foundation shares one in-memory gate between existing source/picker operations, counted snapshot/capability/workflow/data reads and retained model callbacks. Draining closes admission immediately, invalidates old lease generations and waits for the last callback owner before reporting quiescence. Reads retain their previous concurrency, capped at 64 simultaneous admissions; model flights remain independent of the single source-operation lane, while compound model freshness checks retain their existing short source-operation lease. Only capacity contention reports busy; closed or failed admission reports unavailable. Quiescence does not mean storage is locked, data is erased or a write was cancelled. No drain/lock command is mounted; cache purge, standalone credential/profile routing, renderer delivery fencing and typed encrypted-store adoption remain pending. Current GUI and portable backups still use plaintext storage.

The authentication scan may read up to 64 MiB per connection. Encryption does not authenticate a reviewer, stop replacement with an older valid database, detect every modification after a check, protect a compromised unlocked process or establish certification. Backup/export bytes remain plaintext even when sourced from a keyed store. **E1b native key custody, E1c verified migration/recovery and E1d encrypted archives must complete before the desktop can be described as encrypted.** See the [encrypted workspace contract](docs/encrypted-workspace.md) and exact source validation in the [development ledger](docs/development.md).

### Capabilities, revisions, and review

A source-backed capability begins at one saved section. Editing creates an immutable successor revision and advances the capability's current head only when the expected revision still matches. Earlier content and reviews remain available. History comparison is local content comparison, not proof of semantic equivalence.

Composition adds capabilities born from recipes and revisions produced by particular recipe applications. A capability's birth origin remains stable; each revision separately records ordinary or composition provenance. An ordinary edit after composition keeps its parent relationship without claiming that the old byte mapping describes the new text.

A local content review binds to one exact revision. The recorded reviewer is the fixed label `local_operator`, not an authenticated identity. New revisions are unreviewed and do not inherit a prior review. All these records retain `authority: none`.

### Storage versions and bounds

| Version | Records | Transition behavior |
| --- | --- | --- |
| Schema 1 | Source snapshots: original bytes and source metadata | Created only by an explicit write; analysis fragments are recomputed rather than stored as a trusted report |
| Schema 2 | Source-backed capabilities, immutable revisions, and local reviews | First capability creation upgrades schema 1 within the same transaction |
| Schema 3 | Composition recipes, applications, derived-capability owners, and revision derivations, alongside existing records | Successful composition writes or qualifying restores migrate atomically with their data |

Reads and previews do not initialize or migrate the real workspace. Unsupported schemas and unexpected structures fail without resetting or replacing the database. Older APIs reject schemas they do not understand; versioned APIs preserve source and composition distinctions.

Current limits include 128 snapshots, 128 capabilities, 32 revisions per capability, 1,024 total revisions, 128 recipes, 128 applications, provenance depth 128, and a 64 MiB database allocation ceiling. Source/revision content remains bounded to 256 KiB. These limits are implementation constraints, not a promise of unlimited enterprise scale.

See [local snapshots](docs/local-workspace.md), [reviewed capabilities](docs/reviewed-capabilities.md), and [composition destinations](docs/composition-destinations.md).

## Composition: Decompose, Merge, and Split

### Operations

| Operation | Inputs | Outputs | Initial behavior |
| --- | --- | --- | --- |
| Decompose | One complete saved source | One or more, within the output limit | Start from the analyzer's exact partition, including preamble and whitespace |
| Merge | Two to sixteen distinct pinned capability revisions | Exactly one | Preserve input order; introduced separators are explicit authored pieces |
| Split | One pinned capability revision | Two to sixteen | Partition the whole input into editable output recipes |

Inputs name exact saved IDs and digests. Historical revisions may be selected explicitly; the host never silently substitutes a newer revision. Span boundaries are half-open UTF-8 byte offsets, not JavaScript string indexes.

### Recipes and coverage

An output is an ordered sequence of copy, authored, or replacement pieces. Copy pieces retain exact source ranges. Authored pieces record new text and an explanation. Replacement pieces bind changed text to its original range and a reason. Exclusions remove no source record; they explain why a range contributes no output bytes.

The core derives coverage from actual recipes. It distinguishes copied, duplicated, replaced, excluded, and unassigned input spans. Deliberate duplication needs acknowledgment; exclusions and replacements need reasons. Declared conflicts need explicit resolution before a saveable result. Conflict declarations are manual records: Rangoon does not automatically discover every contradiction in prose or certify a resolution as safe.

The native result includes materialized output text, provenance mappings, diagnostics, and saveability. An incomplete draft can remain inspectable while persistence is blocked.

### Destinations and identity

Every output explicitly chooses one of two closed targets:

```json
{"kind":"new"}
```

```json
{"kind":"append","capabilityId":"capability:<digest>","expectedRevisionId":"revision:<digest>"}
```

Append requires the actual current head of that destination, even when an older revision is used as an input. Duplicate destinations are rejected. New destinations must be absent. All destinations are applied together; an operation cannot partially advance a set of skills.

A **recipe identity** describes transformation content and order. An **application identity** binds the recipe to ordered destinations. A composition-produced revision additionally binds its application, output index, parent, title, and content. Ordinary edits retain their separate identity algorithm. Independent golden vectors pin these encodings.

### Prepare, acknowledge, commit

The store prepares and validates a proposed post-state without modifying the actual workspace. The native host retains that preparation and returns an opaque preview handle. Commit accepts acknowledgment of that exact handle and workspace state, not a replacement recipe or renderer-calculated result.

Within an immediate transaction, the store rechecks saved inputs, destination heads, complete workspace state, materialization, quotas, and proposed records. Schema migration, recipe/application rows, new owners, output revisions, derivations, and head updates commit together. Earlier inputs, revisions, and reviews remain intact.

The receipt contains actual saved capability details in output order. A changed workspace or target requires a fresh preview and acknowledgment. An uncertain response does not trigger automatic mutation retry. Transformation acknowledgment is distinct from later per-revision content review and grants no execution authority.

See the [composition specification](docs/composition.md), [destination contract](docs/composition-destinations.md), and [native session contract](docs/composition-native.md).

### Direct composition canvas

The real Decompose/Merge/Split workbench supports dragging source sections and recipe pieces, click-to-connect ports, visible action menus, right-click actions and keyboard alternatives. These edit the same ordered recipe used by native preview and save. Undo/redo retains at most 32 entries and 4 MiB of combined serialized history; it cannot restore a previous acknowledgment or save receipt. Pending native operations lock editing, stale drag/connection state is rejected, and unsaved field edits require Apply/Cancel before structural changes. Narrow layouts provide explicit text actions instead of relying on dragging.

This graph reorganizes content; it is separate from workflow control/data flow and LNSAT authority. Synthetic browser interaction/layout checks and existing native composition evidence do not establish complete native canvas qualification. See [composition UI](docs/composition-ui.md) and the [development ledger](docs/development.md).

## Workflow definitions and revision records

`rangoon-workflow` implements the portable authoring foundation. Persistence is provided separately by `rangoon-store`, and the native host provides six authoring commands. The real `#workflows` editor uses that custody boundary; the separate design preview remains synthetic. Its closed node types are Input, Capability, Check, Branch, Checkpoint and Output. Typed data mappings are separate from control edges. Capability nodes declare exact capability/revision pairs; branches carry explicit paths, and checkpoints declare a prompt without recording approval.

`inspect_definition` strictly decodes bounded JSON, preserves decodable incomplete drafts, and reports structural errors for ports, references, topology, cycles, reachability and data availability. Canonical serialization preserves strings and array order. Only structurally valid definitions receive a `workflow-definition:` identity. The report always distinguishes structural validity from unverified references, unavailable execution and no authority.

The `records` module adds `workflow_id_from_nonce`, `prepare_revision` and `decode_revision`. A workflow ID derives from a caller-supplied 32-byte nonce; this pure module supplies no randomness or authenticated identity. Immutable revision IDs bind the workflow, nullable parent, explicit `draft` or `validated` intent, canonical definition and canonical layout using domain-separated, length-framed SHA-256. Layout changes alter the revision ID without altering the definition ID. `validated` requires structural validity, but the portable candidate cannot prove dependency existence or successful storage. Drafts preserve invalid but decodable graphs.

| Boundary | Current portable contract |
| --- | --- |
| Definition | 128 KiB; at most 128 nodes, 256 control edges, 1,024 data edges and eight inputs/outputs per node |
| Revision | 160 KiB raw/canonical record; strict schema, IDs, field types, duplicate-key rejection and identity recomputation |
| Layout | `prepare_revision` caps raw layout at 16 KiB; `decode_revision` caps canonical nested layout at 16 KiB within the 160 KiB raw record limit. At most 128 unique node-index entries; integer coordinates from -100000 through 100000 |
| Nesting | At most 32 JSON containers; malformed/resource failures return closed errors. Decodable structural faults produce validation diagnostics |
| Authority | No database, filesystem, network, provider, process, engine, run or approval behavior |

The W2b Rust store implements transactional expected-head saves, complete immutable history, pinned dependency resolution, migration, backup/restore and dependency-aware deletion. W2c-1 native Data inventory, export, restore and deletion controls are implemented in this source. W2c-2 native authoring commands are also implemented in this source; W3 library/canvas/inspector editing follows. Incomplete typed drafts remain saveable; stale or failed saves preserve current edits. The [durable workspace contract](docs/workflow-workspace.md) freezes these semantics; exact qualification and publication receipts belong in the [development ledger](docs/development.md).

The current source integrates workflow rows into the complete validated record set and schema-4 state digest. Its flat V3 archive codec preserves source, capability, composition and workflow records, validates canonical bytes and exact pins, and checks both payload limits and reconstructed SQLite allocation. V1/V2 compatibility remains; legacy V2 export rejects richer records rather than omitting them. SQLite allocation exhaustion reports `Full`; malformed archives report `BackupInvalid`, except the unchanged legacy decoder behavior documented in the contract.

The following Rust lifecycle APIs are implemented. Native authoring custody is described below; the real editor remains subsequent work:

| API | Current behavior |
| --- | --- |
| `WorkspaceBackup::decode`, `id`, `byte_length` | Strict V1/V2/V3 archive inspection with an opaque validated record set; V3 reconstruction occurs in memory. |
| `Workspace::export_workflow_backup` | Noncreating complete export: V3 for schema 4 and unchanged V2 for schemas 1–3. |
| `Workspace::prepare_workflow_restore` | Noncreating, exact-state preview with complete addition counts, kept-owner counts and a private retained plan. Validated workflow pins are checked against the final additive union. |
| `Workspace::workflow_data` / `list_workflows` / `open_workflow` | Read-only inventory and selected-history inspection APIs. Complete record rendering preserves exact definitions, parent order and separate dependency observations; display labels are bounded and sanitized. Schemas 1–3 return an empty workflow inventory; schema 4 includes complete saved workflows and histories. |

`inspect_workflow_save` now validates canonical candidates, exact parent/head relationships, dependency observations, workflow quotas and complete prospective SQLite allocation without creating a workspace. The save transaction implementation supports immutable append and precise current-head retries, preserving original timestamps and history on retries. Incomplete drafts are accepted; validated intent requires exact saved capability revisions. Public `save_workflow` performs noncreating preflight and then atomically migrates/saves complete state. Disposable disk tests exercise first save, restart, head updates, exact retries, stale/deleted-owner replay refusal and concurrent saves; transaction fixtures also cover interruption and capacity failure.

Existing version-aware source reads/`save_v1`, skill `*_v1` operations, composition preview/apply and stored compilation now use explicit complete-state routing. They validate workflow histories and preserve schema 4 and exact pinned revisions when skill heads advance. Composition stale-state checks include workflow metadata. Legacy source save, skill APIs and v1/composition data-control APIs still reject schema 4 before projecting or mutating records. Compatibility is exercised against disposable schema-4 fixtures. Native workflow authoring commands remain separate from the complete Data controls.

`restore_workflow_backup` now preflights without creating files, then rechecks the exact retained plan and complete state before inserting additions in one transaction. Empty V3 restores preserve absent, empty and existing workspaces; existing no-ops use read-only transactions. Public restore supports V1/V2/V3 data, upgrades schema monotonically when additions require it, and preserves complete local histories. Disposable disk tests exercise exact backup roundtrips, no-op bytes and retained-plan checks; transaction fixtures cover crash recovery and SQLite allocation failure.

`inspect_workflow_deletion` returns a private retained impact plan; `delete_workflow_record` revalidates its complete fields and workspace state inside an immediate transaction. Public operations handle sources, skills and workflows using complete records. Any historical draft reference to a skill owner blocks deletion, deleting a workflow removes only its history, and schema 4 never downgrades. Read-only preflight precedes write access, with full plan/state comparisons repeated inside the transaction. This is logical deletion, not secure erasure; the complete v2 native data-control adoption is specified below.

Previewing an empty V3 archive preserves an absent or existing destination's schema and creates no destination files. Complete v2 native data controls are implemented in this source. Native workflow authoring commands are implemented in this source; editor integration remains incomplete. Original images remain the layout reference for that editor. See [workflow definitions](docs/workflow-definitions.md), [workflow records](docs/workflow-records.md), and [validation evidence](docs/development.md).

### Native workflow authoring API

The desktop command layer now owns one in-memory draft context and one exact inspected save candidate. The six commands are registered only for the local main workbench; the optional model/credential windows receive no authoring permissions. This is native API source implementation, not a completed workflow editor. The Workflows design preview remains synthetic.

| Command | Inputs and behavior |
| --- | --- |
| `list_workflows` | Read complete workflow inventory without creating a workspace or draft |
| `open_workflow` | Bounded raw request selecting a current head or exact historical revision; changes no head/session |
| `begin_workflow_draft` | New native-generated identity, or append to an explicitly selected current head; issues an opaque draft handle |
| `inspect_workflow_save` | Draft handle, closed intent, raw JSON definition/layout; builds the canonical revision using native-retained identity/parent and retains the full store plan |
| `commit_workflow_save` | Preview handle, exact expected state and explicit acknowledgment; consumes one attempt before reinspection/transaction, returns only a confirmed receipt |
| `clear_workflow_draft` | Matching active draft handle; clears session state without deleting persistent records |

Open/begin/commit/clear bodies are at most 1 KiB; inspection bodies are at most 160 KiB, with the existing 128 KiB definition and 16 KiB raw layout bounds. Requests use closed versioned raw UTF-8 JSON schemas; typed envelopes reject missing/unknown/duplicate fields and invalid selectors. Nested definition/layout JSON is retained as raw values so the existing strict domain decoders still see duplicates, depth and byte limits. These checks bound decoding after Tauri receives the buffer, not transport allocation.

Preparation accepts authored definition/layout and a native draft selector. It accepts no renderer workflow/parent/revision identity, SQL, path, timestamp or store plan. The native-generated identity stays stable while the draft is repaired. A successful new draft or preview replaces the corresponding prior session; preparation failure preserves the previous native candidate. A future renderer must treat uncertain preparation as stale. Matching confirmation consumes a single attempt even when storage refuses or its response is uncertain; no automatic write retry occurs. Draft identity remains available for a fresh inspection, while renderer text, selection, viewport and history remain the upcoming W3 controller's responsibility.

Reinspection compares the full retained plan. One narrow no-write exception permits the store's exact existing-current-head candidate retry: the schema/workflow/revision and unresolved count match, the store confirms canonical byte equality, and the head equals that candidate. A changed count, different/historical candidate or deleted owner is refused. Equal unresolved counts do not prove each dependency status stayed equal; a no-op receipt reports fresh statuses. The store transaction revalidates complete records and exact candidate/state relationships before any write.

Incomplete decodable definitions may be saved as drafts; validated intent requires structural validity and exact available local capability revisions. Neither intent records review, approval, execution or authority. Source/session tests are separate from native IPC/editor and all-three-OS GUI qualification. See [native authoring](docs/workflow-native-authoring.md) and the [development ledger](docs/development.md) for exact scope, checks and open gates.

### Real workflow editor

The `#workflows` area connects native authoring custody to a saved library, positioned graph and right inspector. Its six node types expose typed `text`, `json` and `boolean` ports. Solid directed wires carry `next`, `true` and `false` control outlets; dashed directed wires carry separately labeled data mappings. Library-card and port drag/drop, click-to-connect, Outline forms and contextual keyboard actions share the same controller. Zoom and pan change the transient viewport; node positions belong to the persisted revision. Outline is the initial representation on a narrow first visit. The Workflows header uses distinct existing Rangoon technical artwork and retains the shared themed shell and sidebar mascot.

New and append sessions use native-issued identities. Exact historical capability revisions are selected from actual local inventories, rather than typed IDs or silently substituted heads. Field buffers belong to one controller target and must be applied or cancelled before structural changes or native inspection. Local undo/redo is bounded to fifty actions. Saved workflow revisions open read-only; editing requires an explicit append at the selected current head. History comparison reads two exact revision IDs without advancing either head.

Draft inspection preserves decodable incomplete graphs. Validated inspection requires a fresh native Draft report for the unchanged definition and layout, structural validity and zero unresolved references; it still performs a new native inspection. Closed response validation binds reports, dependency pairs, history, candidate identity and full definition/layout relationships. It rejects malformed or substituted success envelopes instead of fabricating observations. Explicit acknowledgment binds the exact inspected candidate. Confirmation consumes its renderer preview before dispatching one native save; uncertain replies retain local edits and require fresh inspection. A confirmed receipt survives a failed library refresh. Workspace writes invalidate workflow candidates and other workbenches' retained compilation/model/composition observations as appropriate.

The [editor contract](docs/workflow-editor.md) defines these behaviors and the [development ledger](docs/development.md) records source, synthetic browser, review and publication evidence separately. Synthetic bridge checks exercise the production controller and binder; they do not qualify native IPC, saved restart behavior, Windows/Linux GUI, installers or released OS support. Native validation remains authoritative. Workflow authoring does not execute these nodes, activate LNSAT, connect providers or record execution approval. The historical Workflows design preview remains synthetic.

## Instruction compilation

The R4a library compiles one exact capability revision into an inspectable instruction artifact. `Workspace::compile_capability(capability_id, revision_id, profile)` resolves the content, local review and composition lineage through the existing validated store; it accepts IDs rather than renderer-supplied content or a review flag. A reviewed historical revision remains selectable, with the observed current head reported separately. Reads do not create a workspace, migrate its schema, mark a revision reviewed or write an artifact.

| Profile | Fixed output name | Behavior |
| --- | --- | --- |
| `agents_md_v1` | `AGENTS.md` | Preserve the selected revision's exact UTF-8 bytes |
| `claude_md_v1` | `CLAUDE.md` | Preserve exact bytes; conservatively block candidate readiness for ASCII `@` or `<!--` anywhere |

The Claude checks deliberately include false positives such as email addresses and code examples. They prevent presenting possibly imported or omitted content as a ready candidate under this narrow profile. They never follow includes, rewrite content, remove comments or authorize an execution. Structured requirements supplied to the pure API are retained as unsupported diagnostics and block candidates. Stored capabilities currently contain opaque Markdown rather than structured requirements, so compilation always warns that semantic equivalence, dependency closure and enforcement are unverified.

Every report carries the exact artifact text, byte digest/length, selected revision/provenance references, immutable profile descriptor/digest and ordered diagnostics. Artifact text remains inspectable when readiness is blocked. Readiness has three values:

- `blocked`: a static compatibility error exists; there is no candidate manifest.
- `review_required`: static checks have no blocking error but the selected revision lacks local content review.
- `candidate`: the selected revision has local review and the narrow static checks allow an in-memory candidate manifest. This grants no execution or installation authority.

The compiler preserves BOM, CRLF/LF, Unicode, whitespace and final-newline state. Its domain-separated compilation identity binds profile, revision, artifact metadata and diagnostics. Review is separate: adding or changing a valid review observation leaves compilation identity unchanged, while the candidate manifest and identity bind that exact observation. The manifest uses a versioned compact JSON encoding; its SHA-256 covers its canonical bytes without a recursive self-hash. Digests establish consistency, not authentication or tamperproof local storage.

Profile versions describe Rangoon's format contract. Both profiles explicitly report runtime qualification `untested`, target budget `unknown`, semantic equivalence `unverified`, and `authority: none`. A generated file may still be ignored, truncated, combined with other instructions, or interpreted differently by a real harness. No external CLI version is advertised as qualified. The 256 KiB content limit is Rangoon's input bound, not a guarantee about available target context.

The pure compiler has no filesystem, process, environment, provider or engine access. The native Compile page calls a narrow `compile_capability` command with exact capability/revision IDs and a closed profile. Its raw UTF-8 JSON request is bounded to 4,096 bytes and rejects unknown/duplicate fields; source content, local review and provenance are resolved by the host. The command shares the workspace operation guard and never changes the Import session or saved records.

The inspection workbench keeps Generated text, Diagnostics and Evidence separate. It supports historical revisions, preserves unsaved Skills drafts, rejects mismatched native responses, and marks retained reports stale after known workspace writes. Refresh resolves the selected revision again; compilation runs only on explicit action. Candidate manifest text comes from the host's canonical serializer, with renderer manifest/hash consistency checks. Browser-only use shows the unavailable native bridge. Displaying text is not export or target qualification; no Copy, Install, Run or authority action is exposed.

The bundle foundation has a pure encoder and strict in-memory verifier plus bounded host helpers for an explicitly caller-selected path. It preserves exact artifact bytes and canonical manifest bytes, records component lengths, and appends a SHA-256 checksum of the preceding bytes; inspection separately reports the whole-file digest. Maximum total length is 295,016 bytes. Inspection reports `verification: internal_consistency_only` and `authority: none`. Digests detect inconsistency, not authentication, complete source-graph lineage, or target behavior. Native Export and external bundle inspection commands/UI are implemented in this source: requests are ID-only raw UTF-8 JSON bounded to 4,096 bytes; export re-resolves and recompiles after the picker, binds the fresh candidate ID, and exclusively creates a new plaintext file without overwrite; write/sync uncertainty is distinct from known no-output failure. External inspection never opens the workspace, imports, reviews or executes; it returns internal-consistency evidence with `authority: none`. The isolated macOS QA bundle exercised AGENTS export cancellation, successful exports for both profiles, and external Inspect cancellation, golden success from no-workspace state, own-export success, and invalid-file failure. Native GUI qualification remains open for collision/write uncertainty, full keyboard coverage, Windows/Linux, and target loaders; publication evidence belongs in the [development ledger](docs/development.md).

The CLI remains an analyzer and inert engine-status tool, with no compilation command. SKILL metadata/support-file formats, target drift/collision handling, native GUI qualification and all-three-OS target-loader evidence remain required R4/R5 work. See the [compiler contract](docs/compilation.md), [bundle contract](docs/instruction-bundle.md), [native inspection contract](docs/compilation-ui.md), [native bundle UI contract](docs/instruction-bundle-ui.md), and [model assistance contract](docs/model-assistance.md) for exact schemas, identity framing, state handling and acceptance boundaries. M1a model assistance is implemented as a pure crate; local and cloud capability remain separate V1 deliverables, and use remains optional and explicit.

## Model assistance

The implemented M1a core in [`rangoon-model-assistance`](crates/rangoon-model-assistance/) has two bounded entry points:

```rust
prepare_pack(raw_json: &[u8]) -> Result<ContextPack, Diagnostic>
validate_response(pack: &ContextPack, raw_json: &[u8]) -> Result<ValidatedResponse, Diagnostic>
```

It supports three closed tasks: `classify_v1`, `decompose_v1`, and `compare_v1`. A request carries already resolved source or capability revisions, exact selected byte ranges, explicit protected ranges, task/profile limits, and a requested output bound. Packing preserves input order and exact bytes, reports selected/protected/omitted coverage, and reuses only byte-identical complete blocks. Every input/range keeps an alias to the transmitted block, so deduplication does not merge provenance, scope, or authority. `selectedBytes`, `uniqueTextBytes`, and `omittedBytes` are byte measurements; `tokenAccounting` remains `unknown`, with no claim about token savings or compression quality.

The packer uses fixed checked-in task templates and bounded canonical JSON. The response decoder is closed and bounded before typed decoding; it requires the task-specific proposal kind, citations wholly inside selected ranges, and bounded uncertainty entries. `authoredText` remains inert model-authored text. Successful validation carries `authority: none`; it does not review, apply, execute, or mutate content. The pure core cannot authenticate database existence, revision ancestry, provider/profile identity, or operator consent, and it cannot authorize sending a payload. The separate [`rangoon-model-local`](crates/rangoon-model-local/) library provides a source-only M1b transport seam: `LocalProfile::parse`, `PreparedRequest::new`, and explicit `LocalClient::check`/`analyze` use direct Tokio TCP and Hyper HTTP/1 only for a caller-invoked numeric loopback target (`127.0.0.1` or `::1`) and prepared payload. It uses no proxy, redirect, retry, fallback, credential, model launch, or background traffic; the loopback peer may forward or retain data. The native host supplies command, review, consent, cancellation and freshness paths for local and cloud routes. The cloud route is source-implemented with fixed-origin TLS and full credential custody; M1b/V1 remain incomplete pending provider compatibility, proposal application, tokenizer/quality qualification, native GUI and three-OS release evidence.

The [`rangoon-model-session`](crates/rangoon-model-session/) crate supplies portable session custody around those libraries, and `apps/desktop/src/models.rs` wires it into the native host. `LocalSession` keeps one optional profile, one prepared request and at most one active operation in memory; `configure`, `inspect`, `clear`, `cancel`, `begin_prepare`, `begin_check` and `begin_send` enforce session lifecycle. Preparation resolves exact saved source or capability-revision IDs, packs each nonempty record as one protected full-content selection, rejects empty records, and exposes immutable request metadata. Random single-use `prepared:` and `run:` handles, checked generations, cancellation and non-cloneable RAII leases prevent stale or replayed ownership. `Transmission::freshness` is a read-before/read-after helper for native orchestration. The library performs no network calls or authority actions; the native host owns the seven main commands, three review commands, isolated exact-payload window, parented OS dialog, single-use consent, cancellation and freshness checks. See the [native session contract](docs/model-native-session.md) and [native workbench contract](docs/model-native-workbench.md).

The local contract binds an exact immutable profile and final request payload, permits one in-flight operation process-wide, and fixes 3-second connect, 5-second version-check, and 120-second analysis deadlines. It caps the final request at 256 KiB, version responses at 1 KiB, chat responses at 1 MiB, and decoded message content at 128 KiB. Cancellation is monotonic and releases the guard; response parsing and proposal validation remain strict and advisory.

### Attributed proposal inspection

The pure `inspect_proposal` API checks a validated response against its exact context pack and selected proposal index, then exposes bounded proposal text and citation evidence for comparison. It performs no freshness check, destination selection, network request or mutation. Native completed-result retention and durable attributed application remain unfinished; routing model text through an ordinary skill edit would lose required model/task/pack/response provenance. See [proposal inspection](docs/model-proposal-inspection.md).

### Cloud session custody

The portable [`rangoon-model-session::cloud`](crates/rangoon-model-session/src/cloud.rs) module adds `CloudSession`, with a cloud-only profile, immutable staged request and one active operation per shared session. It reads the same validated saved records as the local path, preserves their full contents as protected ranges, and constructs the actual cloud adapter request. The caller supplies only a nonsecret credential revision; this module never opens the OS credential store.

Closed `rangoon.cloud-selection.v1`, `rangoon.cloud-send.v1` and `rangoon.cloud-cancel.v1` inputs have independent schema tags and random `cloud-prepared:`/`cloud-run:` handle namespaces. A prepared view exposes the exact outer body, byte length, SHA-256, model, fixed origin, credential revision, dependencies and separate inner context pack. Changed credential revisions change request identity. A successful claim consumes the handle once; profile replacement, clear, cancellation and a new preparation invalidate obsolete work. Read-only freshness checks compare exact content, dependency identity and observed capability heads.

Portable custody remains network-free and authority-free. Native `apps/desktop/src/cloud_models.rs` now calls the adapter through explicit capability-configured commands, pairs full credential envelopes to retained preparations, holds shared model/credential leases, performs source-free GET checks, requires final OS consent, and applies pre/post freshness gates. The module and native results grant no execution or LNSAT authority; SQLite remains plaintext. Provider compatibility and full cross-platform qualification remain open. See the [cloud session contract](docs/model-cloud-session.md) and [native cloud contract](docs/model-cloud-native.md).

## Backup, restore, and deletion

The native Workspace surface provides real inventory, export through a save picker, restore through a file picker, and dependency-aware deletion. These controls exist now; they are not merely roadmap items.

### Portable backup

Native export uses the complete version-aware store: V2 for schemas 1–3 and V3 for schema 4. V3 includes workflow owners, exact canonical revisions, node positions, parent chains and saved timestamps alongside source bytes, skill histories, reviews, recipes, applications and live provenance. V1 and V2 backups retain their original interpretation and remain accepted by the versioned restore path. Unsaved editor drafts and transient native preview handles are not backup records.

V2 and V3 use versioned magic headers, bounded canonical JSON manifests, ordered raw payload segments, and SHA-256 trailers. Limits are 2 MiB for the manifest, 68 MiB for the complete archive, and 64 MiB for aggregate raw payload. Reconstructed SQLite allocation must independently fit the database ceiling. The archive is plaintext; its digest is not a signature or encryption.

### Additive restore

Restore computes the final union before mutation. Existing local records remain authoritative for membership and retained history. Absent capabilities may be imported with their complete histories; restore does not append missing historical revisions to an already-present capability or replace its head. Same-ID identity-bearing data must agree. Existing workflow owners retain their complete local history and head. Incoming workflow history must be an exact prefix including canonical bytes and timestamps; newer, divergent or retimed incoming histories are refused rather than silently discarded. An archive containing only workflows can be restored.

Missing pinned dependencies block dependent imports. The host retains the selected decoded archive and exact plan. Confirmation binds backup identity, current workspace state, and the proposed operation. State drift rejects the restore rather than applying an old plan. Migration and additions commit atomically. A matching native confirmation takes the retained candidate before entering storage; success, refusal or uncertain return consumes that attempt. Fresh preview is required for another attempt, and no write is automatically retried.

### Dependency-aware deletion

Deleting a capability removes its owned revisions, reviews, and derivation membership. A surviving capability that depends on one of those revisions blocks deletion. Any syntactically valid reference to that skill owner from any saved workflow revision also blocks deletion, including historical draft references with missing or malformed revision IDs. The impact preview lists skill and workflow blockers separately. Source deletion is likewise blocked by surviving source origins or recipe inputs. Orphaned application/recipe metadata can be removed as specified; unrelated inputs and other skills are not implicitly deleted.

Immutable recipe targets may describe outputs that were later deleted. Readback and restore use live membership and do not recreate those outputs merely because the historical application names them. A fresh, explicit composition can deliberately recreate a deterministic new destination; there are no permanent deletion tombstones.

Deleting a workflow removes its owner and complete immutable history only. It does not remove source files, input skills, unrelated workflows or lower schema 4. Native confirmation selects the exact private retained plan and consumes one attempt before storage revalidation.

### Complete native inventory and recovery DTOs

The Data route and existing six native commands now adopt `rangoon.workspace-data.v2`, `rangoon.restore-plan.v2` and `rangoon.deletion-plan.v2` together. Source, skill and workflow owners remain separate; capability revision and workflow revision totals are never combined. The renderer rejects older/unknown shapes, contradictory validated-intent observations and impossible counts. It renders escaped bounded labels, local dependency observations and explicit Inspect/Delete actions without implying a workflow editor or execution exists. A workflow-only restore is actionable; a no-additions preview checks every record count. Native commands retain opaque archives/private plans and accept selectors rather than renderer-supplied impact or SQL.

This source adoption can restore schema-4 archives through explicit native selection and confirmation. It registers no workflow creation/save command. Existing local analysis, skills and composition drafts stay usable; workflow deletion is never treated as source deletion. Native session and synthetic renderer evidence are distinct from interactive OS picker/IPC qualification. See [native controls](docs/workflow-native-controls.md) and the [development ledger](docs/development.md) for exact checks and remaining GUI gates.

Deletion is logical removal, not secure erasure. See [workspace data controls](docs/workspace-data-controls.md) and [composition recovery](docs/composition-recovery.md) for exact archive encoding, dependency closure, stale-state checks, and failure tests.

## Native application boundary

The workflow builder has browser-native drag previews, attached control/data wires and an explicitly synthetic in-memory playground at `preview/workflow-playground.html`. Native persistence and authority remain separate. The CLAUDE.md profile displays an official locally bundled Claude mark as format identification only; AGENTS.md remains provider-independent. No animation framework, CDN or runtime asset download is introduced. The current UI packet passes 213 frontend tests and fresh independent review; rendered checks cover drag, undo/redo, zoomed wires, keyboard tabs/context menus, and dark/light layouts at 320, 768 and 1440 pixels. Cancellation and reduced-motion rules have source/fixture coverage, with live assistive-technology and reduced-motion emulation qualification still open. Exact evidence is tracked in the [development ledger](docs/development.md); this local source packet is not a released feature.

The OS application is named **Rangoon** and uses the selected logo without added text. The startup experience may display the Rangoon.ai wordmark. Startup resolves to usable application state or a visible failure; it does not invent progress or engine readiness. The visual system includes custom Foldline SVG icons, dark/light tokens, visible keyboard focus, and reduced-motion behavior.

Only the bundled main window receives the enumerated application commands. Native code opens its own pickers; the renderer receives no generic shell, arbitrary filesystem, SQL, or external HTTP capability. Bundled navigation restrictions and a native CSP complement command permissions.

| Command family | Examples | Boundary |
| --- | --- | --- |
| Source | `select_and_analyze`, `save_analysis`, `list_snapshots`, `open_snapshot`, `clear_analysis` | Native selection and retained reports; explicit snapshot save |
| Skills | `list_capabilities`, `open_capability`, `create_capability`, `revise_capability`, `review_capability` | Saved references, expected heads, immutable revisions, local review |
| Workspace | `get_workspace_data`, `export_workspace_backup`, `prepare_workspace_restore`, `restore_workspace_backup`, `inspect_workspace_deletion`, `delete_workspace_record` | Native pickers, retained plans, exact confirmations |
| Composition | `read_composition_source`, `preview_composition`, `commit_composition` | Saved input resolution without replacing Import selection, and one retained preparation |
| Compilation | `compile_capability` | Closed raw request, exact saved revision/profile, read-only report and canonical manifest inspection |
| Engine | `get_engine_status` | No-argument local unavailable diagnostic |
| Model session and workbench | `get_local_model_profile`, `configure_local_model`, `clear_local_model`, `prepare_local_model`, `check_local_model`, `send_local_model`, `cancel_local_model`; isolated `get_local_model_review`, `confirm_local_model_review`, `cancel_local_model_review` | Session-only numeric-loopback profile, exact protected records, native review/OS consent, cancellation and freshness; advisory output only |
| Cloud model session and workbench | `get_cloud_model_profile`, `configure_cloud_model`, `clear_cloud_model`, `prepare_cloud_model`, `check_cloud_model`, `send_cloud_model`, `cancel_cloud_model`; isolated cloud review commands | Explicit cloud profile/capability config, retained request-envelope pairing, shared leases, fixed-origin TLS, source-free GET, final OS consent, cancellation and freshness; advisory output only |

Composition commands specifically use raw UTF-8 JSON IPC bytes. Preview is bounded to 8 MiB + 16 KiB; confirmation to 4,096 bytes before domain deserialization. Tauri has already allocated the transport buffer, so this is not a transport-level allocation guarantee. Closed DTOs reject unknown/duplicate fields and unsupported schemas.

The host retains at most one composition preview. A fresh successful preview receives a 32-byte OS-random handle and replaces the previous slot. Confirmation marks the handle attempted before entering storage; it cannot be used for another attempt after failure or a lost response. Verified success consumes the slot. Restart invalidates all such handles. These are local selection handles, not authentication credentials.

## Local database security

### Protections implemented today

The store uses bundled SQLite, validated schema/record shapes, bounded transactions, fixed public errors, and unkeyed identity checks. Reopening source bytes and reconstructing composition provenance detects inconsistent records rather than silently accepting them.

New Unix workspace directories/files use owner-only permissions. Windows inherits the per-user application directory ACL; Unix mode behavior is not claimed for Windows. Known symlinks/reparse points and non-regular database/journal files are rejected. Parent directories are not held against a hostile concurrent local user.

SQLite connections disable trusted schema, use rollback-journal `DELETE` mode and `synchronous=FULL`, and apply allocation/value limits and a bounded lock timeout. Transaction and process-interruption tests are useful evidence; they do not prove power-loss durability on every device or filesystem.

### Limits of the present design

**The database and exported backups are unencrypted.** A party with sufficient filesystem access can read them. A writer able to replace the store can recompute its unkeyed hashes; internal consistency does not establish authentic authorship, an untampered history, or freshness after rollback. Local review labels and wall-clock timestamps are unauthenticated.

Provider requests require the explicit configured model path, retained payload review and native consent described above. There are no automatic background source uploads. Analysis output and backups contain source content, so publishing those artifacts can disclose that content. Logical deletion cannot promise removal from backups, filesystem snapshots, or storage media.

### Proposed hardening sequence

1. Define the threat model: accidental corruption, offline theft, another local account, malicious same-user software, rollback, and a compromised unlocked process require different controls.
2. Evaluate an encrypted SQLite backend and encrypted backup format with OS-protected key custody on all three platforms. Require explicit key-loss recovery, rotation, migration, and no silent plaintext fallback.
3. Add authenticated history that binds operations, identities, sequence, and prior state using protected key material. Keep consistency hashes for content addressing; do not relabel them as authentication.
4. Design independently stored checkpoints for rollback detection. Stronger enterprise evidence may require an external witness; restoring all local state together cannot be made reliably fresh by another file beside the database.
5. Fail closed on integrity failures, preserve recoverable evidence, and offer explicit recovery rather than silently creating a new empty store.
6. Test altered pages/records, replayed backups, interrupted key rotation, lost keys, disk exhaustion, migration failure, and recovery on macOS, Windows, and Linux.

These are proposed controls, not implemented guarantees. A compromised administrator or unlocked application process can exceed the protection of local encryption. Nothing in this repository establishes production readiness, security certification, government compliance, domestic processing, or government endorsement.

## Approval, telemetry and live operations

Approval management, telemetry and dynamic operations visualization are core product scope. The shared flow is **request → policy evaluation → required human/delegated-agent approval → scoped authorization → execution-time checks → execution → outcome evidence**. Rangoon owns configuration, routing, visibility and evidence presentation; LNSAT and other qualified authority adapters enforce their actual contracts. Approval alone cannot bypass consumption-time checks.

Planned operations surfaces include scoped gateway/rule configuration, approval chains, mandatory human duties, explicitly bounded agent delegation, expiry/escalation/revocation/emergency suspension, an approval inbox, agent inventory and authority assignments. Agents cannot grant themselves broader authority. Telemetry must redact sensitive payloads and correlate request/attempt/trace IDs, exact policy versions, approvals, resource access, costs, latency, outcomes and failures. A live graph and timeline connect agents, workflows, tools, resources, gateways, approvers and engines, with filters, exact evidence drill-down and inert historical replay.

These are accepted product requirements, **not implemented runtime features**. Shared closed records, deterministic projections and adapter conformance precede activation. Per-operation coverage must distinguish unavailable, advisory, partially mediated and fully mediated paths; “universal” never means blanket enforcement. Current engine transport remains unavailable, workspace SQLite remains plaintext, and telemetry is not an authenticated audit log. See the [operations control-plane contract](docs/operations-control-plane.md) for responsibility boundaries, state machines, privacy, failure handling and O1–O5 acceptance gates; exact source/runtime evidence belongs in the [development ledger](docs/development.md).

### Portable observation foundation (O1a)

`rangoon-ops` implements the first operations source stage. `decode_window(&[u8])` returns an immutable `DecodedWindow` with canonical bytes, their SHA-256 digest and a read-only serializable inventory. Its input schema is `rangoon.ops.window.v1`; the inventory is `rangoon.ops.inventory.v1`. Snapshot metadata pins the reducer version, workspace/system/environment/engine scope, topology revision, policy/coverage revisions and per-producer prefix/watermark/fence inputs. A bounded tail retains subsequent events. Supplied prefixes are observations, not authenticated checkpoints.

The nineteen closed record kinds cover five connected concerns:

| Concern | Observation records |
| --- | --- |
| Systems and topology | Actor, resource, tool, workflow and engine |
| Configuration and delegated scope | Policy revision, gateway revision, assignment and delegation |
| Exact request and review | Request, immutable initial/successor evaluation, approval plan and approval decision |
| Reported execution lifecycle | Authorization, attempt and suspension |
| Visibility and provenance | Metric, evidence and per-operation coverage |

Every action/configuration event carries typed correlation selectors; only the five context-free inventory kinds may have null event correlation. Evaluation records retain the exact input/context snapshots and digests, expected effective predecessor and lineage fence. Decisions bind their evaluation, plan, stage, actor and delegation. Authorizations and attempts remain separate reported observations. Actor/delegation source verification, target-OS coverage and freshness are explicitly reported values. Neither `reported_verified` nor `fully_mediated` changes the decoder's fixed **authority `none`, authenticity `unverified`, execution `unavailable`** output.

The decoder rejects duplicate object keys (including equal values), unknown fields and variants, absent required nullable fields, malformed IDs/digests/tokens, unsafe integers and trailing content. Input and canonical windows are capped at 2 MiB each; structural limits are depth 32, 32,768 JSON values, 64 fields per object, 1,024 raw array entries, 32 distinct producers, 1,024 total events, 128 topology nodes, 512 edges and 32 entries in other lists. Explicit null costs and latency remain unknown rather than zero. Array order and string values are preserved; serialization normalizes object field order. All failures expose fixed codes without parser prose or input excerpts.

This foundation validates shape and resource bounds. It deliberately preserves duplicate event identities/sequences, broken references and contradictory lifecycle observations for O1b diagnostics. A content digest proves internal consistency only. Token syntax cannot detect disguised secrets: trusted future ingress must authenticate sources, mint application IDs and classify/redact external references before decoding. No telemetry collector, retention database, approval routing, grant issuance, connection or execution is introduced.

See the exact [O1a records and decoding contract](docs/operations-records.md) and [development evidence](docs/development.md). O1b is staged into the structural projection below and later semantic relationship assessment; a clean structural projection never certifies an approval trace.

### Structural timeline and declared graph (O1b-1)

`rangoon-ops` implements `project_window(&DecodedWindow)` as a pure read-only step over the immutable O1a window under the independently reviewed [projection contract](docs/operations-projection.md). It returns one timeline entry for every observation, including duplicates and conflicts, and a graph containing only declared topology. Exact input ordinals and prefix/tail locations address original records; graph and accessible table can share this result in O2 instead of implementing separate decision logic.

Replay identity binds the complete input digest, snapshot/reducer/scope, topology and policy/coverage pins, and sorted producer-prefix fences/watermarks. Timeline presentation sorts by producer, fence, sequence, event ID and input ordinal. Unknown/tied timestamps remain display data; they do not select authority or causal precedence. A late arrival creates a new exact input identity and leaves a previous projection unchanged.

Eighteen fixed structural diagnostics cover duplicate/contradictory identities, sequence gaps, missing/wrong/cross-fence predecessors, prefix fences/watermarks, scope and event-envelope disagreement, and missing/ambiguous declared graph references. Counts include every affected entry; observations never disappear to make a trace look clean. The output is bounded to **1 MiB** using a capped writer; amplification beyond that cap fails atomically with `projection_limit`. Input remains intact, and no partial graph is returned.

This stage fixes **authority `none`, authenticity `unverified`, execution `unavailable`, semantic assessment `unavailable`, authoritative eligibility `unknown`, and authoritative effective evaluation `null`**. Its `structural_unverified`, `incomplete` and `conflict` states cover only the named structural checks. Reported full coverage, delegated identity verification, issued authorization or completed execution cannot change those trust fields.

Structural v1 remains frozen. The separate relationship schema below assesses reported links without changing those bytes or trust fields.

### Reported request, evaluation and approval links (O1b-2a)

`relate_window(&DecodedWindow)` first projects the exact immutable window, then returns a separate `rangoon.ops.relationships.v1` document under the [relationship contract](docs/operations-relationships.md). It retains one link row per event and all matching observation ordinals for referenced records. Closed reference checks distinguish absent, ambiguous, wrong-kind and byte-identical repeated records. Flow binding compares workspace, system, environment, engine, request, trace and policy revision; each step keeps its own span ID.

Reported evaluation chains require one root, exact previous/expected selectors, a single successor at each step, matching declared lineage fences and producer streams, and increasing stream sequences. Missing history, branches, cycles, identity conflict, scope/envelope mismatch or inconsistent request/gateway/context pins produce `unknown_or_conflict` with null tip and result. Timestamps never select a winner. Late approvals bound to an older evaluation remain historical; they do not migrate to the successor or override a reported deny.

Plan/decision/authorization links check exact evaluation, plan and stage bindings. This stage does **not** establish approver eligibility, satisfy human duties, authenticate delegation, assess quorum/expiry/revocation/suspension, consume authorization or reconcile effects. Its reported chain tip is an unverified observation selector. Authoritative eligibility remains `unknown`, authoritative effective evaluation remains `null`, and authority/authenticity/execution remain `none`/`unverified`/`unavailable`.

The separate output is capped at **1 MiB**, with at most **8,192 reference rows** and **8,192 combined matching ordinals** across references and chain requests. Work budgets are checked before adding rows or copies; cap failures return fixed, payload-free errors atomically. Structural projection overflow maps to `relationship_structural_limit`; relationship work or output overflow maps to `relationship_limit`. No partial output or hidden dropped observations are returned.

O1b-2b is staged into the reported identity/delegation inspection below (2b1), human/stage/quorum/decision duties (2b2), and lifecycle/outcome/coverage assessment (2b3). Current expiry, revocation, suspension and execution eligibility require later qualified authority work. O2 builds real read-only operations routes; O3 qualifies ingress/privacy/retention; O4 qualifies one complete approval/authorization/consumption/evidence path; O5 expands adapters and hosted operation. These source stages supply no live collector, approval router, authority engine, execution service or authenticated audit log. Full O1b, O1 and V1 remain incomplete.

### Reported actor aliases and delegation inspection (O1b-2b1)

`inspect_delegations(&DecodedWindow)` uses the exact immutable window and its structural projection to produce a separate `rangoon.ops.delegations.v1` document under the [delegation inspection contract](docs/operations-delegations.md). Subject groups use the reported `(systemId, canonicalSubjectId)` tuple: aliases within one system share diagnostics; identical subject IDs from different systems remain separate. Every actor occurrence, conflicting record variant and unassessed event remains inspectable. This does not authenticate identity or infer aliases from names or external references.

Explicit delegation links inspect uniquely resolved actors, finite environment/operation/resource/policy scope sets, parent subject binding and reported expiry attenuation. Alias-aware checks flag a delegate granting to itself or changing its own delegation even when different actor IDs report the same subject. Missing or conflicting actor references retain null resolved tuples and their diagnostics; null comparisons never become proof of self-delegation or a parent mismatch. Empty scope dimensions are incomplete, never wildcards. Reported expiry values are compared only against declared parents, without a clock or current-validity claim.

An iterative leaf-to-root path retains missing, ambiguous and wrong-kind parent IDs, closing cycle IDs and ancestor diagnostics. All byte-identical observations contribute diagnostics, so a clean repeat cannot hide a tainted observation. Independent grants, assignments and gateway admission are outside these explicit parent checks. Parent links never admit subdelegation. Human-required stages, approver eligibility, quorum, approval succession, current expiry/revocation/suspension, outcome reconciliation and operation-specific enforcement coverage remain subsequent stages.

The immutable output fixes **authority `none`, authenticity `unverified`, execution `unavailable`, authoritative eligibility `unknown`, authoritative effective evaluation `null`, and subdelegation admission `unavailable`**. Assessments are `linked_unverified`, `incomplete`, `conflict` or `not_assessed`; reported active/verified/current labels cannot promote them. Output is limited to **1 MiB** with independent **8,192-reference**, **8,192-matching-ordinal** and **8,192-ancestry-step** budgets checked before growth. A path ID counts once, including the own ID and closing/dangling step. Each record observation is compared with its ID’s first typed record once; reference and ancestry lookups use an invocation-local resolution index, avoiding repeated byte scans. The index is discarded before return. Overflow fails atomically with a fixed payload-free diagnostic. Existing structural and relationship v1 schemas remain unchanged. Exact source, fixture, independent review and publication evidence belongs in the [development ledger](docs/development.md); no live operations or authenticated delegation capability is established by this source stage.

## LNSAT integration

Rangoon is the user-facing product. **LNSAT remains an independent authority and evidence engine.** A future model provider can propose content; LNSAT qualification governs a different concern: whether an exact consequential operation is permitted and what evidence establishes its outcome. Neither role is implemented by changing a UI status badge.

The current `GovernancePort` has five closed operations:

| Operation | Intended future role | Current result |
| --- | --- | --- |
| `negotiate_contract` | Verify exact peer contract and supported surface | Unavailable |
| `inspect_configuration` | Read authorized diagnostic configuration | Unavailable |
| `read_evidence` | Retrieve exact authorized evidence | Unavailable |
| `submit_operation` | Submit a governed operation | Unavailable |
| `reconcile_operation` | Resolve an uncertain prior effect | Unavailable |

The placeholder performs no discovery, launch, filesystem access, network transport, credential handling, mutation, or execution. Status reports `placeholder`, `not_attempted`, `not_checked`, null installed/observed versions, and no authority. `engine status` exits 0 for successful local diagnostics; `engine check` returns unavailable and exits 4.

The [integration specification](docs/engine-integration.md) records an immutable public LNSAT research reference. Its source version, wire contract, product surface, storage schema, and eventual release support are distinct dimensions. A wire label containing `v1_0` does not establish a supported LNSAT product 1.0. Rangoon never opens the LNSAT database. The model-assistance core is separate and advisory: it cannot authorize transport, execution, review, or mutation.

Adapter work requires a supported artifact, exact typed request/result contracts, peer authentication, secret custody, compatibility tests, and explicit unknown-outcome handling. Read-only evidence qualification precedes consequential operations. Lost responses must not cause blind write retries.

## Run from source

### Requirements

- Node.js 20+ for the local preview and Node tests; no npm dependency installation is required.
- Rust 1.85+ for the edition-2024 core workspace. CI pins its minimum toolchain explicitly.
- Rust 1.98 and host-native prerequisites for the separate desktop workspace: Xcode command-line tools on macOS, Microsoft C++ tools/WebView2 on Windows, or the documented WebKitGTK dependencies on Linux. See [desktop setup](docs/desktop-spike.md#run-from-source).

```sh
git clone https://github.com/hypler-dev/rangoon.git
cd rangoon
```

### Synthetic design preview

```sh
npm start
```

Open `http://127.0.0.1:4377`. The splash preview is `/splash.html`, and the icon specimen is `/icon-gallery.html`. The normal browser cannot call the native bridge. Reloading resets the sample workspace; theme preference may remain locally. Synthetic controls do not operate real providers, deployments, or approvals.

### Deterministic CLI

```sh
cargo run --locked -p rangoon-cli -- --help
cargo run --locked -p rangoon-cli -- analyze --name AGENTS.md < fixtures/contracts/AGENTS.md
cargo run --locked -p rangoon-cli -- engine status
cargo run --locked -p rangoon-cli -- engine check --operation read_evidence
```

The last command deliberately exits 4. The analyzer writes JSON to stdout; invalid input/usage exits 2 with fixed JSON errors on stderr; I/O/serialization failures exit 3. A successful report includes original text.

On Windows, use binary-preserving Command Prompt redirection with `fixtures\contracts\AGENTS.md`. Text-mode PowerShell pipelines may change encoding/newlines before the CLI receives them. Hashes describe bytes actually supplied. The CLI currently has no `inspect`, `compose`, workspace-management, or execution subcommand.

### Native desktop workbench

```sh
cargo +1.98.0 run --manifest-path apps/desktop/Cargo.toml --locked
```

Choose `fixtures/contracts/AGENTS.md` for a first run. Inspect the report, explicitly save it, derive a skill from a section, edit/review a revision, then quit and reopen saved data. Open Decompose to work from a saved source, Merge to combine two saved skill revisions, or Split to divide one saved revision. Inspect the exact native preview before acknowledging and saving. Backup and restore controls are in Workspace. Installer bundling is disabled; this command runs a development executable.

## Validation and platform support

For local minimum-toolchain source validation, install Rust 1.85.0 with rustfmt and Clippy, then run `node scripts/check-source-rust.mjs`. The script resolves the toolchain's Cargo path, pins compiler/documentation executables and subprocess PATH, prints and verifies Cargo/rustc/Clippy versions, then runs formatting, workspace tests and warnings-denied Clippy. It rejects environment compiler-wrapper overrides and disables file-configured compiler wrappers for its child processes without changing configuration files. This avoids a Homebrew `cargo-clippy` overriding the requested rustup toolchain. `CARGO_TARGET_DIR` may select an isolated build cache; the separate desktop workspace retains its own Rust requirement.

| Evidence | What it establishes | What it does not establish |
| --- | --- | --- |
| Core tests and independent vectors | Contract, identity, transformation, storage, and recovery behavior under tested conditions | Native interaction, production readiness, or adversarial certification |
| Node controller/view tests | State transitions, closed DTO handling, escaped rendering, and synthetic interactions | Real native IPC or OS picker behavior |
| Rendered browser QA | Layout, theme, focus, and interaction behavior for recorded states | Native persistence or engine integration |
| Native source/tests/build | Host code compiles and tested helper paths pass on an identified runner | Clean-host GUI, installation, signing, updates, or released support |
| macOS development-bundle QA | Named real picker, persistence, skill, and data-control flows on the recorded build | Windows/Linux GUI parity or complete desktop qualification |
| Release qualification | Future clean-host install/use/update/rollback evidence for exact artifacts | Any broader certification without its own assessment |

The current workflow runs Rust checks on Ubuntu 24.04, Windows 2022, and macOS 14; frontend checks on Ubuntu; and native format/tests/Clippy/build on all three OS runners. It builds executables without publishing installers. Windows/Linux interactive qualification remains pending.

The [vision build rule](AGENTS.md) keeps the original eight screens as the structural baseline while requiring technical evidence for each feature. `npm run build:check` runs syntax, asset integrity, [vision contracts](docs/vision-contract.json), and the complete Node behavioral suite. Implemented surfaces must name native/core source separately from their UI; sample-only and unavailable surfaces keep explicit next technical outcomes. Visual-only work cannot count as capability completion. Rendered dark/light review at 320/768/1440 and fresh independent review complement these narrow source checks.

```sh
npm run build:check
node scripts/validate-review.mjs
node scripts/check-source-rust.mjs
cargo +1.98.0 fmt --manifest-path apps/desktop/Cargo.toml -- --check
cargo +1.98.0 test --manifest-path apps/desktop/Cargo.toml --locked
cargo +1.98.0 clippy --manifest-path apps/desktop/Cargo.toml --all-targets --locked -- -D warnings
cargo +1.98.0 build --manifest-path apps/desktop/Cargo.toml --locked
git diff --check
```

Current counts and exact run receipts belong in [development.md](docs/development.md), rather than a permanently fixed test-count badge. A passing adjacent stage does not close an untested release gate.

## Roadmap

The accepted [intent](docs/intent.md), detailed [plan](docs/plan.md), [product experience contract](docs/product-experience.md), and [application architecture](docs/application-architecture.md) guide delivery. The milestones below describe intended outcomes and acceptance gates, not fixed dates or a declaration that version 1.0 is complete.

### 1. Complete the real composition workbench

**Foundation present:** deterministic core, new/append destination semantics, atomic storage, versioned recovery, native preview/commit, and interactive editor routes with initial macOS save/restart evidence.

**Deliver:** complete qualification and refine the Decompose, Merge, and Split experience across saved-input selection, historical revision pinning, source/recipe/graph views, output destinations, before/after inspection, explicit coverage actions, conflict declarations, and actual saved-result navigation.

**Acceptance:** exact BOM/CRLF/Unicode preservation; no silently omitted spans; stale heads and workspace changes invalidate acknowledgment; failure preserves the draft; all output destinations commit together; restart/reopen works; dark/light, keyboard, reduced motion, and narrow layouts remain usable. Complete native GUI evidence on all three target OSs before declaring that milestone qualified everywhere.

### 2. Harden local data and recovery

**Foundation present:** bounded SQLite, schema 1/2/3 validation, transaction/recovery tests, V1/V2 backup handling, additive restore, and dependency-aware logical deletion.

**Deliver:** select and implement the reviewed encryption/key-custody design, authenticated history, explicit recovery UX, retention controls, and a documented schema/backup compatibility matrix.

**Acceptance:** demonstrate key-loss and backup recovery, corruption response, stale/rollback limits, interrupted migration/rotation, disk-full behavior, and no silent plaintext downgrade. Preserve legacy data through an explicit verified migration. Treat secure deletion and compromised-host resistance as bounded claims.

### 3. Build versioned harness adapters and deterministic export

**Foundation present:** pure `agents_md_v1` and `claude_md_v1` text-format profiles, exact stored-revision selection, static compatibility diagnostics, deterministic candidate manifests, a pure exact-byte bundle encoder/strict verifier, bounded selected-file host helpers, and native Export/Inspect UI/IPC implemented in this source. Native GUI qualification and actual target-loader qualification are not complete.

**Deliver:** a shared capability representation plus at least two explicitly supported harness/format adapters. Choose initial targets from actual user projects and maintain a per-version support matrix.

Each adapter declares accepted input fields, target syntax, preserved semantics, unsupported fields, loss diagnostics, and required permissions. Export plans show destinations and exact changes before writing. Bundle identity includes source/revision digests, transformation and adapter versions, diagnostics, and test results.

**Acceptance:** independent golden/negative fixtures; deterministic repeat exports; no silent constraint loss; confined output paths; explicit handling of generated versus user-owned files; reproducible compatibility reports. A familiar provider logo is not compatibility evidence.

### 4. Make Test Lab, agents, and workflows operational

**Foundation present:** pure typed workflow definitions, deterministic structural validation, immutable revision candidates and the complete durable Rust storage/recovery lifecycle. Native workflow authoring custody/commands, the real library/canvas/inspector source and complete v2 Data inventory/recovery are implemented. Native workflow save/restart/history/failure and three-OS GUI qualification remain open. The separate workflow design preview is synthetic.

**Deliver:** user-facing static validation, fixture simulations, reusable agent definitions, and workflow graphs with typed inputs/outputs, dependencies, retries, timeouts, cancellation, and evidence links. Templates and a registry build on versioned records and dependency closure.

Keep pure validation and simulated steps distinct from provider or connector runs. Separate a workflow definition from a run record. Runs need durable states for queued, running, waiting for review, completed, failed, cancelled, and unknown effect; failure and unknown effect must not be conflated.

**Acceptance:** deterministic fixtures where applicable; observable state transitions; recoverable interruptions; bounded retries; no automatic retry of an uncertain consequential effect; meaningful comparison against expected results. Real execution requires its own qualified runner and authority path.

### 5. Qualify LNSAT and connector operations

**Deliver:** an exact, versioned LNSAT adapter beginning with contract/identity checks and read-only evidence, followed by one disposable governed operation with a complete consequence receipt. Add connectors individually with scoped permissions and declared effect classes.

A connector transports requests; it does not become the source of authority. An authenticated credential proves access only within its actual scope. Approval must bind the exact operation, resources, constraints, and expiry where required by the qualified engine contract.

**Acceptance:** compatibility failures remain explicit; authentication and authorization are independently tested; replay, expiry, revocation, cancellation, lost responses, and reconciliation have evidence; UI activity is not mislabeled as authoritative audit evidence. LNSAT 1.0 arrival triggers qualification, not automatic activation.

### Model assistance gates (M1b–M1e)

M1a is the pure context-pack and advisory-response core described above. Remaining gates stay separate from that implementation and require their own evidence:

- **M1b:** the source-only `rangoon-model-local` library provides immutable connection profiles, a bounded numeric-loopback adapter, source-free connection check, cancellation/timeout/size handling, no startup traffic, and no automatic provider fallback. The separate `rangoon-model-session` crate provides portable session-only profile/request custody, exact saved-ID resolution, protected full-record packing, single-use handles, generation/cancellation/RAII leases, and freshness helpers. The native host now wires the actual `#model-assistance` workbench, seven main commands, three isolated review commands, exact-payload consent, narrow IPC, cancellation and freshness checks. Isolated macOS synthetic QA passed the named prepare/GET/POST/cancel/inert-response cases recorded in [development.md](docs/development.md). Native cloud integration is source-implemented; three-OS credential custody qualification, proposal application, tokenizer/quality qualification, real compatibility and Windows/Linux GUI remain open. M1b is incomplete.
- **M1c:** native credential management is implemented under the [cloud custody contract](docs/model-cloud-custody.md). Fixed OpenAI-slot inspect, add/replace and remove commands use explicit OS stores and native Save/Remove decisions. The portable fixed-origin cloud adapter is implemented and tested with synthetic TLS fixtures; cloud session custody adds saved-ID resolution and single-use request ownership. Native exact-request/credential consent integration is source-implemented. Real-provider compatibility and all-three-OS runtime qualification remain open; M1c is incomplete.
- **M1d:** native payload review and pure attributed proposal inspection exist. Complete native result retention/comparison, durable provenance, stale/error/partial states, accessible controls, and explicit draft application. Proposals remain advisory and inert until existing local review rules accept a change.
- **M1e:** task-quality and packing qualification on public or synthetic fixtures, with stated tokenizer/accounting assumptions, coverage and correctness measurements, latency/input/output measurements, regression limits, and honest failures. This gate does not follow from the M1a byte counters.

### 6. Release the free three-OS desktop product

**Deliver:** verified attribution/contribution/dependency notices, reproducible builds, documented minimum OS/architecture requirements, packaged artifacts, signing strategy, updates, backup compatibility, and rollback procedures for macOS, Windows, and Linux in the initial release.

**Acceptance:** clean-host install/launch/select/save/reopen/edit/review/compose/backup/restore/delete/update/uninstall checks; platform accessibility and keyboard evidence; artifact hashes; retained failure receipts; explicit limitations. Core portability and native CI are prerequisites, not substitutes for this release matrix.

### 7. Expand to self-hosted teams, then managed enterprise

**Deliver:** an owner-hosted service sharing domain contracts, followed by separately qualified managed hosting. Introduce authenticated actors, workspace/tenant isolation, role boundaries, collaboration, durable jobs, quotas, migrations, operational monitoring, and recovery procedures.

Enterprise candidates include SSO/SCIM, managed retention, audit export, fleet configuration, private registries, deployment policy, and support. Core content portability should not depend on a proprietary hosted account; final licensing and commercial packaging remain decisions to make explicitly.

**Acceptance:** isolation and authorization tests across every API/job/storage boundary; load and recovery evidence; tested backup/restore and disaster recovery; clear data residency, egress, deletion, and outage behavior. Tenant administration, subscription entitlement, and execution authority remain separate concepts.

### 8. Extend systems and standards with measured conformance

**Deliver:** a versioned adapter SDK with manifests, schemas, fixtures, and compatibility tests. Consider MCP for tool/resource interoperability, A2A where cross-agent communication is required, OpenAPI for service contracts, and OpenTelemetry for operational observability only when the corresponding feature and privacy review justify adoption.

Pin supported protocol versions and negotiate capabilities explicitly. Unknown extensions must not silently grant permissions. Redact or omit source bodies, prompts, secrets, and customer data from telemetry by default. Standards support needs a named conformance matrix and tests; listing a standard is not an implementation claim.

### 9. Build a credible government research evidence package

**Deliver:** a reproducible evaluation plan and claim-to-evidence register aligned to a specific funding solicitation when selected. The current government-grant work is research preparation; eligibility, registrations, application submission, and compliance remain separate work.

Measure preservation of source constraints across transformations/adapters, provenance completeness, reviewer task completion, recovery under interruption, interoperability limits, and resistance to bypass in specifically qualified authority paths. Record baselines, datasets, methodology, negative results, and reproducible artifacts. Avoid invented compliance percentages or unsupported claims of universal safety.

## License and commercial services

Rangoon's original source and documentation use [Apache License 2.0](LICENSE), with attribution in [NOTICE](NOTICE). The license permits use, modification and redistribution, including commercial use. Redistributors must provide the license, retain applicable notices and mark modified files; distributed derivatives must carry applicable NOTICE attribution. The license includes a bounded contributor patent grant and its stated termination provisions, and does not grant trademark rights beyond its stated exceptions. Third-party components and separately identified materials retain their own terms. See the [official license](https://www.apache.org/licenses/LICENSE-2.0) for the authoritative conditions.

Hypler's intended consulting business complements the free community app: implementation and integration services, capability/workflow design, deployment assistance, training, custom development and paid maintenance/support. Customers may use or modify community code commercially without buying consulting. Paid services do not convert planned enforcement, certification, hosted operations or platform qualification into current capabilities. Consulting agreements, support commitments and any future separately licensed enterprise components need their own terms; adding restrictions to the Apache license is not the business model.

The repository keeps the unmodified license text at its root so GitHub can recognize Apache-2.0. The portable Cargo workspace declares `license = "Apache-2.0"`, every first-party member inherits it, and the separate desktop manifest and Node package declare the same SPDX identifier. `publish = false` and `private: true` remain in force: license metadata does not publish packages or installers. GitHub's default-branch license display is verified separately after main publication; an open pull request alone does not update that display.

Contributor/DCO policy and complete dependency/distribution notices remain release work. Package metadata identifies first-party licensing; it does not relicense dependencies or replace distribution attribution. License adoption does not complete installer, security or government qualification.

## Contributing and documentation

Use [intent.md](docs/intent.md) for accepted scope and [development.md](docs/development.md) for exact implementation/publication evidence. Product designs and older packet records are supporting context; their original status labels may describe an earlier stage. Changes should name the controlling contract, preserve unrelated work, and include appropriate tests plus independent review.

Every push or merge to main must include a substantive update to this README that reflects the final delivered change and its limits. The controller verifies this in the reviewed publication diff under [AGENTS.md](AGENTS.md); exact test and merge receipts remain in the development ledger.

Keep fixtures secret-free. Imported instruction text is data, including when it contains commands. Do not commit private project content, credentials, local databases, provider payloads, production records, or engine tokens. Preserve the distinction between synthetic UI, implemented source, observed native behavior, and released support.

| Document | Purpose |
| --- | --- |
| [Intent](docs/intent.md) | Accepted outcome, constraints, and authority |
| [Development ledger](docs/development.md) | Exact tests, reviews, CI, commits, and publication receipts |
| [Product experience](docs/product-experience.md) | Image-derived behaviors and recovery expectations |
| [Architecture](docs/application-architecture.md) | Module ownership and staged engine adoption |
| [Plan](docs/plan.md) | Broader product, platform, and delivery direction |
| [Source analysis](docs/source-analysis.md) | Parser subset, input bounds, records, and identity framing |
| [Local workspace](docs/local-workspace.md) | Snapshot storage, permissions, and initial schema |
| [Reviewed capabilities](docs/reviewed-capabilities.md) | Immutable skill history and local review |
| [Composition](docs/composition.md) | Recipes, coverage, conflicts, and exact materialization |
| [Destinations](docs/composition-destinations.md) | New/append outputs, schema 3, and revision provenance |
| [Recovery](docs/composition-recovery.md) | Archive format, additive restore, and dependency deletion |
| [Native composition](docs/composition-native.md) | Raw IPC, retained previews, and confirmation semantics |
| [Instruction compilation](docs/compilation.md) | Pinned format profiles, byte preservation, compatibility limits, candidate identities and bundle foundation |
| [Instruction bundles](docs/instruction-bundle.md) | Exact portable format, strict verification and bounded host file boundary |
| [Model assistance](docs/model-assistance.md) | Accepted local/cloud assistance contract, context packing, proposal validation and staged gates |
| [Native Compile inspection](docs/compilation-ui.md) | Exact-ID command, workbench states, stale report handling and qualification gates |
| [Native bundle export and inspection](docs/instruction-bundle-ui.md) | Export/Inspect commands/UI in this source, picker custody, receipts, uncertainty and external-file limits |
| [Workspace controls](docs/workspace-data-controls.md) | Original inventory, export, restore, and deletion experience |
| [Native workflow authoring](docs/workflow-native-authoring.md) | Draft identity, exact candidate custody, historical reads and one-attempt save commands |
| [Complete native controls](docs/workflow-native-controls.md) | Explicit v2 inventory, workflow recovery/deletion, and single-attempt custody |
| [Desktop setup](docs/desktop-spike.md) | Native prerequisites and runtime qualification limits |
| [Engine integration](docs/engine-integration.md) | Unavailable port and LNSAT qualification gates |
| [Claims and evidence](docs/claims-and-evidence.md) | LNSAT research snapshot and safe claim boundaries |
| [Visual system](docs/visual-system.md) | Icons, themes, motion, and asset rules |
| [Preview gallery](docs/preview-gallery.md) | Clearly labeled synthetic design captures |
| [Funding research](docs/government-funding.md) | Government funding research and evidence preparation |

The marketing website is maintained separately. This repository contains the Rangoon application under development.

## Native cloud credential management

The Model Assistance workbench provides explicit **Inspect custody**, **Add/replace credential** and **Remove credential** actions for one fixed OpenAI slot. It does not access credential storage at startup or during polling, test a key, contact OpenAI or send source implicitly. Explicit cloud actions use the app-owned service/account namespace, native full-envelope custody, fixed origin, review and final OS consent; provider compatibility and native GUI/release qualification remain open. The renderer cannot select arbitrary credentials, providers, origins or backends.

The desktop workspace pins `keyring-core` 1.0.0 with explicit macOS User Keychain, Windows Credential Manager (Local persistence), and Linux Secret Service adapters. An unavailable or locked store reports failure; there is no plaintext or session fallback. The stored binary envelope combines a version marker, random 32-byte revision and bounded credential. Inspection returns only a random revision reference and custody metadata, never a key, prefix or key hash. SQLite, workspace exports and instruction bundles contain no credential records.

New keys are typed into a separately scoped password-entry WebView. That isolated renderer necessarily holds new text transiently; the main workbench never receives it, and existing keys are never sent to the entry window. Native code validates bounded raw IPC and requires a final OS Save/Cancel decision. Save compares the slot with its native baseline, writes once and verifies exact readback. Removal similarly rechecks the selected envelope, requires OS consent and verifies absence. A changed slot is refused; uncertain writes require explicit inspection. No automatic retry or provider fallback exists.

Native secret buffers use zeroizing ownership, but JavaScript, IPC, OS and dependency allocations have no proven erasure guarantee. OS custody does not protect a compromised signed-in session or application process, attest a Linux store's at-rest configuration, encrypt Rangoon's SQLite workspace, or establish tamper resistance/certification. The OS read/compare/write sequence is not atomic against other processes. Synthetic macOS Keychain lifecycle evidence and native tests are recorded in the [development ledger](docs/development.md); Windows/Linux runtime custody and all-three-OS GUI qualification remain open. See the [full custody contract](docs/model-cloud-custody.md).


### Fixed-origin cloud transport library

`rangoon-model-cloud` is the portable transport foundation for the next native
cloud workflow. `CloudProfile::parse` accepts only a versioned profile ID, model
and output cap; the host, port, endpoint, authentication scheme and wire adapter
are fixed. `PreparedRequest::new` checks the context-pack target and binds exact
serialized body bytes to the profile, pack and OS credential revision. The body
uses one non-streaming Responses request with JSON output, no tools, no server
conversation, disabled truncation and `store: false`. JSON mode does not replace
Rangoon's closed proposal validation.

`CloudClient::analyze` requires explicitly supplied zeroizing credential material
matching that revision. It resolves only `api.openai.com`, rejects the documented
special-use address ranges, dials one validated address, and verifies TLS with
bundled WebPKI roots and a fixed server name before constructing Authorization.
It has no proxy/environment-key discovery, redirect following, retries, model
fallback, decompression or detached request driver. Bounds include a shared
10-second DNS/TCP/TLS deadline, 120-second total deadline, 256 KiB final body,
1 MiB response body, and one in-flight cloud operation per process.

Completed output must contain one assistant text message whose JSON validates
against the retained pack and its citations. Refusals, tool output, partial
responses, malformed usage, oversized data and duplicate JSON keys fail with
closed diagnostics. Locally bound receipts distinguish provider-reported model
and usage from unknown cost, region and retention. Cancellation can stop local
waiting and close the socket; it cannot recall a dispatched request or guarantee
no billing. `store: false` is not a Zero Data Retention claim.

`CheckRequest::new` and `CloudClient::check` provide an explicit authenticated
`GET /v1/models/{model}` with an empty body. The immutable request binds the
profile, credential revision, method/path and empty-body digest. It shares the
TLS and single-flight protections above, with a 30-second total deadline and
64 KiB response cap. Strict metadata decoding requires the requested model ID;
a successful receipt means model visibility only, not inference compatibility,
zero cost, local processing, or retention guarantees.

The portable cloud session reserves a resolution lease before native credential
reads and rejects continuation after cancellation or profile changes. Native
local and cloud operations hold a shared model gate through workspace callbacks
and pending OS consent. Dropping an IPC waiter cancels retained work; OS dialog
callbacks complete their own cleanup. Native cloud orchestration also holds
credential-use custody through final envelope re-read and transport.

The portable library does not establish consent or OS freshness by itself;
native source now supplies those checks. No existing keys, paid calls or private
sources are used in source validation. Provider compatibility, native GUI
(including an unresolved Keychain access prompt after a QA rebuild), all-three-OS GUI and release
evidence remain open. See the [adapter contract](docs/model-cloud-adapter.md),
[credential custody](docs/model-cloud-custody.md) and canonical
[development evidence](docs/development.md).
