# Model proposal inspection and attributed application

Authority: the accepted M1d product scope in [intent.md](intent.md) and [model assistance](model-assistance.md). This contract defines the controller's implementation sequence. Evidence and publication state belong only in [development.md](development.md). It does not declare M1d complete.

## Product outcome and sequence

An operator selects a model suggestion, inspects its exact authored text against the selected source evidence, then explicitly creates an unreviewed skill or proposes a revision to an existing skill. Attribution survives save, restart, backup/restore, comparison and subsequent manual edits. Suggestions cannot mark content reviewed, grant authority or execute work.

The existing native workbench already displays validated advisory results. The missing pieces are native retained-result identity, selectable source comparison, durable model derivation, and application with conflict handling. Deliver them in this order:

1. **M1d-1, pure inspection foundation:** construct a bounded, immutable inspection from an existing `ContextPack`, its unforgeable `ValidatedResponse`, and an explicit proposal index. Preserve all selected evidence and its scope/coverage. This adds no native command, persistence or application control.
2. **M1d-2, native retained inspection:** bind the pure inspection to adapter/request identity, configured and observed model identity, outer response digest, native generation and saved dependencies. Retain only the completed evidence needed for inspection; never retain a credential envelope or transport operation to keep a result alive. Provide ID-only selection and actual workbench comparison. Stale evidence remains visibly inspectable, while application remains disabled.
3. **M1d-3, durable derivation and recovery:** version the capability/origin/revision and backup contracts before storing model-derived output. Add validated model derivation records and dependency edges, transactional migration, recovery and deletion rules. Existing source/composition identities and original bytes remain unchanged.
4. **M1d-4, explicit application:** use a native-owned inspection handle, selected proposal and explicit destination. Re-resolve all exact dependencies and destination revision; preserve any unsaved draft. Create new unreviewed content only after explicit review of the proposed change. Then qualify real IPC, restart/recovery and all three desktop GUIs.

These are implementation stages within the accepted feature, not replacements for full application. No partial stage completes M1d or V1.

## M1d-1 API and output contract

The pure API is `inspect_proposal(&ContextPack, &ValidatedResponse, proposal_index: u32) -> Result<ProposalInspection, InspectionDiagnostic>` in `rangoon-model-assistance`. Inputs are already bounded and validated by the existing constructors. `ValidatedResponse` and the inspection remain private-field, serialize-only types; callers cannot construct them by decoding arbitrary renderer JSON. The function checks response pack identity and task against the supplied pack, and rejects an out-of-range selector. It performs no I/O, network call, freshness claim, destination selection or mutation.

The closed output schema is `rangoon.proposal-inspection.v1`. Serialize fields in this fixed order:

- `schemaVersion`, then `inspectionId`;
- `packId`, `packBodySha256`, `responseSha256` (the exact inner proposal JSON digest, distinct from the adapter's outer response digest);
- `task`, `templateVersion`, `templateSha256`, `profileId`, `profileSha256`, `configuredModel`, `maxOutputTokens`;
- `proposalIndex`, `proposalCount`, `proposal` (the existing validated kind, title, authoredText, explanation and citations), `uncertainties`;
- `inputs` and `blocks`, exactly the existing context-body input coverage and deduplicated source-block representation;
- `selectedBytes`, `uniqueTextBytes`, `omittedBytes`, `tokenAccounting: "unknown"`, `contentKind: "model_authored"`, `authority: "none"`.

Inputs preserve tagged source or capability/revision identity, scope, full-input byte length, required/protected/selected ranges and omitted-range complements. Blocks preserve exact selected UTF-8 text and every alias's input index, byte range and protection flag. They do not include omitted source bytes. A citation must resolve to its original input and byte range; equal text under two identities never merges provenance. The inspector does not infer semantic equivalence or claim that a citation entails the generated assertion. Even byte-identical authored output remains model-authored.

`inspectionId` is `inspection:` plus lowercase SHA-256 of `rangoon.proposal-inspection.v1\0`, the unsigned big-endian 64-bit length, and the compact UTF-8 JSON of the same field sequence with `inspectionId` omitted. It binds the selected output, all selected evidence, coverage, uncertainty and configured pack metadata. Order matters; there is no map-key sorting or whitespace normalization. Hashes provide deterministic internal correlation, not authenticated model authorship or tamper protection.

Maximum serialized inspection size is 512 KiB including its ID. Use a capped writer before returning an inspection; never truncate evidence to meet the cap. Closed diagnostics are `inspection_mismatch`, `proposal_not_found` and `inspection_limit`, with no echoed source or response. Existing pack, proposal, template and response validation behavior and wire bytes remain unchanged. No deserializer or durable import path is added by this stage.

An inspection contains selected source text and therefore remains sensitive user content. Keeping credentials and endpoint headers out of the DTO is not a claim that arbitrary selected source is secret-free. No default logging, telemetry, clipboard write, file export or cloud upload is permitted. Session retention in the next stage has a separate bounded lifetime and explicit clear behavior.

## Native and adapter integration requirements

Local and cloud adapters already validate inner proposals before returning `Completion`; do not reparse an opaque serialized completion or ask the renderer to reconstruct provenance. Their later inspection accessors must compare the completion's request/profile/pack identity with the retained prepared request and use its internal typed validated result. The native wrapper adds the adapter version, request ID, outer response digest and observed model label as provider-reported data. Never substitute that label for the configured model or present it as attestation.

The native completed-result record must not contain credentials, credential revisions, OS-store snapshots, authorization headers, transport clients or live operation leases. It may contain bounded exact selected content and validated proposals. Capture it only through a matching live generation/operation, after response validation and the post-read freshness check. Cancellation, clear, reconfiguration, replacement and process exit invalidate its handles. A stale or unavailable dependency status cannot become current because a renderer echoes metadata. Inspecting a retained stale result is read-only; it never authorizes later application.

## Durable application and recovery requirements

Before enabling application, define versioned model derivation records carrying the native-bound configured/observed model labels, adapter/request identity, task/template/pack/inner-and-outer response digests, selected proposal and citation edges. Persist the complete immutable versioned inspection, or an explicitly equivalent lossless representation: every input identity and scope, selected/protected/required/omitted ranges, selected source blocks and all aliases, the selected proposal and its citations, uncertainties, and pack/template/profile/response metadata. A digest or citation edge alone is insufficient to reconstruct the selected context after restart. Never persist omitted source text through this inspection record. Validate inspection identity and all equivalent representation invariants on readback and backup restore. Preserve both model-authored bytes and any subsequent operator-edited draft bytes with separate hashes and parentage. A source/composition origin cannot silently stand in for model derivation. An existing skill keeps its original birth identity while a new model-derived revision points to the derivation/application record.

Migration must be transactional and readback must validate record identity, references, ranges and byte digests. Backup/restore must retain derivations and their dependencies, reject unknown/incomplete versions, and detect same-ID/different-content conflicts without partial writes. Deletion must show and enforce dependent derivation/revision edges. Old backups remain readable according to an explicit compatibility contract. Plaintext-at-rest remains disclosed until a separately qualified encryption design exists.

Native application binds an explicit new-skill or append-revision destination, current expected head and draft identity. Changed/deleted inputs, changed targets, replayed handles, concurrent edits and cancellation fail closed without overwriting work. Applying creates unreviewed content; previous reviews never carry forward. Classification and comparison findings are inspection-only until an explicit conversion contract exists; decomposition capability suggestions can be eligible only when the durable application contract and UI are implemented.

## Experience and acceptance

Keep the shared compact header, route artwork, selection/work area/inspector hierarchy and light/dark themes. Add proposal selection inside Model Assistance rather than a competing top-level product. The inspector separates authored proposal, exact cited source, wider selected context, uncertainty, omitted ranges and native status. The full proposal and full selected blocks/context remain available as inert text; omitted source content is unavailable through this inspector. Model-supplied HTML, Markdown links and commands never become navigation or actions. No fabricated progress, counts, readiness or review state.

M1d-1 tests cover deterministic identity, independently calculated digest framing, distinct task/template/model/profile/input/response/selector identities, mismatched pack refusal, selector boundaries, empty valid proposal lists, UTF-8/BOM/CRLF, scoped equal-text aliases, partial selections and omitted/protected ranges, hostile inert authored content, exact response binding, and capped serialization. Existing M1a golden vectors must remain unchanged. Run the pinned portable Rust validator and fresh independent review. Later stages additionally require cancellation/generation/replay tests, stale/deleted dependencies, destination conflicts, recovery/backup round trips, rendered keyboard/accessibility states and real all-three-OS IPC receipts. Source-only evidence cannot close those runtime gates.

Current M1d-1 owned files: `crates/rangoon-model-assistance/src/{lib.rs,pack.rs,response.rs,inspection.rs}`, `crates/rangoon-model-assistance/tests/inspection.rs`, this contract, and the model overview/continuation/development records. No database, native command, provider, UI control or release mutation is part of the pure foundation. The controller owns architecture, integration and publication; a bounded worker owns only the five named Rust source/test files, followed by fresh read-only review.
