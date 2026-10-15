<!-- intent-driven-delivery:spec:v1 -->
# Specification: skill discovery and library organization

Status: accepted
Intent: [skill discovery and library organization](intent.md#skill-discovery-and-library-organization--october-13-2026)
Authority: [accepted continuation](intent.md#skill-discovery-and-library-organization--october-13-2026)
Owner: Jeff
Last updated: 2026-10-13

## Outcome and current boundary

Users should find existing instruction and skill files, understand what was found, explicitly import useful content, and organize saved skills without losing source history. The first implementation, UI-C3, searches and filters the already-loaded saved skill library. It does not search the filesystem. Native directory discovery, persistent categories, tags and collections remain separate technical packets. The [development ledger](development.md#ui-c3-saved-skill-library-search--october-13-2026) owns implementation and validation status.

The current native picker reads one selected Markdown file; the existing inert analyzer checks caller-provided bytes. Saved source sections already produce immutable capability revisions with origin, content review and history. Reuse those paths. Neither finding a file nor categorizing its text authenticates it, records a content review, or grants execution permission. Rangoon remains usable without models, providers or LNSAT.

| Existing function or requested feature | Current source versus planned increment |
| --- | --- |
| Selected Markdown file analysis and explicit snapshot save | Existing native selected-file and saved-source lifecycle; no directory discovery. Native source/platform evidence remains in the ledger. |
| Saved skill inspection, immutable revisions, local content review and composition provenance | Existing controllers and workspace services; UI-C3 preserves their identities and behavior. |
| Saved-summary query and Review/Origin facets | UI-C3 implementation scope; exact validation/acceptance belongs to its current receipt. No filesystem or full-content search. |
| Folder/skill-file discovery, rescan and selected batch import | Planned native intake packet; no current scanner or discovery command is claimed. |
| Persistent categories, tags and virtual collections | Planned metadata contract; current summary/detail schemas have no category/tag fields. |
| Removing saved records | Existing dependency-aware workspace data controls, separate from library filtering. No deletion of original files is added. |

## Behavior

### UI-C3: search the loaded saved library

Keep the existing grouped shell, compact themed header, left saved library, central editor and right provenance/history inspector. Add controls inside the library, not another top-level dashboard or a new canvas.

## Interfaces and contracts

The controller retains view-only filter state; native IPC and persisted capability schemas remain unchanged.

| Control | Meaning |
| --- | --- |
| Search saved skills | Local, bounded query over loaded summary titles and exact record/origin IDs. Case-insensitive text matching is presentation only; identities and authority checks remain exact. No full-content, disk or network search. |
| Review: All / Reviewed / Unreviewed | Filters the summary's existing latest-revision content-review flag. It does not review a skill. |
| Origin: All / Source section / Composition | Filters existing birth-origin kind. This is not a user category, supported application, permission or compatibility claim. |
| Clear filters | Clears only query and both facets. It cannot open, select, save, reload, review or remove a record. |

## Data, privacy, and permissions

Query is at most 128 Unicode code points; normalize case and Unicode for display matching only. Treat punctuation literally, never as regular expressions, path patterns or selectors. Reject invalid query/filter input without changing the previous view. Search and filter state is ephemeral, separate from persisted capability data and never sent to the native bridge, a provider or telemetry. No eager content reads are needed: at most 128 already-loaded summaries are inspected.

Filtering preserves the selected record, exact viewed revision, unsaved draft, source context and action guards. A selected record hidden by filters remains open with a visible library notice; clearing filters reveals its row again. Filtering must never silently select the first match. Input rerenders preserve focus and caret, existing editor/library scroll, and available draft/revision state. Changes while an operation is pending do not change that operation or its result admission.

## States and failure handling

Distinguish library unavailable, loading, failed with potentially stale prior results, genuinely empty, and no matching loaded records. Show visible match/total counts only for known loaded records; never turn missing data into zero skills. Keep stale/loading warnings even when filters hide every prior result. Announce filtered result counts without replacing existing operation/error announcements. Clear remains reachable when filters are active. Search/select/button targets are at least 44px, wrap at narrow widths, work with keyboard and preserve both themes and existing reduced-motion/forced-color rules.

Acceptance: behavior tests prove literal/Unicode/bounded matching, both facets combined, no automatic selection, no native calls, retained dirty/historical/pending/conflict state and Clear semantics. Render tests cover escaped query/title, unavailable/loading/stale/empty/no-match and hidden-selection messages. Actual browser tasks cover query typing/caret, select/Clear, keyboard, result counts and no overflow at 320/768/1440 in light/dark. Populated browser fixtures must be clearly synthetic and cannot claim native IPC or saved-task completion. Fresh independent review follows named build, document and preservation checks.

## Planned native discovery

The intended workflow is **Choose folders → Find files → Review candidates → Import selected content → Organize saved skills**. Discovered files and saved skills are different collections. A found file stays outside the workspace until explicit import. Do not display a functioning discovery button before a native command exists.

One native, user-selected root starts each search. Display the selected scope, exclusions and limits before scanning. No home-directory sweep, credential/environment discovery, automatic installed-harness search, background watcher, Git clone or upload. An explicit rescan is a new bounded read operation. The renderer receives opaque candidate identities and relative display paths; it must not choose arbitrary absolute paths for native reads.

Recognize candidate profiles separately: `SKILL.md` skill definitions; `AGENTS.md`, `CLAUDE.md` and other explicitly supported instruction profiles; generic Markdown is a separate opt-in candidate class. Filename/structure hints identify a format, not a reusable capability or trust level. Record unsupported/malformed front matter honestly. Use a bounded, closed parser without executable YAML tags, includes or network resolution; parser/version/support conformance must be frozen in the native packet. Supporting files are references until separately reviewed, never run during discovery.

Proposed initial limits for the native packet: depth 8, 5,000 directory entries, 1,000 candidates, 256 KiB per analyzed file and 16 MiB aggregate read bytes; the existing 10,000-line analysis limit remains. These are proposals until a reviewed scanner contract pins exact behavior. Report cancelled, limited/partial, unreadable, excluded, unsupported, changed and complete states with deterministic counts. Sort paths consistently; partial results must never be labeled a complete inventory.

Default exclusions include `.git`, dependency/build/cache trees, environment files and credential stores. Recognized hidden skill directories need explicit profile-specific allowlists; skipping every dot-directory would miss real skill locations. Reject symbolic links/reparse traversal and keep root/path custody during reads. Hostile replacement, traversal, hard-link exposure, case collisions, device/FIFO files, invalid names, oversized files, permission changes, cycles and cancellation require adversarial tests on each target OS. Do not infer Windows/Linux custody qualification from Unix tests.

Opening/importing a candidate revalidates exact source bytes/digest and scope against its retained discovery record; changed or vanished files require refresh. Reuse the existing selected-source analysis, explicit snapshot save, revision and provenance semantics rather than a second importer. Batch imports must preview existing storage quotas and dependency effects, require explicit selection, and report individual success/failure; they must not silently overwrite, merge or auto-review content. Identical digests can indicate exact-byte duplicates; similar names/content can suggest review only. Approximate matching never decides source identity or authorization.

Discovery and file content remain local. Production mounting must use the shared native admission, callback drain, stale-delivery fencing and retained-data purge contract. New persistence cannot bypass the unfinished encrypted routing/consent/recovery path or silently write to the legacy plaintext workspace. The native scanner packet must define its in-memory retention and disposal; full-text indexing and a watcher are deferred.

## Compatibility and migration

UI-C3 changes no persisted schema, backup or export format. Missing filter fields in older fixture states default to the unfiltered library. Existing native IDs, local content review and source/composition provenance remain exact.

### Planned durable organization and management

| Concept | User meaning and storage boundary |
| --- | --- |
| Category | A user-confirmed primary purpose, such as Engineering, Documentation or Operations. Keep Uncategorized valid. Category suggestions expose deterministic reasons and are never auto-applied. |
| Tags | Optional many-to-many labels, bounded in count and length. Preserve user spelling; normalized search aliases never become record identities. |
| Collections | Named virtual groups referencing exact capability IDs. Adding to a collection does not move, rename or edit source files. |
| Review and compatibility | Existing exact-revision content review and separately qualified target compatibility. These cannot be inferred from a category, logo or collection. |
| Management | Inspect, edit/revise, compare, export and reuse existing records. Bulk metadata changes need exact selected IDs, preview, stale-state checks, cancellation and complete outcome reporting. Deletion uses existing dependency-aware data-control plans and explicit confirmation; never a new file-delete shortcut. |

Keep organization metadata separate from immutable instruction content and birth provenance. A metadata edit must not silently create an instruction revision, discard history or confer authority. Before durable implementation, specify metadata revisions/concurrency, encrypted persistence, backup/restore format and collision behavior, capacity, logical deletion/reference blockers, native/CLI parity and restart recovery. Reuse existing workspace services. Do not create a parallel database or browser-local-storage catalog.

## Ordered implementation packets

1. UI-C3: loaded saved-library search and existing review/origin facets. Exact ownership and validators are in [plan.md](plan.md#skill-library-and-discovery-sequence).
2. A source-only, fixture-qualified native discovery contract and scanner, with explicit-root custody and deterministic candidate inventory. Root owns privacy/security and exact future file ownership; do not wire it into production before its admission/retention/consent review.
3. Native picker/search/review/import integration after complete encrypted command routing, delivery fencing and purge gates. Qualify cancellation, changed files, partial results and restart behavior; desktop OS qualification stays separate.
4. Durable metadata and virtual collections through existing encrypted workspace services, with schema/backup/deletion/concurrency review. Then task-tested bulk organization and CLI parity.

This sequence adds a useful local search increment without presenting planned discovery or metadata as shipped capability. It does not add execution, provider installation, credentials, network calls, models, dependencies or release authority.

## Acceptance mapping

| Outcome | Named validation |
| --- | --- |
| Literal, bounded query and existing facets; no authority/record mutation | `node --test tests/skills-model.test.mjs tests/skills-view.test.mjs tests/term-help.test.mjs`; exact bridge-call and retained-state assertions. |
| Honest state and focus behavior | Actual unavailable preview plus clearly synthetic populated browser tasks; 320/768/1440 light/dark; query/caret, selects, Clear, pending/conflict/historical retention and no overflow. |
| Existing behavior and original visual direction | `npm run build:check`; `node scripts/validate-review.mjs`; named artifact, link/anchor, owned whitespace and exact preservation checks. |
| Independent acceptance evidence | Fresh read-only UI/source/document review of frozen incremental patch, hashes and completed evidence; results in development.md. |

## Non-goals and open questions

UI-C3 does not read folders/files, write metadata, invoke native operations, use providers/models or qualify encryption, execution, installation, runtime compatibility or release. No dependency or framework is added. Native discovery limits/profile parsing, exact cross-OS path custody and scan-session retention need their own reviewed contract. Durable organization must settle metadata revision/concurrency and backup/deletion semantics before schema work. The controlling E1 routing/purge/consent/recovery gates remain open; this specification does not activate encrypted storage or migrate legacy data.
