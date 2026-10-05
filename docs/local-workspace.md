<!-- intent-driven-delivery:spec:v1 -->
# Specification: Explicit local source snapshots

Status: accepted
Intent: [Rangoon application intent](intent.md), October 5 R2a continuation and [reviewed capabilities](reviewed-capabilities.md) R2b specification
Owner: Jeff; implementation and release judgment remain with the primary controller
Last updated: 2026-10-05

## Behavior

R2a closes one durable loop: choose a Markdown file, inspect its exact original text, explicitly **Save locally**, quit, reopen Rangoon, and open the saved snapshot. The saved list contains actual local records. Identical basename and bytes return the existing record; changed bytes or basename create a separate immutable snapshot. No source is saved automatically. The interface describes unencrypted local storage before Save.

Saving is separate from review. Every reopened source is analyzed again by the current deterministic analyzer, retains `authority: none`, and has unreviewed fragments. There are no reviewed skills, project ownership, merge/split operations, execution grants or provider calls in this slice. The illustrated product remains the broader direction in the [experience contract](product-experience.md).

## Interfaces and contracts

The native host retains the latest successfully selected or reopened report. The renderer can send an opaque `sourceId`, never a path or source body, to the workspace commands:

| Command | Request | Successful result |
| --- | --- | --- |
| `select_and_analyze` | None; native picker | Existing `analyzed` report contract |
| `save_analysis` | Current `sourceId` | `saved`, metadata and `alreadySaved` |
| `list_snapshots` | None | `listed`, bounded metadata array |
| `open_snapshot` | Saved `sourceId` | `analyzed` report after revalidation |
| `clear_analysis` | None | `cleared`; active host report removed |

Metadata contains only `sourceId`, `displayName`, `sha256`, `byteLength`, and `savedAtMs`. The latter is the local wall clock at the first successful save, not an authenticated timestamp. Source identities are validated `source:` plus 64 lowercase hexadecimal characters. Saving requires that the requested identity still matches the host's active report. A hash identifies bytes and basename; it is not evidence of who wrote them.

`rangoon-store` is independent of Tauri. Its caller supplies a trusted application directory. The desktop uses Tauri's `app_local_data_dir()/source-workspace/workspace.sqlite3`. The renderer cannot choose that path. Source bytes are stored as BLOBs with their basename, digest and identity. Derived fragments, review decisions and execution permissions are not persisted.

## States and failure handling

Choose, Save and Open share a native busy guard. List reads use their own bounded transaction. UI requests preserve the current report on failure; an empty library is distinct from an unavailable library. Clear advances a host generation so a late picker/open result cannot repopulate cleared state. Clear removes only active analysis, not saved records. An explicit Save captures its source before committing; Clear never cancels a disk commit. The UI disables Clear and source-changing actions while Save/Open is pending.

SQLite transactions keep source metadata and bytes together. An uncertain save response can be retried without duplicating content. A returned error does not assert that nothing reached disk. The app never resets an unreadable or unfamiliar database. A failed initial transaction can leave an empty database, which the next explicit Save may initialize. Listing alone does not create a workspace.

The workspace accepts at most 128 snapshots, each under the analyzer's 256 KiB limit, with a 64 MiB database ceiling. Limits return errors and preserve existing records; R2a had no automatic eviction or retention/delete UI. R2c now adds explicit owner-controlled backup, additive restore, and dependency-aware logical deletion for saved records; see [workspace-data-controls.md](workspace-data-controls.md). A malformed record can prevent the bounded list from loading; the app reports an error rather than omitting it silently. Metadata listing does not validate every content digest; Open reanalyzes the requested bytes and checks identity, digest and byte length within the same read transaction.

## Data, privacy, and permissions

Original source text is stored **unencrypted on this computer** after explicit Save. There is no upload or browser storage of source data. Theme remains the only browser local-storage value. OS permissions and disk encryption remain the user's local security controls; this is not an encrypted vault or a tamper-proof evidence store. Cloud-synced OS folders, backups and privileged local users are outside this application's custody claim.

New workspace directories and files use owner-only permissions on Unix. Windows inherits the per-user application-directory ACL; no equivalent Unix-mode claim is made. Known symlinks/reparse points and non-regular database/journal files are rejected, and SQLite is opened with `NOFOLLOW`. Parent directories are not held or confined against a hostile concurrent local user. There is no arbitrary database import API. Errors expose fixed codes and messages, never absolute source paths or raw SQLite errors.

## Compatibility and migration

Schema version 1 uses SQLite `application_id = 0x52474e31`, `user_version = 1`, and one exact `snapshots` table schema. Unexpected tables, triggers, views, versions or application identities are rejected. Schema creation and the first snapshot share one transaction. No migration from a prior product store exists, and unsupported files are never replaced. Future schema changes require an explicit migration and recovery plan.

The pinned Rust binding is `rusqlite 0.40.2` with bundled SQLite. Connections disable trusted schema, use rollback-journal `DELETE` mode, `synchronous=FULL`, bounded SQLite values/SQL length, a one-second lock timeout and a maximum page count. SQLite's documented [atomic-commit design](https://www.sqlite.org/atomiccommit.html) and [trusted-schema setting](https://www.sqlite.org/pragma.html#pragma_trusted_schema) inform these choices. Process-interruption tests do not prove power-loss durability on every filesystem, hardware device or network share.

All three desktop operating systems remain initial release targets. The core and native CI matrices qualify source builds/tests; signed installers, packaged upgrade paths and Windows/Linux interactive save/restart checks remain release requirements.

## Acceptance mapping

1. Missing workspace listing creates no disk state; explicit Save creates the local store.
2. Save/reopen preserves BOM, CRLF, original bytes, digest and identity; every derived fragment remains unreviewed.
3. Duplicate saves return the same metadata and timestamp; changed content preserves earlier snapshots.
4. Invalid identities, stale host identity, cleared pending requests, corrupt bytes, unexpected schema and symlink/reparse paths fail without replacing active analysis or resetting saved data.
5. Transaction rollback, interrupted writer recovery, page-limit exhaustion and available platform permission checks preserve committed records.
6. Native command permissions cover exactly the five documented commands. Controller tests cover failed/retried operations and stale UI results; actual GUI evidence covers Save, restart, list and reopen.
7. Fresh independent code/UI review and exact CI/publication receipts are recorded in [development status](development.md). Claims remain bounded by completed evidence.

## Non-goals and open questions

R2b adds capability/revision/review tables to the same database at schema 2 only after explicit creation from a saved source section. Schema 1 remains source-only; reads and listing do not migrate or create stores. The first capability creation upgrades schema 1 lazily in the same immediate transaction, and older binaries reject schema 2. Limits are 128 capabilities, 32 revisions per capability and 1,024 total revisions. Local review is content inspection only and retains `authority: none`; original source bytes remain unchanged. See [reviewed-capabilities.md](reviewed-capabilities.md) for the complete contract and validation state.

R2a itself did not include reviewed assets or deletion controls; the linked R2b and R2c specifications add those later slices. No directory import, autosave, project model, source-file watching, encrypted database, synchronization, providers, approval engine, managed enterprise store or installers. R2c logical deletion is application-record removal, not secure erasure; original selected files remain untouched. Semantic decomposition, merge/split, arbitrary multi-source spans and broader composition remain planned work. R2c source publication is recorded in [development status](development.md); cross-platform GUI qualification, installer/released-OS support and release evidence remain pending.
