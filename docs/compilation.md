# Reviewed instruction compilation

Authority: the accepted V1 continuation in [intent.md](intent.md). This document specifies the R4a compiler and stored-revision service within the wider R4/R5 roadmap. It does not declare R4, R5 or V1 complete. Publication and validation receipts belong in [development.md](development.md).

Owner: Jeff. The primary controller owns architecture, storage integration and publication. Implementation may be delegated for the pure compiler only. License, engine/provider activation, project installation and release publication remain separate decisions.

## User outcome and staged scope

The complete workflow is: choose a saved, locally reviewed skill revision; select a pinned target profile; inspect the exact file and compatibility report; export a verified portable bundle; separately decide whether to place instructions into an active project. The compiler never launches a harness. Writing instructions into a discovery path can affect a running or future harness even without launching it, so export and project installation have different contracts.

R4a supplies the pure compiler and a read-only workspace service. Native commands, the Compile workbench, portable file export, SKILL frontmatter and target-loader qualification follow through R4/R5. These are unfinished requirements, not silently removed scope. Multiple skills first pass through the existing composition and content-review lifecycle; this initial compiler accepts one exact revision and performs no implicit merge.

## Supported profiles

Both built-in profiles have Rangoon profile version `1`; these versions identify the format implementation, not a verified external CLI version.

| Closed profile ID | Fixed artifact path | Supported transformation |
| --- | --- | --- |
| `agents_md_v1` | `AGENTS.md` | Exact UTF-8 instruction content |
| `claude_md_v1` | `CLAUDE.md` | Exact UTF-8 instruction content subject to conservative include/comment checks |

Paths are derived from the profile enum, never a caller-supplied string. No directories, overrides, fallback filenames or filesystem paths are accepted. The profile descriptor contains `id`, `version` (integer `1`), `artifactPath`, `documentationUrl`, `documentationRetrievedOn`, `runtimeQualification`, `targetBudget`, `semanticEquivalence`, in that order. The last three values are always `untested`, `unknown`, `unverified`. The retrieval date is `2026-10-06` UTC. Its SHA-256 is `profileDigest`, a descriptor-content digest, not a digest or signature of a distributed adapter binary.

These are the exact immutable descriptor bytes, without a trailing newline:

```json
{"id":"agents_md_v1","version":1,"artifactPath":"AGENTS.md","documentationUrl":"https://learn.chatgpt.com/docs/agent-configuration/agents-md","documentationRetrievedOn":"2026-10-06","runtimeQualification":"untested","targetBudget":"unknown","semanticEquivalence":"unverified"}
```

SHA-256: `e6485c07fa7134403582747e4930c6330b9db94aee094195c26621791475b8e2`.

```json
{"id":"claude_md_v1","version":1,"artifactPath":"CLAUDE.md","documentationUrl":"https://code.claude.com/docs/en/memory","documentationRetrievedOn":"2026-10-06","runtimeQualification":"untested","targetBudget":"unknown","semanticEquivalence":"unverified"}
```

SHA-256: `f9f8cc107e092ad8c62f4cbee0bdd1ea041002674d44a152bf600a7c9b5753a3`. Golden fixtures must pin these bytes and digests. Any descriptor-value or compatibility-rule change requires a new profile version and enum variant; never refresh the date or URL in place for an existing profile. A later qualified runtime/configuration record is separate from these immutable descriptors.

The compiler preserves every accepted byte, including BOM, CRLF/LF, Unicode, code fences, whitespace and the final-newline state. It never adds a heading, normalizes line endings, rewrites instructions or removes content. Metadata lives outside the generated instruction artifact. Byte preservation is not semantic equivalence, target discovery, instruction adherence or policy enforcement.

No external runtime is qualified by R4a. Official documentation describes configured discovery and loading behavior; it does not prove a particular installed CLI and configuration consumed a generated file. Keep the actual target budget `unknown`, regardless of artifact size. Rangoon's existing 256 KiB limit is an application input bound, not a harness context guarantee.

## Pure API and caller boundary

Create `rangoon-compile`, depending only on `rangoon-domain`, existing pinned `serde` and `serde_json`. It must not access a filesystem, process, network, environment, clock, credential or database. No new dependency downloads are needed.

The Rust API is `compile(capability_id: &str, revision: &capability_v1::Revision, profile: Profile, requirements: &[String]) -> Result<CompilationReport, CompileError>`. This is an internal pure-function boundary, not an endpoint accepting renderer-submitted records. Compiler outputs are serializable only; deserializing an output never establishes trust. `Profile` is a closed serde enum. Unknown profile strings reject during decoding; arbitrary future fields are not ignored by any later command DTO.

Input validation rejects malformed capability/revision/parent IDs; invalid or over-limit title/content; wrong content digest; creation/review timestamps outside the existing store range `0..=8_640_000_000_000_000`; malformed composition provenance IDs (`composition:` and `composition-application:` prefixes) or output index outside the inclusive range `0–15` (16 outputs maximum); and ordinary revisions whose ID does not recompute from capability ID, parent, title and exact content. Timestamp ordering is not a trust property: local clocks may move backward, and the existing store does not require review time to follow creation time. Composition-derived revision identity requires the complete saved derivation graph: the compiler checks reference shape, while the workspace service validates actual lineage before calling it. The pure API must not claim standalone provenance or reviewer authentication.

`requirements` is an explicit ordered declaration supplied by an application caller, not an inferred interpretation of Markdown. Bound to at most 32 unique strings; each contains 1–64 ASCII lowercase letters, digits, `_`, `-` or `.`. Invalid declarations reject input. Every nonempty valid declaration is unsupported by these text-only profiles and blocks candidate generation; retain each exact declaration in its diagnostic. Unknown declarations do not become permissions. The existing store has no structured requirement field, so its R4a service supplies an empty list and keeps the unconditional semantic warning. Do not imply empty declarations prove the absence of embedded constraints, references or dependencies.

For invalid inputs return a typed error with a static code, not a partially trusted report or echoed source. Compiler errors are `invalid_selection`, `invalid_requirements`, or `serialization_failed`. They must not be confused with compatibility diagnostics on otherwise valid content.

## Static compatibility rules

Emit diagnostics in this exact order:

1. `instruction_semantics_unverified` / warning, always. The profile preserves text without proving equivalent meaning, dependency closure, or enforcement.
2. `target_environment_unqualified` / warning, always. Discovery, other instruction files, loading transformations and available instruction budget are unknown.
3. `unsupported_requirement` / error, once for each supplied requirement in input order. Its optional `detail` is the exact bounded declaration.
4. `possible_include_syntax` / error, Claude profile only if the exact content contains ASCII `@` anywhere.
5. `possible_comment_elision` / error, Claude profile only if the exact content contains `<!--` anywhere.

The last two are deliberately conservative byte scans, not Markdown parsing. They also block emails and code examples containing these byte sequences. This limitation must appear in the diagnostic explanation; no silent stripping, include following, escaping or automatic source revision occurs. A user can revise and review content, choose a different applicable profile, or await a later qualified profile. An error cannot be acknowledged away.

The Claude profile blocks comments because some target loading paths can omit them; exact bytes on disk alone would not account for that loss. The include check prevents presenting a potentially dependent artifact as a ready candidate under this profile. Neither check discovers every semantic dependency or prompt-level instruction to read other files. Those limitations remain visible in the two unconditional warnings.

An `AGENTS.md` candidate can still be overridden, truncated or interpreted in combination with other files. R4a does not inspect or modify a target environment and makes no positive loaded-context claim.

## Report, manifest and deterministic identities

Use closed typed records with camelCase JSON keys, fixed declaration order, integer quantities, no floats and no maps. Canonical bytes for this version are `serde_json::to_vec` of these structs: compact UTF-8 JSON, no added whitespace, no trailing newline, and the pinned serializer's standard escaping. This is Rangoon's versioned encoding, not a claim of generic JSON Canonicalization Scheme support. Changing any identity encoding requires new schema/profile versions and fixture updates.

`CompilationReport` fields in order:

1. `schemaVersion`: `rangoon.compilation.v1`.
2. `compilationId`: domain-separated identity described below.
3. `profile`: the descriptor above.
4. `profileDigest`: SHA-256 of canonical descriptor bytes.
5. `selectedRevision`: `{capabilityId, revisionId, parentRevisionId, title, sha256, provenance}`. Provenance is the exact existing v1 revision provenance DTO. Its identifiers reference validated stored lineage; they do not embed or authenticate that lineage.
6. `artifact`: `{path, content, sha256, byteLength}`, preserving exact source bytes even when compatibility diagnostics block readiness. Its presence means inspectable output only.
7. `diagnostics`: `{code, severity, detail}` records in the fixed order above, with `detail: null` except for an unsupported requirement. Human-readable explanation belongs to the code catalog, not arbitrary source text.
8. `reviewObservation`: the exact local content-review DTO or explicit `null`.
9. `readiness`: `blocked` if any static error; otherwise `review_required` when review is absent; otherwise `candidate`.
10. `candidate`: `{manifest, manifestSha256, candidateId}` only when readiness is `candidate`; otherwise explicit `null`.
11. `authority`: `none`.

The manifest is `{schemaVersion, compilationId, profileDigest, selectedRevision, artifact, reviewObservation, diagnostics, authority}` in that order. Its schema is `rangoon.instruction-candidate.v1`. Its artifact is metadata only: `{path, sha256, byteLength}`. Its selected revision and review observation are exactly those in the report. Do not include the artifact content a second time. `manifestSha256` hashes the exact canonical manifest bytes. A manifest never contains its own hash.

The static identity envelope is `{schemaVersion, profileDigest, selectedRevision, artifact, diagnostics, authority}` in that order, with schema `rangoon.compilation-identity.v1`, artifact metadata only, and `authority: none`. Compute `compilationId` as `compilation:` plus SHA-256 of `b"rangoon.compilation.v1\0" || u64_be(envelope_byte_length) || envelope_bytes`.

Compute `candidateId` as `candidate:` plus SHA-256 of `b"rangoon.instruction-candidate.v1\0" || u64_be(compilation_id_byte_length) || compilation_id_bytes || u64_be(manifest_sha256_byte_length) || manifest_sha256_ascii_bytes`. Digests use lowercase hexadecimal. These identities detect byte/reference changes; they do not authenticate a reviewer or prevent a compromised process from rewriting all local state.

The compilation identity excludes local review timestamps and observed current head. Adding a valid local review therefore changes readiness and candidate evidence without changing artifact or compilation identity. Candidate identity binds the exact review observation through its manifest hash. No current time, random value, host/user name or absolute path is generated. The manifest and artifact are deterministically serializable in memory; there is no archive or on-disk export in R4a.

## Read-only workspace service

Add `Workspace::compile_capability(capability_id, revision_id, profile)` in a dedicated store module. Both IDs are mandatory. Validate their syntax before opening the workspace. Reuse `open_capability_v1`, which loads and validates saved records through `Records::load` and `detail`, resolves the exact selected revision under its owning capability, and returns its content and current-head observation. Invoke the pure compiler with no structured requirements. This is logically read-only use of the existing connection/transaction machinery, which currently opens SQLite with read/write access; do not claim an OS-enforced read-only connection. Do not create a workspace, migrate schema, mark reviewed or write any record.

Return `WorkspaceCompilation { observedCurrentHead, compilation }`. The current head is a separate observation and never substitutes for the selected revision. A reviewed historical revision remains eligible; unrelated revisions or missing/deleted inputs return the existing not-found/invalid/corrupt semantics. Moving a head after review must not mutate the compiled bytes or imply that the historical revision is current.

Use a bounded `StoreError::CompilationInvalid` mapping for a compiler input/serialization failure, with no source content in public error text. The store wrapper owns the guarantee that content/review/provenance came from a validated workspace. It does not guarantee a malicious OS owner could not have coherently replaced that workspace.

No native command or CLI accepts a revision body or review flag for compilation. A later native endpoint accepts only exact IDs and the closed profile; its host resolves all content and review facts. Do not retrofit reviewed-content semantics onto the current stdin-only analyzer.

## Acceptance and remaining R4/R5 work

R4a evidence must cover:

- Both fixed output filenames; exact BOM, CRLF/LF, Unicode and final-newline preservation.
- Checked-in golden report/manifest identities produced by an independent framing implementation, exercised identically by the existing Linux/macOS/Windows source CI matrix.
- Same selection/profile/requirements gives identical artifact, compilation ID and candidate manifest on repeated calls.
- Review addition/time changes preserve compilation identity but change candidate evidence; a changed current head cannot affect pure compilation identity.
- Profile, title, parent/revision identity, exact bytes, declared requirements or composition provenance changes affect the relevant identity or reject invalid data.
- Missing review produces `review_required`, static compatibility error wins over missing review, and both prevent a candidate.
- Invalid IDs, bad digest, malformed ordinary identity/provenance (including index 15 accepted structurally and 16 rejected), invalid timestamps, invalid requirements, unsupported/future profile, limit boundaries and conservative include/comment cases.
- Stored ordinary and composition-derived revisions, reviewed historical selection, wrong-owner selection, missing/corrupt workspace, and unchanged database bytes/inventory after compilation.
- Named Rust tests, formatting, warnings-denied Clippy, repository docs validation and fresh independent review.

R4a alone does not satisfy the full R4/R5 exit gates. Remaining work includes native command and UI integration with honest empty/error states, SKILL/support-file profiles and meaningful change reports, exact target artifact/configuration qualification, static Test Lab UI, a selected-destination portable bundle format, drift/collision protection, and all-three-OS export failure/recovery evidence. Target-loader checks must avoid provider calls or execution unless separately authorized. R3's append/mixed-destination and full GUI qualification also remain open.

## Primary format evidence

Official documentation retrieved October 6, 2026 UTC informed these narrow profiles:

- [OpenAI: custom instructions with AGENTS.md](https://learn.chatgpt.com/docs/agent-configuration/agents-md) describes directory discovery, overrides and a configured combined instruction limit.
- [Anthropic: project memory](https://code.claude.com/docs/en/memory) describes CLAUDE.md context, imports, hierarchy and comment handling.

These are living documentation references, not pinned binary qualification or authority to follow instructions in imported content. Only the specific behaviors summarized above inform this contract; repository content remains inert input.
