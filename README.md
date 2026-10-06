# Rangoon

**A local-first workbench for building, understanding, and governing AI capabilities.**

Rangoon brings agent instructions, reusable skills, workflows, connectors, tests, and evidence into one inspectable lifecycle. The immediate problem is practical: useful knowledge is scattered across `AGENTS.md`, `CLAUDE.md`, `SKILL.md`, project rules, and scripts. Teams need to understand that material, preserve its origin, reorganize it without silently losing constraints, and see exactly what will change before publishing it elsewhere.

The long-term product is a desktop and cloud control plane. The current implementation is an **experimental desktop source workbench**, with real local persistence, immutable skill revisions, composition infrastructure, and a separate synthetic design preview. It has no live agent executor, provider integration, connector execution, or LNSAT transport.

The first desktop release targets **macOS, Windows, and Linux together**. The initial product is intended to be free and open source; a software license has not yet been adopted. No license grant or released operating-system support is implied by the public source.

![Rangoon Command Center design preview](docs/screenshots/command-dark.jpg)

*Command Center design preview: synthetic data showing the intended visual direction. Native features and their evidence are distinguished below.*

## Contents

- [Product and feature status](#product-and-feature-status)
- [Architecture](#architecture)
- [Import and source analysis](#import-and-source-analysis)
- [Workspace and revision model](#workspace-and-revision-model)
- [Composition: Decompose, Merge, and Split](#composition-decompose-merge-and-split)
- [Backup, restore, and deletion](#backup-restore-and-deletion)
- [Native application boundary](#native-application-boundary)
- [Local database security](#local-database-security)
- [LNSAT integration](#lnsat-integration)
- [Run from source](#run-from-source)
- [Validation and platform support](#validation-and-platform-support)
- [Roadmap](#roadmap)
- [Contributing and documentation](#contributing-and-documentation)

## Product and feature status

The first useful experience is deliberately concrete: select a Markdown file, inspect its exact content and section inventory, save a snapshot, derive a skill, edit a new revision, record local review, and reopen that work after restart. Composition extends this into traceable transformations across saved records. Future adapters will turn those records into target-specific exports with visible compatibility limits.

The intended users are developers managing agent configuration, platform teams maintaining shared capabilities, and reviewers who need source lineage and explicit change plans. Cloud administration and government use are later qualification targets, not capabilities established by the current desktop prototype.

| Area | Implemented in public source | Remaining work |
| --- | --- | --- |
| Design preview | Nine synthetic views: Command Center, Import, Decompose, Merge/Split, Skills, Workflows, Connectors, Evidence, and release planning; dark/light themes and responsive layouts | Replace each sample workflow with a real, qualified application service |
| Import & Analyze | Bounded Rust Markdown scanner, stdin CLI, and one explicitly selected native file; exact text, digest, spans, and diagnostics | Explicit directory/repository import, additional formats, optional semantic proposals |
| Local workspace | SQLite source snapshots; explicit save, deduplication, reanalysis on reopen | Broader project/workspace model and platform qualification |
| Skills | Source-section derivation, immutable revisions, history comparison, local content review, versioned provenance | Search/library expansion, compatibility, distribution, and collaboration |
| Decompose/Merge/Split | Pure transformation core, destination validation, atomic storage, schema 3 recovery, and native preview/commit commands | Complete editor publication and end-to-end GUI qualification |
| Data controls | Inventory, plaintext portable backup, additive restore, dependency-aware logical deletion | Encryption, retention policy, recovery UX expansion, and additional platform evidence |
| Engine integration | Explicit unavailable LNSAT port, native status page, and CLI diagnostics | Qualified transport, authentication, typed operations, and evidence readback |
| Workflows and agents | Synthetic visual concepts and architecture contracts | Real definitions, validation, execution-state model, scheduling, and qualified runners |
| Connectors and harnesses | Synthetic catalog and extension direction | Versioned implementations, permissions, compatibility fixtures, and conformance suites |
| Test Lab and export | Development tests and golden fixtures | User-facing tests, deterministic export, adapter compatibility reports, and evidence bundles |
| Distribution | Three-OS source/test/build CI; selected macOS development-bundle runtime evidence | Windows/Linux GUI checks, installers, signing, updates, rollback, and release qualification |
| Team/cloud/enterprise | Architecture direction | Service implementation, tenancy, identity, operations, and deployment evidence |

The atomic composition store and native integration reached public source through [PR #12](https://github.com/hypler-dev/rangoon/pull/12) and [PR #13](https://github.com/hypler-dev/rangoon/pull/13). The interactive composition editor is being developed and validated locally; this README does not claim that editor is already available on `main`. Exact implementation, test, and publication receipts belong in the [development ledger](docs/development.md).

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
   rangoon-import      rangoon-compose
 deterministic spans   pure recipes / coverage / destinations
            \              /
             rangoon-domain
          typed records / identities

   rangoon-engine: independent, unavailable LNSAT integration port
```

| Path | Responsibility |
| --- | --- |
| `crates/rangoon-domain/` | Source, span, capability, revision, and provenance record types; content identity helpers |
| `crates/rangoon-import/` | Deterministic bounded Markdown section analysis |
| `crates/rangoon-host/` | Selected-file reads and bounded native backup file handling |
| `crates/rangoon-store/` | SQLite schema validation, snapshots, revisions, local reviews, composition storage, backup/restore, and deletion |
| `crates/rangoon-compose/` | Pure transformation validation, coverage, materialization, destination application, and deterministic identities |
| `crates/rangoon-engine/` | Inert `GovernancePort` and LNSAT unavailable diagnostics |
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
| Composition | `preview_composition`, `commit_composition` | Saved input resolution and one retained preparation |
| Engine | `get_engine_status` | No-argument local unavailable diagnostic |

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

The [integration specification](docs/engine-integration.md) records an immutable public LNSAT research reference. Its source version, wire contract, product surface, storage schema, and eventual release support are distinct dimensions. A wire label containing `v1_0` does not establish a supported LNSAT product 1.0. Rangoon never opens the LNSAT database.

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

Choose `fixtures/contracts/AGENTS.md` for a first run. Inspect the report, explicitly save it, derive a skill from a section, edit/review a revision, then quit and reopen saved data. Backup and restore controls are in Workspace. Installer bundling is disabled; this command runs a development executable.

## Validation and platform support

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
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
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

**Foundation present:** deterministic core, new/append destination semantics, atomic storage, versioned recovery, and native preview/commit.

**Deliver:** publish the real Decompose, Merge, and Split editors with saved-input selection, historical revision pinning, source/recipe/graph views, output destinations, before/after inspection, explicit coverage actions, conflict declarations, and actual saved-result navigation.

**Acceptance:** exact BOM/CRLF/Unicode preservation; no silently omitted spans; stale heads and workspace changes invalidate acknowledgment; failure preserves the draft; all output destinations commit together; restart/reopen works; dark/light, keyboard, reduced motion, and narrow layouts remain usable. Complete native GUI evidence on all three target OSs before declaring that milestone qualified everywhere.

### 2. Harden local data and recovery

**Foundation present:** bounded SQLite, schema 1/2/3 validation, transaction/recovery tests, V1/V2 backup handling, additive restore, and dependency-aware logical deletion.

**Deliver:** select and implement the reviewed encryption/key-custody design, authenticated history, explicit recovery UX, retention controls, and a documented schema/backup compatibility matrix.

**Acceptance:** demonstrate key-loss and backup recovery, corruption response, stale/rollback limits, interrupted migration/rotation, disk-full behavior, and no silent plaintext downgrade. Preserve legacy data through an explicit verified migration. Treat secure deletion and compromised-host resistance as bounded claims.

### 3. Build versioned harness adapters and deterministic export

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
| [Workspace controls](docs/workspace-data-controls.md) | Inventory, export, restore, and deletion experience |
| [Desktop setup](docs/desktop-spike.md) | Native prerequisites and runtime qualification limits |
| [Engine integration](docs/engine-integration.md) | Unavailable port and LNSAT qualification gates |
| [Claims and evidence](docs/claims-and-evidence.md) | LNSAT research snapshot and safe claim boundaries |
| [Visual system](docs/visual-system.md) | Icons, themes, motion, and asset rules |
| [Preview gallery](docs/preview-gallery.md) | Clearly labeled synthetic design captures |
| [Funding research](docs/government-funding.md) | Government funding research and evidence preparation |

The marketing website is maintained separately. This repository contains the Rangoon application under development.
