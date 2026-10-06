# Native Compile inspection

Authority: the accepted V1 continuation in [intent.md](intent.md), implementing the next bounded stage of [compilation.md](compilation.md). The primary controller owns native integration, validation and publication; Jeff retains product and release authority. Publication and qualification receipts belong in [development.md](development.md).

## Outcome and scope

The desktop operator selects a saved skill, an exact current or historical revision, and one closed text profile. An explicit Compile action resolves that selection from the validated local workspace and displays generated text, ordered diagnostics, provenance, review observation and deterministic identities. It never changes source, reviews a revision, exports a file, installs instructions, runs a harness or contacts LNSAT. Browser-only operation has an explicit unavailable state.

This stage implements the native inspection command and workbench source. Native GUI/IPC qualification remains pending and separate from source tests and synthetic browser checks. Portable export, SKILL/support-file profiles, target-loader qualification and full R4/R5 remain open. The existing local database and local review annotations remain unauthenticated and unencrypted. Displayed hashes establish no new authentication guarantee.

## Native boundary

Add one local-main-window command, `compile_capability`. Its body is raw UTF-8 JSON, limited to 4,096 bytes before typed deserialization. Tauri already receives the transport buffer; this limit is not a transport-level memory cap. Reject object-form IPC, invalid UTF-8/JSON, duplicate or unknown fields, missing fields, unknown schema/profile and malformed identifiers.

The closed request contains exactly `schemaVersion` (`rangoon.compile-request.v1`), `capabilityId`, `revisionId` and `profile` (`agents_md_v1` or `claude_md_v1`). Both IDs are required; no current-head fallback is permitted. The renderer passes `TextEncoder().encode(JSON.stringify(request))` as a `Uint8Array` to `invoke`; object or string IPC is invalid. The renderer supplies no source body, review flag, requirements, path or manifest. Existing list/open capability commands provide selection data without changing the source-analysis session or Skills drafts.

Compilation participates in the existing native operation guard. While another guarded operation runs, return the fixed `workspace_busy` error without invoking the store. Hold the guard until the blocking task completes. Resolve content through `Workspace::compile_capability`; no host-retained confirmation handle is needed because no write or later action is authorized by a report.

Successful response fields are `outcome: compiled`, `schemaVersion: rangoon.compilation-inspection.v1`, `observedCurrentHead`, `compilation` (the exact existing report), and `candidateManifestJson` (the exact canonical UTF-8 manifest string for a candidate, otherwise `null`). Generate this string with the compiler's canonical serializer, not a renderer reconstruction. Failures use `outcome: failed` with fixed public `code`/`message`; no raw parser, path, SQLite error or source echo. Unexpected join/serialization failures remain unavailable. Invalid requests use the existing bounded `compilation_invalid` store error.

## Controller and state

The Compile controller owns an independent library, selected detail, profile and result. It reuses capability validators for list/detail reads, validates result shape and selection/profile correlation, and rejects unsupported authority/readiness/qualification values. For candidates, parsed `candidateManifestJson` must equal the report's manifest and Web Crypto SHA-256 of its exact UTF-8 encoding must equal `manifestSha256`; reject mismatch or unavailable digest computation. Noncandidate responses require exact `null`. Recompute the versioned domain-framed compilation identity for every report and the candidate identity when present; syntactically valid digests alone are insufficient. Reject absent or failing Web Crypto for every report. Recheck the request epoch after asynchronous verification. These are internal consistency checks, not authentication. The host remains the source of stored-content validation; renderer checks do not independently qualify a target.

No skill or profile is selected implicitly. A selected capability opens its observed head for inspection; a history control explicitly selects another exact revision. Any changed capability, revision or profile invalidates the prior report. Generate only on explicit action. Every asynchronous operation captures an epoch; a newer selection, refresh or known workspace mutation prevents old responses from restoring a stale candidate. Pending controls are disabled and direct controller calls cannot bypass that state.

Refresh and failures retain prior usable data with clear stale/error labels. Known workspace writes mark retained results stale and disable Compile until explicit refresh resolves the selection again. Refresh preserves an existing exact revision when still present; it never silently switches to a new head, reruns compilation or promotes a prior report. A missing selection is cleared with a visible message. External edits outside the current process are detected by the store on the next read; the UI does not promise live filesystem monitoring.

The renderer distinguishes native bridge unavailable, loading, no skills, no selection, no profile, pending, error, stale, blocked, review required and static candidate. Blocked reports retain inspectable text. Candidate means locally reviewed static output only: runtime remains untested, budget unknown, semantic equivalence unverified and authority none. Historical selection is valid and distinct from the separately observed current head.

Opening the selected revision in Skills is navigation/readback only. It preserves existing unsaved Skills drafts and never implicitly reviews or revises content. Compilation keeps the source-analysis report, composition draft and workspace confirmation state untouched.

## Workbench design

Use the existing dense dark/light shell, custom bundle icon, orange selection accents, cards and monospace identity fields. Add `#compile` to the real workbench rail. The desktop layout has a bounded selection column, flexible inspection area and evidence panel. At intermediate widths evidence stacks below; below 760 pixels all sections use one column without page overflow. Match the existing global rail collapse and reduced-motion behavior.

Selection provides saved skill buttons, exact revision history, two explicit profile controls, Refresh, Compile selected revision, and Open selected revision in Skills. Primary compilation stays disabled while unavailable, pending, incomplete or stale. No Copy, Export, Install, Run or target-check action appears in this slice.

Report tabs are Generated text, Diagnostics and Evidence. Generated text uses escaped, noneditable `<pre>` content with keyboard scrolling; it is an inspection view rather than a byte-verification or clipboard channel. Show the host's byte length and SHA-256. Diagnostics retain compiler order, code, severity and bounded detail; explain conservative Claude checks including false positives. Evidence displays profile facts/digest, selected and observed head IDs, parent/provenance, local review observation, compilation ID and candidate identity/manifest when present. Documentation URLs are inert text. All dynamic content is escaped, never rendered as Markdown/HTML or fetched remotely.

Route entry focuses the Compile heading. Selection/refresh controls retain focus; successful compilation focuses the generated-text heading. Tabs use a tablist with roving focus, Arrow keys and Home/End. Use a polite status announcement, one error announcement, visible focus and labeled controls. Preserve the existing forced-colors and reduced-motion rules. Long identifiers wrap; artifact/manifest text scrolls within its panel.

## Acceptance

- Native closed-request decoding rejects malformed/oversized input and renderer attempts to supply source/review/path fields. Local permissions expose only the new named command.
- Native readback preserves exact bytes, report identities, historical selection and review semantics; no database writes or analysis-session changes occur. Canonical manifest text matches its reported hash and is absent for blocked/unreviewed reports.
- Controller tests cover unavailable/empty/error states, explicit selection/profile, exact raw request bytes, response correlation, stale asynchronous results, known-mutation invalidation, historical selection and refresh/deletion recovery.
- View and binding tests cover escaping hostile content, candidate limits, ordered diagnostics, keyboard tabs/focus, draft-preserving Skills navigation and shared busy state.
- Rendered synthetic states cover dark/light at 320/768/1440 widths. Real macOS native IPC inspection is separate evidence; Windows/Linux GUI, installers and release qualification remain open until actually run.
- Named native/Node/repository validators and fresh independent source/UI review precede source publication. No source test is relabeled as runtime, security or release certification.
