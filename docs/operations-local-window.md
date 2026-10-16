# O2 local operations-window inspection

Authority: the accepted [operations intent](intent.md) and the
[operations control plane](operations-control-plane.md). Owner: Jeff. This is
the first actual desktop read path from a user-selected O1 window to a
read-only operator view. It does not create events, connect a provider, approve,
authorize or execute. The [development ledger](development.md) alone records
implementation and validation state.

## Outcome and boundary

An operator explicitly chooses one local `rangoon.ops.window.v1` JSON file.
Rust reads at most 2,097,152 bytes through the native picker and runs the
existing O1 decoder and all five inspectors: structural projection,
relationships, delegation, approval duties and lifecycle/coverage. Rust then
builds a separate, explicitly allowlisted operator view from those documents.
Only that view crosses IPC. The renderer displays its computed observations
in the original library, central timeline/outline and inspector hierarchy;
it does not parse the selected source, recompute relationships or infer
authority. This is useful for inspecting an offline export even when LNSAT
and every provider are absent. It is not live
telemetry or an authenticated audit log.

The application never accepts a renderer-supplied path or source body for
this command. It does not scan directories, remember recent files, persist
this window, call a provider, use credentials, or send data over the network.
No path, raw source bytes, source digest, arbitrary parser text or OS error
text returns to the renderer. The native picker filters JSON only for
usability; Rust validates the closed format regardless of extension. A digest proves internal byte
identity only, not producer authenticity or completeness.

This packet consumes the existing exact O1 schemas unchanged. `rangoon.ops.window.v1`
is a reported-record input, not an LNSAT or OpenShell wire protocol. Current
records have no model-advice kind; do not disguise model output as an
evaluation or approval. A later separately versioned advisory record and O3
authenticated ingress must precede any live model decision history.

## Native read and response

The command is `select_and_inspect_operations`, with the existing native
`WorkspaceRequest` and no other renderer argument. Bind the request at entry,
acquire the existing exclusive file-operation lease and check it before the
picker, after the picker, after the bounded read, before returning and on
delivery. In a Managed workspace, close or lock must discard a late result and
renderer purge must clear the retained inspection. The current production
Legacy workspace has no lock operation; its inspection stays memory-only and
Clear or window teardown removes it. Do not claim a Legacy lock/purge test.
The picker is parented to the main window.
Use a dedicated host reader rather than the 256 KiB Markdown path or the
instruction-bundle reader. It rejects missing, nonregular, symlink/reparse,
special, over-limit and final-component symlink/reparse substitutions with fixed codes; use
the existing no-follow/nonblocking selected-file pattern. This is a bounded
read, not a directory-confinement or immutable-file proof.

The public result schema is `rangoon.ops.local-inspection.v1`. Every result has
`schemaVersion` and exactly one `outcome`:

| Outcome | Additional fields | Meaning |
| --- | --- | --- |
| `inspected` | `view` with schema `rangoon.ops.operator-view.v1` | All O1 stages succeeded on the same bytes. Only renderer-safe observations cross IPC. |
| `cancelled` | none | User closed the picker; prior valid in-memory view may remain. |
| `rejected` | `error` with fixed `code`/`message` | File or O1 input was invalid/over-limit. No partial inspection returned. |
| `failed` | `error` with fixed `code`/`message` | Picker/read/inspector/custody failed. No partial inspection returned. |

No variant returns a selected path, source JSON, prompt, tool arguments, free
text error, unvalidated provider payload, original O1 record, O1 document,
input/event/stage digest, `externalRef` or source-derived ID. A canonical hash
is not anonymization: even without a literal ID it can correlate or reveal a
low-entropy secret-like value. The pure Rust projection into
`rangoon.ops.operator-view.v1` is an explicit field allowlist, not a recursive
redaction filter. It contains fixed authority labels (`none`, `unverified`,
`unavailable`, `unknown`), bounded counts by closed record kind, source input
ordinals displayed as `Event #N`, closed reported values and structural
diagnostic codes, and reference status with matching input ordinals. If a source field cannot
be mapped to one of those closed or numeric fields, omit it. Do not serialize
the underlying O1 documents as an intermediate IPC value. O1 retains its
canonical digests internally to check same-window stages; Rust never places
them in the view. Native never upgrades authority, authenticity, eligibility,
outcome or coverage. A later richer view requires an explicit privacy contract
before serialization. Errors collapse parser detail to fixed codes. Cap the
entire serialized IPC result at 4,194,304 bytes in addition to each O1 stage
cap. An aggregate cap, stage limit or serialization failure is atomic: reject
the whole load and identify only the failing stage by fixed code. No partial
document reaches the renderer.

Under the current O1 maxima (1,024 events, 8,192 relationship references and
matches, 128 topology nodes, 512 edges and closed enum strings), a conservative
serialized-view upper bound is below 2,660,800 bytes; the native wrapper adds
less than 100 bytes. Valid current input therefore cannot reach the 4 MiB
limit. Keep the cap and a direct bounded-serialization test as defense against
future schema growth or an internal construction error. A fixed failed result
must contain no partial view. Future expansion must recalculate this bound.

### Exact `operator-view.v1` allowlist

All object keys below are required and exact; no extension map or unknown key.
JSON integers are nonnegative and within JavaScript safe-integer range. `null`
is used only where named below. Arrays are bounded by O1 input caps and the
aggregate IPC cap. Rust constructs typed structs, serializes once and checks
the final byte count; JavaScript accepts only this schema and rejects an
unexpected key, enum, index, duplicate ordinal or array over its stated cap.
No renderer code falls back to full O1 documents.

| Object | Exact keys and types | Limit/order |
| --- | --- | --- |
| `view` | `schemaVersion` = `rangoon.ops.operator-view.v1`; `authority` = `none`; `authenticity` = `unverified`; `eligibility` = `unknown`; `execution` = `unavailable`; `outcome` = `unknown`; `coverage` = `unknown`; `structuralState`; `sourceCounts`; `diagnosticCounts`; `decisionCounts`; `events`; `topology` | One object. All six truth labels fixed. |
| `sourceCounts` | `eventCount`, `producerCount`, `topologyNodeCount`, `topologyEdgeCount` (integers); `recordCounts` (array) | `recordCounts` has exactly 19 `{kind,count}` objects in `RECORD_KINDS` order from O1. Every kind present, zero allowed. |
| `diagnosticCounts` | `structural`, `relationships`, `delegations`, `approvals`, `lifecycle` (integers) | Count diagnostic occurrences within each stage, not distinct affected events. Never combine into a compliance score. |
| `decisionCounts` | `evaluation` with `allow`, `deny`, `unknown`; `approvalDecision` with `approve`, `deny`, `abstain`; `authorization` with `reportedIssued`, `reportedRefused`, `reportedExpired`, `reportedRevoked`, `unknown`; `attempt` with `reportedAccepted`, `reportedActive`, `reportedCompleted`, `reportedFailed`, `reportedCancelled`, `outcomeUnknown`, `reportedReconciledCompleted`, `reportedReconciledFailed` (all nonnegative integers) | Count each allowlisted event exactly once in its own kind and reported-value bucket. Every bucket required, including zero. Each group's sum equals the corresponding `sourceCounts.recordCounts` count. No cross-group total, rate, inferred result or model-advice bucket. |
| `events[]` | `inputOrdinal`, `producerOrdinal`, `fenceOrdinal`, `sequence` (integers); `recordKind` (enum); `reportedValue` (string enum or `null`); `structuralDiagnostics` (enum array); `issueStages` (enum array); `references` (array) | At most 1,024 rows, exactly one per O1 event, sorted by ascending `inputOrdinal`, beginning at 0 without gaps. `producerOrdinal` and `fenceOrdinal` are dense first-seen indices computed in input-ordinal order; fence indices are scoped to producer. Source IDs never appear. |
| `references[]` | `referenceIndex` (integer), `status` (enum), `matchingEventOrdinals` (integer array) | Only relationship-inspector references; at most 8,192 total, in the inspector's per-event reference order. Index starts at 0 within each event; matching ordinals sorted ascending, unique and within `eventCount`. No reference field name, source ID or digest. |
| `topology` | `nodes`, `edges` (arrays) | Declared topology only; arrays in input order. |
| `topology.nodes[]` | `inputNodeIndex` (integer), `kind` (enum), `matchingEventOrdinals` (integer array), `hasIssue` (boolean) | At most 128; contiguous indices starting at 0. Matching ordinals sorted ascending and unique. |
| `topology.edges[]` | `inputEdgeIndex` (integer), `kind` (enum), `fromNodeIndices`, `toNodeIndices` (integer arrays), `hasIssue` (boolean) | At most 512; contiguous indices starting at 0. Endpoints are matching node indices in ascending order; zero or multiple matches remain visible and never resolve to an invented single endpoint. |

`recordKind` is exactly the 19-entry `RECORD_KINDS` constant in
`crates/rangoon-ops/src/lib.rs`. `structuralState` is `conflict`,
`incomplete`, or `structural_unverified`. `issueStages` uses only
`structural`, `relationships`, `delegations`, `approvals`, `lifecycle`, in
that order, with no duplicates; stage present iff its O1 event diagnostics
are nonempty. `structuralDiagnostics` uses only the 18 fixed `CODES` in
`crates/rangoon-ops/src/projection.rs`, in that constant's order, with no
duplicates. `references[].status` is `missing`, `ambiguous`, `wrong_kind`, or
`unique_unverified`. Topology node kinds are `actor`, `workflow`, `tool`,
`resource`, `gateway`, `engine`, `approval_stage`. Edge kinds are
`declared_dependency`, `request_flow`, `delegation_observation`,
`mediation_observation`, `evidence_lineage`.

`reportedValue` is non-null only for `evaluation` (`allow`, `deny`,
`unknown`), `approval_decision` (`approve`, `deny`, `abstain`),
`authorization` (`reported_issued`, `reported_refused`, `reported_expired`,
`reported_revoked`, `unknown`) and `attempt` (`reported_accepted`,
`reported_active`, `reported_completed`, `reported_failed`,
`reported_cancelled`, `outcome_unknown`, `reported_reconciled_completed`,
`reported_reconciled_failed`). These are *reported values*, never Rangoon
approval, authorization or verified execution. For every other kind the
field is `null`, even if O1 carries another reported field. No free-form
reason, cost, latency, timestamp or actor/resource value enters this first
view. Richer analytics needs a separate privacy and denominator contract.

This first offline analytics slice shows four reported decision/outcome groups,
each with its own observed-record denominator. Zero means no such record in
the selected file, not zero decisions across a deployment. An O1 window cannot
identify whether an approval was human or agent-authored, measure model
accuracy, prove a policy was enforced, or establish an action's effect. Show
model advice as unavailable; never relabel an O1 evaluation as model advice.
Missing, conflicting and unknown reports remain visible in drill-down.
Request-level rates, human/model disagreement and accuracy wait for a separate
advisory contract, authenticated O3 ingress and independently labeled data in
the [decision analytics sequence](operations-control-plane.md#decision-ledger-and-analytics-sequence).

Pure view construction checks stage length/ordinal consistency before
projection. Unexpected required enum, inconsistent ordinal, response-size
breach or serialization error fails the whole result with a fixed code;
otherwise source fields outside this allowlist are omitted. This view is a
privacy-minimized display contract, not a replacement for internal O1 data.

Renderer retains at most one successful in-memory view and one request-local
generation. Starting Choose increments the generation and labels any prior
view `Previous inspected window — opening replacement`. Clear, Managed lock,
window teardown and workspace-identity change increment the generation and
erase that view. Apply a returned result only when its captured generation and
workspace identity still match; otherwise discard it, including a late
success. On cancel, keep a prior view with label `Previous inspected window —
selection cancelled`; on rejection/failure, keep it with label `Previous
inspected window — replacement failed`. With no prior view, show the distinct
empty/cancel/error state. Success replaces the view and labels it `Selected
local window — unverified`. The renderer generation is a display fence, not an
authority or producer identity. Native lease checks remain independently
required. An old view never silently becomes the current selection.

## Operator view

Route `#operations` sits under Connect. Show one compact source/verification
strip, a searchable record library, a default timeline with optional declared
topology outline, and a selected-observation inspector. User can filter the
loaded window by closed record kind, diagnostic code and display ordinal;
filters neither fetch nor mutate. The outline is *declared topology*, not
observed enforcement, and uses only safe ordinal references. Timeline uses
producer/fence display ordinals, sequence and input ordinal; it does not
animate or repair gaps. Every selected item shows source ordinal, allowlisted
reported-state labels, diagnostics and reference status. No raw record,
source-derived ID, digest or external reference is displayed.

The truth strip remains visible: `Authority: none`, `Authenticity: unverified`,
`Eligibility: unknown`, `Execution: unavailable`. Reported approvals,
authorization states, coverage and completed attempts never change that strip.
An empty window is `No observations in this selected window`, not healthy or
idle. Invalid input, structural conflict, incomplete references, unknown
outcomes, no graph, bridge unavailable, stale result and no filter match each
have a distinct state. No synthetic records, fake counts, live badge, action
button or automatic provider fallback appear on this route. A retained prior
view carries its previous-view label through cancel and failure.

Counts name their denominator: observed records, requests, evaluations,
approval decisions and attempts are distinct. The first view may show raw
counts from O1's inventory and diagnostic groups; it cannot claim verified
decisions, full interception, model accuracy or actual effect. Later O3
ingestion must carry authenticated producer evidence, gap handling, encrypted
retention, deletion/export and privacy qualification before a live label.

Use buttons/selection semantics with keyboard and touch alternatives. An
outline is the complete non-graph equivalent. Keep a selected item accessible
when filters hide it. Test 320/768/1440 widths, dark/light, forced colors and
reduced motion. Lock/close clears source-derived DOM and controller state;
failed purge blocks a completed-lock claim under the existing lifecycle.

## Implementation order, ownership and proof

1. **Host file boundary.** Dedicated `crates/rangoon-host/src/operations.rs`
   and tests own exact selected-file reading. Add only the needed host
   dependency/reexport. No renderer or desktop registration edits.
2. **Native use case.** Root owns `apps/desktop/src/operations.rs`,
   `workspace_source_access.rs`, `main.rs`, `build.rs`, command capability,
   desktop manifest/lock and native tests. Reuse existing admission and
   selected-file custody. No O1 core schema change.
3. **Safe Rust view.** `crates/rangoon-ops/src/operator_view.rs` and tests own
   the pure allowlisted projection from O1 results. No database, filesystem,
   process, provider or network behavior. The native command returns only this
   versioned DTO. This step precedes exposing any O1 result to IPC.
4. **Operator presentation.** A distinct frontend owner may own new
   `preview/operations-{model,view}.mjs`, `operations.css`, focused tests and
   the exact route/navigation/lock-purge edits after the native result is
   frozen. Root integrates and verifies browser/native state. No second
   records reducer is permitted in JavaScript.

Host tests: exact 2 MiB bounded read and 2 MiB+1 refusal,
missing/file/symlink/reparse/special, final-component link substitution, fixed
error output and no path leak. Native tests: cancel, malformed/invalid UTF-8,
invalid/valid O1 windows, all five inspector stages, aggregate cap and atomic
stage failure, same-byte internal digest/ordinal identity, no
source/path/external ID/digest return, stale lease before/after read and
Managed lock purge. Pure view tests use two valid windows differing only in
secret-like source IDs (including consistent references) and assert byte-identical
serialized operator views, with no secret or source-derived digest in either.
They also prove closed labels, ordinal references, bounded output, omission
of source fields outside the allowlist and atomic failure in the direct
bounded-serialization helper above the cap. Current valid-input maxima cannot
exercise the 4 MiB branch through the O1 pipeline; test the fixed native
failure envelope separately.
Legacy tests
cover manual Clear/teardown without claiming native lock support.
Frontend tests: unavailable desktop bridge, empty, valid, malformed replacement
retains prior view, conflicts/unknown, filtering/selection, no action controls,
keyboard/outline and stale-result purge. Run pinned portable and native Rust
checks, frontend build/test, actual responsive/keyboard review and fresh
independent source/UI review. A source test cannot establish all-three-OS GUI,
authenticated ingress, real LNSAT effect or OpenShell support.

No provider install, account, paid call, credential, engine activation,
database migration, network collection, Docker run, merge, deployment or
release is opened by this contract. Publication still needs its separate
README and final-head gates.
