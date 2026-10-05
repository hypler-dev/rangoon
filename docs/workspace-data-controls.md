<!-- intent-driven-delivery:spec:v1 -->
# Specification: Workspace data controls (R2c)

Status: accepted scope under the version 1.0 build continuation; source and runtime evidence reviewed; publication pending
Intent: [intent.md](intent.md), version 1.0 build continuation
Owner: Jeff; primary controller owns storage, privacy, native boundaries and release judgment
Last updated: 2026-10-05

## Behavior

Give the owner control over saved local data before expanding composition. A Workspace page shows actual usage and saved sources/skills, exports a portable backup through a native save picker, previews an explicitly selected backup, restores missing records, and previews dependency-aware deletion. No automatic retention, eviction, overwrite, engine operation, upload or directory scan is introduced.

Backup includes every saved source, capability, immutable revision and local review record in one consistent read transaction. Unsaved editor drafts are excluded. Backup files contain unencrypted source content; the UI explains this before opening the destination picker. Existing destination files are never overwritten. An incomplete file after a failed write must not be reported as a successful backup.

Restore is additive. New sources and whole missing capabilities are restored. Existing source bytes and existing skills, including all their revisions/reviews, remain unchanged. The preview explicitly counts existing skills that will be kept; older or different backup versions do not replace them. Restore cannot silently append an old revision, overwrite a current head, promote authority, or reset an unfamiliar/corrupt workspace. After a successful restore, selecting the same backup again produces a fresh preview with no additions. A request using the original pre-restore state is stale and must not be retried automatically. Restore into an empty workspace preserves all archived content and local review metadata. Reviews in a backup are unauthenticated historical records, not proof that the current operator reviewed the content.

Deletion operates only on saved Rangoon records. A source referenced by any skill is blocked, with named dependent skills shown. Deleting a skill removes that skill's revisions and reviews atomically while preserving its source. The confirmation names the target and exact affected counts, explains permanence and recommends a backup. Deletion is logical removal, not secure erasure; SQLite may retain reusable pages. Original selected files are never modified. No automatic cascade deletes a source or another skill.

## Interfaces and contracts

All commands are restricted to the local main window. The renderer never supplies a file path, archive body, SQL, or engine request.

| Command | Arguments | Result |
| --- | --- | --- |
| `get_workspace_data` | None | `loaded`, `workspace` validated counts, saved source/skill summaries and opaque `stateId` |
| `export_workspace_backup` | None; native save picker | `exported`, `backupId`, `byteLength`, or `cancelled` |
| `prepare_workspace_restore` | None; native one-file picker | `restore_ready`, `plan` with backup ID, expected state ID, added/kept counts |
| `restore_workspace_backup` | `backupId`, `expectedStateId` | `restored`, actual added/kept counts; only host-retained validated backup may be used |
| `inspect_workspace_deletion` | `kind` (`source` or `capability`), `id` | `deletion_ready`, target title, affected counts, dependency summaries and expected state ID |
| `delete_workspace_record` | `kind`, `id`, `expectedStateId` | `deleted`, kind and ID; stale confirmation is rejected before target lookup; absent targets require a new inspection |

Failures use fixed codes/messages. Workspace counts include `sources`, `capabilities`, `revisions`, `reviews`, `databaseBytes` and `reusableBytes`; limits retain 128 sources, 128 skills, 32 revisions per skill, 1,024 total revisions and 64 MiB database allocation. Counts are read from one validated snapshot, not independent queries across changing state. The state ID is a versioned digest of the logical saved content, including timestamps and review metadata. It is a stale-state check, not authorization or a signature.

Restore/deletion preview does not write. Each write acquires an immediate transaction and checks the expected logical state before making changes. Any concurrent change invalidates the preview. The operator refreshes and reviews again; there is no automatic write retry. If a response is lost, refresh the inventory to inspect the committed outcome and prepare a new confirmation. A stale response cannot distinguish another writer from the prior operation, so the app reports no success without a receipt. The host retains at most one selected backup in memory until replaced, consumed or the process exits. Cancelled selection preserves the previous usable preview. A mismatched renderer backup ID cannot select arbitrary host data.

## Portable backup format

Extension: `.rangoon-backup`. Format version 1 is application data, never a SQLite database import or executable archive.

1. Magic bytes `RANGOON-BACKUP-V1\n`.
2. Four-byte unsigned big-endian manifest length (maximum 2 MiB).
3. UTF-8 JSON manifest with explicit version and typed arrays of sources, capabilities, revisions and reviews. Unknown fields/versions are rejected. Records are sorted by their opaque ID and duplicate IDs are rejected. Source/revision records declare the exact byte length of their following content; the manifest contains no paths.
4. Original source blobs in source order, then revision blobs in revision order, each of the exact declared length. No compression, archive filenames, symlinks, traversal or extraction commands exist.
5. Sixty-four lowercase ASCII hexadecimal bytes containing SHA-256 of all preceding bytes. No trailing data is accepted. This detects accidental corruption; a digest is not authenticated provenance.

Total selected file size is capped at 68 MiB; combined content has a 64 MiB upper bound. This is not a promise that 64 MiB of blobs fits: encode/decode also reconstruct the records into application-owned 4 KiB-page SQLite tables under the same 64 MiB allocation limit, including indexes and page overhead. An archive that cannot fit this canonical fresh workspace is rejected. Additive restoration into a populated workspace can still fail its allocation limit atomically. Each source/revision keeps its existing 256 KiB cap. The decoder bounds lengths/counts before processing bodies, validates UTF-8, reanalyzes all sources, verifies identities, lineage, original spans, closed reviewer values, timestamps and quotas. It uses only application-owned table definitions for validation and insertion. No archive-provided schema or SQL is opened or executed. Export is deterministic for the same logical workspace, independent of SQLite page layout.

### Manifest schema and canonical encoder

All fields below are required. JSON object fields not listed are rejected, including duplicate object keys. `parentRevisionId` is the only nullable field. Numeric fields are JSON integers, not quoted strings or fractions. IDs and digests are lowercase hex: IDs use the stated prefix followed by 64 hex characters; SHA-256 fields contain exactly 64 hex characters.

Canonical export uses compact UTF-8 JSON without BOM, whitespace or a trailing newline. Object keys appear in the order below. JSON strings use the `serde_json` compact string encoding: quotation mark/backslash and control characters are escaped, other Unicode scalar values are UTF-8. Decoder accepts semantically equivalent JSON key order/whitespace; the file checksum covers the actual encoded bytes. There is no Unicode normalization. Each array is strictly ascending by its indicated ID, comparing ASCII bytes.

| Object | Required keys in encoder order | Constraints |
| --- | --- | --- |
| Manifest | `version`, `sources`, `capabilities`, `revisions`, `reviews` | Version integer `1`; arrays bounded to 128, 128, 1024, 1024 entries respectively. |
| Source | `metadata` | Object only; content follows the manifest as raw bytes. Sort by `metadata.sourceId`. |
| Source metadata | `sourceId`, `displayName`, `sha256`, `byteLength`, `savedAtMs` | Source ID `source:`; validated Markdown basename; SHA-256 of exact original bytes; byte length 0–262144; timestamp below. |
| Capability | `id`, `sourceId`, `fragmentId`, `latestRevisionId` | Prefixes `capability:`, `source:`, `fragment:`, `revision:`. Sort by `id`. Source and fragment must resolve to the reanalyzed original; head must belong to this capability. |
| Revision | `id`, `capabilityId`, `parentRevisionId`, `title`, `sha256`, `createdAtMs`, `byteLength` | Prefixes `revision:`, `capability:`, nullable `revision:` parent; valid nonblank trimmed title ≤160 UTF-8 bytes without control/line-separator characters; content 1–262144 UTF-8 bytes, nonblank and no NUL. Sort by `id`. |
| Review | `revisionId`, `reviewer`, `reviewedAtMs` | Existing revision ID; reviewer exactly `local_operator`; sort by `revisionId`. These are unauthenticated historical annotations. |

Timestamps are integer milliseconds in 0–8640000000000000. All source and revision IDs are recomputed with the domain identity functions; source text passes the existing UTF-8/line/byte analyzer limits. Each capability has one unbranched immutable chain of 1–32 revisions, terminating at its declared head; its first revision equals its referenced original span. No orphan capability, revision, or review is accepted. Recomputed content hashes must equal all declared hashes. The domain identity algorithm and reference fixtures are in `crates/rangoon-domain/src/capability.rs` and `crates/rangoon-domain/src/lib.rs`; the portable format does not grant execution authority.

The empty manifest is exactly `{"version":1,"sources":[],"capabilities":[],"revisions":[],"reviews":[]}`. Its length is encoded before it and the SHA-256 trailer follows immediately. Golden-byte and independently constructed decoder fixtures test this wire format separately from database layout.

## States and failure handling

Workspace UI distinguishes loading, empty, unavailable, prior results after failure, native picker pending, backup exported, restore preview, blocked deletion, confirmation, mutation pending, stale preview and failure. A browser without a native bridge disables actions and says data is unavailable. It must not show a fabricated empty workspace.

Mutations and pickers share the existing native busy guard. Pending operations cannot be cancelled by navigating to another page or clearing analysis. Successful restore refreshes both libraries while retaining current editor drafts. Successful source deletion preserves the active in-memory analysis but makes it unsaved. Successful skill deletion clears that skill's active/draft state and updates source coverage; the confirmation discloses any unsaved draft loss. Failure preserves existing records, current analysis and drafts. Focus returns to the initiating control or an appropriate surviving heading, and status is announced.

## Data, privacy, and permissions

Backups and the workspace are unencrypted. Only explicit native picker selections authorize source/destination paths. The host rejects final symlinks/reparse points, directories, special files and oversized input, checks opened-file metadata, bounds reads, and uses exclusive creation for export. No absolute path is returned to the renderer or fixed errors. Parent-directory custody, cloud-synced user destinations, privileged local users and power-loss durability on every filesystem are outside the current claim.

## Compatibility and migration

No new workspace schema is needed. Schema 1 exports source-only data. Restore initializes a missing store or uses existing schema 1/2; it migrates to schema 2 only if adding a capability, within the same transaction as restored records. Existing schema 2 remains schema 2 even if all skills are deleted. A failed restore leaves previously committed records intact. It may leave an empty newly created database. Workspace data and restore previews can inspect that verified empty file without initializing it; explicit Save/Restore initializes it. Newly initialized files use 4 KiB pages, matching canonical backup validation. Existing noncanonical empty SQLite files are rejected rather than vacuumed or replaced; populated schema 1/2 stores keep their existing supported page size and additive restore remains subject to their allocation limit. Older binaries still reject schema 2. Unknown backup versions and workspace schemas remain unavailable and are never replaced.

## Acceptance mapping

1. Deterministic backup and fresh-workspace restore preserve BOM/CRLF source bytes, all revisions, local reviews, IDs and timestamps. No authority elevation occurs.
2. Restore preview counts match the transaction; existing skills remain unchanged; fresh preview after restore shows no additions; original confirmation retries are stale; stale preview, quotas, disk-full and interrupted restore preserve committed records.
3. Malformed lengths, truncated/trailing data, excessive counts, bad checksum, duplicate IDs, altered source/revision identities, broken lineage and forged review fields are rejected before any destination mutation.
4. Referenced source deletion is blocked; skill deletion removes only its owned history/reviews; subsequent source deletion succeeds; stale confirmation and failed/interrupted deletion preserve committed data.
5. Host file tests cover bounded regular-file reads, final link/reparse rejection, exclusive destination creation and write failure. No original input file or unrelated workspace is modified.
6. UI tests cover exact native arguments, unavailable states, cancellation, preview invalidation, draft retention/clearing, keyboard focus and confirmation. Isolated unsigned macOS QA covers native backup/restore and deletion/reopen states. Explicit synthetic populated dark/light responsive checks pass at 320/768 pixels; they use fixtures using the production controller/view/styles, with no DB or native bridge.
7. Fresh independent storage/native and UI review, named local tests, exact-head CI and publication receipts precede main merge. All-three-OS GUI, packaging and release gates remain separate.

## Non-goals and open questions

No encryption/key management, secure erase, automatic backups, scheduled retention, selective archive import, revision-history merging, multi-project roots, source watching, cloud synchronization, permissions engine, telemetry, provider, arbitrary database import, installers or release claim. These controls establish owner-managed local recovery; project ownership/reimport and richer composition continue in later stages of the full v1 objective.
