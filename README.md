# Rangoon

**A local-first workbench for building, understanding, and governing AI capabilities.**

Rangoon brings agent instructions, reusable skills, workflows, connectors, tests, and evidence into one inspectable lifecycle. The immediate problem is practical: useful knowledge is scattered across `AGENTS.md`, `CLAUDE.md`, `SKILL.md`, project rules, and scripts. Teams need to understand that material, preserve its origin, reorganize it without silently losing constraints, and see exactly what will change before publishing it elsewhere.

The long-term product is a desktop and cloud control plane. The current implementation is an **experimental desktop source workbench**, with real local persistence, immutable skill revisions, composition infrastructure, an actual `#model-assistance` workbench, and a separate synthetic design preview. The native application has no live agent executor, cloud-provider integration, connector execution, or LNSAT transport. Its model path is limited to explicitly configured numeric-loopback local sessions and advisory results; cloud request profiles, transport, proposal application, quality and release qualification remain open. Native OS credential management is implemented separately, with explicit inspection and isolated entry; it cannot contact a provider.

The first desktop release targets **macOS, Windows, and Linux together**. The initial product is intended to be free and open source; a software license has not yet been adopted. No license grant or released operating-system support is implied by the public source.

![Rangoon Command Center design preview](docs/screenshots/command-dark.jpg)

*Command Center design preview: synthetic data showing the intended visual direction. Native features and their evidence are distinguished below.*

## Contents

- [Product and feature status](#product-and-feature-status)
- [Architecture](#architecture)
- [Import and source analysis](#import-and-source-analysis)
- [Workspace and revision model](#workspace-and-revision-model)
- [Composition: Decompose, Merge, and Split](#composition-decompose-merge-and-split)
- [Instruction compilation](#instruction-compilation)
- [Model assistance](#model-assistance)
- [Backup, restore, and deletion](#backup-restore-and-deletion)
- [Native application boundary](#native-application-boundary)
- [Local database security](#local-database-security)
- [LNSAT integration](#lnsat-integration)
- [Run from source](#run-from-source)
- [Validation and platform support](#validation-and-platform-support)
- [Roadmap](#roadmap)
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
| Decompose/Merge/Split | Real native editor routes over saved records, exact recipes/coverage, new/append destinations, retained previews, atomic storage, and schema 3 recovery | Complete advanced GUI/failure qualification and Windows/Linux interactive evidence |
| Data controls | Inventory, plaintext portable backup, additive restore, dependency-aware logical deletion | Encryption, retention policy, recovery UX expansion, and additional platform evidence |
| Engine integration | Explicit unavailable LNSAT port, native status page, and CLI diagnostics | Qualified transport, authentication, typed operations, and evidence readback |
| Model assistance | M1a pure pack/validation, session-only local custody, and a native `#model-assistance` workbench are implemented. The main window exposes `get_local_model_profile`, `configure_local_model`, `clear_local_model`, `prepare_local_model`, `check_local_model`, `send_local_model`, and `cancel_local_model`; an isolated native review window exposes `get_local_model_review`, `confirm_local_model_review`, and `cancel_local_model_review`. Exact-payload review uses a final parented OS dialog; responses remain inert and advisory | Cloud transport and three-OS credential custody qualification, tokenizer/accounting, proposal application, real-model compatibility, Windows/Linux GUI and release qualification remain open; M1b/V1 are incomplete |
| Workflows and agents | Synthetic visual concepts and architecture contracts | Real definitions, validation, execution-state model, scheduling, and qualified runners |
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
| `crates/rangoon-engine/` | Inert `GovernancePort` and LNSAT unavailable diagnostics |
| `crates/rangoon-model-assistance/` | Pure bounded context packing and advisory-response validation; no transport, provider, credential, or authority behavior |
| `crates/rangoon-model-local/` | Explicit numeric-loopback Ollama-compatible HTTP/1 transport for prepared advisory requests; the desktop host supplies consent and calling orchestration; no credential or authority behavior |
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

The packer uses fixed checked-in task templates and bounded canonical JSON. The response decoder is closed and bounded before typed decoding; it requires the task-specific proposal kind, citations wholly inside selected ranges, and bounded uncertainty entries. `authoredText` remains inert model-authored text. Successful validation carries `authority: none`; it does not review, apply, execute, or mutate content. The pure core cannot authenticate database existence, revision ancestry, provider/profile identity, or operator consent, and it cannot authorize sending a payload. The separate [`rangoon-model-local`](crates/rangoon-model-local/) library provides a source-only M1b transport seam: `LocalProfile::parse`, `PreparedRequest::new`, and explicit `LocalClient::check`/`analyze` use direct Tokio TCP and Hyper HTTP/1 only for a caller-invoked numeric loopback target (`127.0.0.1` or `::1`) and prepared payload. It uses no proxy, redirect, retry, fallback, credential, model launch, or background traffic; the loopback peer may forward or retain data. The native host now supplies the command, review, consent, cancellation and freshness path. Isolated macOS synthetic QA passed 16-record preparation, source-free GET, zero POST through review/OS cancellation, one approved 12,580-byte POST, inert HTML output, and invalid-response rejection; see the [development ledger](docs/development.md). M1b/V1 remain incomplete: cloud transport and three-OS credential custody qualification, proposal application, tokenizer/quality qualification, real compatibility and Windows/Linux GUI support remain open.

The [`rangoon-model-session`](crates/rangoon-model-session/) crate supplies portable session custody around those libraries, and `apps/desktop/src/models.rs` wires it into the native host. `LocalSession` keeps one optional profile, one prepared request and at most one active operation in memory; `configure`, `inspect`, `clear`, `cancel`, `begin_prepare`, `begin_check` and `begin_send` enforce session lifecycle. Preparation resolves exact saved source or capability-revision IDs, packs each nonempty record as one protected full-content selection, rejects empty records, and exposes immutable request metadata. Random single-use `prepared:` and `run:` handles, checked generations, cancellation and non-cloneable RAII leases prevent stale or replayed ownership. `Transmission::freshness` is a read-before/read-after helper for native orchestration. The library performs no network calls or authority actions; the native host owns the seven main commands, three review commands, isolated exact-payload window, parented OS dialog, single-use consent, cancellation and freshness checks. See the [native session contract](docs/model-native-session.md) and [native workbench contract](docs/model-native-workbench.md).

The local contract binds an exact immutable profile and final request payload, permits one in-flight operation process-wide, and fixes 3-second connect, 5-second version-check, and 120-second analysis deadlines. It caps the final request at 256 KiB, version responses at 1 KiB, chat responses at 1 MiB, and decoded message content at 128 KiB. Cancellation is monotonic and releases the guard; response parsing and proposal validation remain strict and advisory.

## Backup, restore, and deletion

The native Workspace surface provides real inventory, export through a save picker, restore through a file picker, and dependency-aware deletion. These controls exist now; they are not merely roadmap items.

### Portable backup

The current native export uses the V2 archive format for source bytes, capability histories, reviews, recipes, applications, and live provenance. V1 backups retain their original interpretation and remain accepted by the versioned restore path. Unsaved editor drafts and transient native preview handles are not backup records.

V2 uses a versioned magic header, bounded canonical JSON manifest, raw payload segments, and a SHA-256 trailer. Limits are 2 MiB for the manifest, 68 MiB for the complete archive, and 64 MiB for aggregate raw payload. Reconstructed SQLite allocation must independently fit the database ceiling. The archive is plaintext; its digest is not a signature or encryption.

### Additive restore

Restore computes the final union before mutation. Existing local records remain authoritative for membership and retained history. Absent capabilities may be imported with their complete histories; restore does not append missing historical revisions to an already-present capability or replace its head. Same-ID identity-bearing data must agree.

Missing pinned dependencies block dependent imports. The host retains the selected decoded archive and exact plan. Confirmation binds backup identity, current workspace state, and the proposed operation. State drift rejects the restore rather than applying an old plan. Migration and additions commit atomically.

### Dependency-aware deletion

Deleting a capability removes its owned revisions, reviews, and derivation membership. A surviving capability that depends on one of those revisions blocks deletion. Source deletion is likewise blocked by surviving source origins or recipe inputs. Orphaned application/recipe metadata can be removed as specified; unrelated inputs and other skills are not implicitly deleted.

Immutable recipe targets may describe outputs that were later deleted. Readback and restore use live membership and do not recreate those outputs merely because the historical application names them. A fresh, explicit composition can deliberately recreate a deterministic new destination; there are no permanent deletion tombstones.

Deletion is logical removal, not secure erasure. See [workspace data controls](docs/workspace-data-controls.md) and [composition recovery](docs/composition-recovery.md) for exact archive encoding, dependency closure, stale-state checks, and failure tests.

## Native application boundary

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

Composition commands specifically use raw UTF-8 JSON IPC bytes. Preview is bounded to 8 MiB + 16 KiB; confirmation to 4,096 bytes before domain deserialization. Tauri has already allocated the transport buffer, so this is not a transport-level allocation guarantee. Closed DTOs reject unknown/duplicate fields and unsupported schemas.

The host retains at most one composition preview. A fresh successful preview receives a 32-byte OS-random handle and replaces the previous slot. Confirmation marks the handle attempted before entering storage; it cannot be used for another attempt after failure or a lost response. Verified success consumes the slot. Restart invalidates all such handles. These are local selection handles, not authentication credentials.

## Local database security

### Protections implemented today

The store uses bundled SQLite, validated schema/record shapes, bounded transactions, fixed public errors, and unkeyed identity checks. Reopening source bytes and reconstructing composition provenance detects inconsistent records rather than silently accepting them.

New Unix workspace directories/files use owner-only permissions. Windows inherits the per-user application directory ACL; Unix mode behavior is not claimed for Windows. Known symlinks/reparse points and non-regular database/journal files are rejected. Parent directories are not held against a hostile concurrent local user.

SQLite connections disable trusted schema, use rollback-journal `DELETE` mode and `synchronous=FULL`, and apply allocation/value limits and a bounded lock timeout. Transaction and process-interruption tests are useful evidence; they do not prove power-loss durability on every device or filesystem.

### Limits of the present design

**The database and exported backups are unencrypted.** A party with sufficient filesystem access can read them. A writer able to replace the store can recompute its unkeyed hashes; internal consistency does not establish authentic authorship, an untampered history, or freshness after rollback. Local review labels and wall-clock timestamps are unauthenticated.

The current application makes no provider requests or background source uploads. Analysis output and backups contain source content, so publishing those artifacts can disclose that content. Logical deletion cannot promise removal from backups, filesystem snapshots, or storage media.

### Proposed hardening sequence

1. Define the threat model: accidental corruption, offline theft, another local account, malicious same-user software, rollback, and a compromised unlocked process require different controls.
2. Evaluate an encrypted SQLite backend and encrypted backup format with OS-protected key custody on all three platforms. Require explicit key-loss recovery, rotation, migration, and no silent plaintext fallback.
3. Add authenticated history that binds operations, identities, sequence, and prior state using protected key material. Keep consistency hashes for content addressing; do not relabel them as authentication.
4. Design independently stored checkpoints for rollback detection. Stronger enterprise evidence may require an external witness; restoring all local state together cannot be made reliably fresh by another file beside the database.
5. Fail closed on integrity failures, preserve recoverable evidence, and offer explicit recovery rather than silently creating a new empty store.
6. Test altered pages/records, replayed backups, interrupted key rotation, lost keys, disk exhaustion, migration failure, and recovery on macOS, Windows, and Linux.

These are proposed controls, not implemented guarantees. A compromised administrator or unlocked application process can exceed the protection of local encryption. Nothing in this repository establishes production readiness, security certification, government compliance, domestic processing, or government endorsement.

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

```sh
npm run check
npm run validate:visuals
npm test
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

**Deliver:** user-facing static validation, fixture simulations, reusable agent definitions, and workflow graphs with typed inputs/outputs, dependencies, retries, timeouts, cancellation, and evidence links. Templates and a registry build on versioned records and dependency closure.

Keep pure validation and simulated steps distinct from provider or connector runs. Separate a workflow definition from a run record. Runs need durable states for queued, running, waiting for review, completed, failed, cancelled, and unknown effect; failure and unknown effect must not be conflated.

**Acceptance:** deterministic fixtures where applicable; observable state transitions; recoverable interruptions; bounded retries; no automatic retry of an uncertain consequential effect; meaningful comparison against expected results. Real execution requires its own qualified runner and authority path.

### 5. Qualify LNSAT and connector operations

**Deliver:** an exact, versioned LNSAT adapter beginning with contract/identity checks and read-only evidence, followed by one disposable governed operation with a complete consequence receipt. Add connectors individually with scoped permissions and declared effect classes.

A connector transports requests; it does not become the source of authority. An authenticated credential proves access only within its actual scope. Approval must bind the exact operation, resources, constraints, and expiry where required by the qualified engine contract.

**Acceptance:** compatibility failures remain explicit; authentication and authorization are independently tested; replay, expiry, revocation, cancellation, lost responses, and reconciliation have evidence; UI activity is not mislabeled as authoritative audit evidence. LNSAT 1.0 arrival triggers qualification, not automatic activation.

### Model assistance gates (M1b–M1e)

M1a is the pure context-pack and advisory-response core described above. Remaining gates stay separate from that implementation and require their own evidence:

- **M1b:** the source-only `rangoon-model-local` library provides immutable connection profiles, a bounded numeric-loopback adapter, source-free connection check, cancellation/timeout/size handling, no startup traffic, and no automatic provider fallback. The separate `rangoon-model-session` crate provides portable session-only profile/request custody, exact saved-ID resolution, protected full-record packing, single-use handles, generation/cancellation/RAII leases, and freshness helpers. The native host now wires the actual `#model-assistance` workbench, seven main commands, three isolated review commands, exact-payload consent, narrow IPC, cancellation and freshness checks. Isolated macOS synthetic QA passed the named prepare/GET/POST/cancel/inert-response cases recorded in [development.md](docs/development.md). Cloud transport and three-OS credential custody qualification, proposal application, tokenizer/quality qualification, real compatibility and Windows/Linux GUI remain open; M1b is incomplete.
- **M1c:** native credential management is implemented under the [cloud custody contract](docs/model-cloud-custody.md). Fixed OpenAI-slot inspect, add/replace and remove commands use explicit OS stores and native Save/Remove decisions. Cloud transport, origin/TLS/redirect/proxy qualification, and exact request/credential consent binding remain to be built and qualified; M1c is incomplete.
- **M1d:** native payload preview, coverage/budget display, proposal inspection, stale/error/partial states, accessible controls, and explicit draft application. Proposals remain advisory and inert until existing local review rules accept a change.
- **M1e:** task-quality and packing qualification on public or synthetic fixtures, with stated tokenizer/accounting assumptions, coverage and correctness measurements, latency/input/output measurements, regression limits, and honest failures. This gate does not follow from the M1a byte counters.

### 6. Release the free three-OS desktop product

**Deliver:** an adopted license, reproducible builds, documented minimum OS/architecture requirements, packaged artifacts, signing strategy, updates, backup compatibility, and rollback procedures for macOS, Windows, and Linux in the initial release.

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

## Contributing and documentation

Use [intent.md](docs/intent.md) for accepted scope and [development.md](docs/development.md) for exact implementation/publication evidence. Product designs and older packet records are supporting context; their original status labels may describe an earlier stage. Changes should name the controlling contract, preserve unrelated work, and include appropriate tests plus independent review.

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
| [Workspace controls](docs/workspace-data-controls.md) | Inventory, export, restore, and deletion experience |
| [Desktop setup](docs/desktop-spike.md) | Native prerequisites and runtime qualification limits |
| [Engine integration](docs/engine-integration.md) | Unavailable port and LNSAT qualification gates |
| [Claims and evidence](docs/claims-and-evidence.md) | LNSAT research snapshot and safe claim boundaries |
| [Visual system](docs/visual-system.md) | Icons, themes, motion, and asset rules |
| [Preview gallery](docs/preview-gallery.md) | Clearly labeled synthetic design captures |
| [Funding research](docs/government-funding.md) | Government funding research and evidence preparation |

The marketing website is maintained separately. This repository contains the Rangoon application under development.

## Native cloud credential management

The Model Assistance workbench provides explicit **Inspect custody**, **Add/replace credential** and **Remove credential** actions for one fixed OpenAI slot. It does not access credential storage at startup or during polling, test a key, contact OpenAI or send source. Cloud model transport remains unavailable. The native host selects the app-owned service/account namespace; the renderer cannot select arbitrary credentials, providers, origins or backends.

The desktop workspace pins `keyring-core` 1.0.0 with explicit macOS User Keychain, Windows Credential Manager (Local persistence), and Linux Secret Service adapters. An unavailable or locked store reports failure; there is no plaintext or session fallback. The stored binary envelope combines a version marker, random 32-byte revision and bounded credential. Inspection returns only a random revision reference and custody metadata, never a key, prefix or key hash. SQLite, workspace exports and instruction bundles contain no credential records.

New keys are typed into a separately scoped password-entry WebView. That isolated renderer necessarily holds new text transiently; the main workbench never receives it, and existing keys are never sent to the entry window. Native code validates bounded raw IPC and requires a final OS Save/Cancel decision. Save compares the slot with its native baseline, writes once and verifies exact readback. Removal similarly rechecks the selected envelope, requires OS consent and verifies absence. A changed slot is refused; uncertain writes require explicit inspection. No automatic retry or provider fallback exists.

Native secret buffers use zeroizing ownership, but JavaScript, IPC, OS and dependency allocations have no proven erasure guarantee. OS custody does not protect a compromised signed-in session or application process, attest a Linux store's at-rest configuration, encrypt Rangoon's SQLite workspace, or establish tamper resistance/certification. The OS read/compare/write sequence is not atomic against other processes. Synthetic macOS Keychain lifecycle evidence and native tests are recorded in the [development ledger](docs/development.md); Windows/Linux runtime custody and all-three-OS GUI qualification remain open. See the [full custody contract](docs/model-cloud-custody.md).
