# O1b-2b3 reported lifecycle, outcomes and coverage

Authority: the accepted operations scope in [intent.md](intent.md), the shared
[operations control plane](operations-control-plane.md), and the closed
[nineteen-record vocabulary](operations-records.md). Owner: Jeff. This is the
source contract for the last inert O1b inspector, not a live operations or
authorization contract. The canonical implementation and validation state is
recorded only in [development.md](development.md).

## Outcome and trust boundary

Expose a deterministic, read-only inspection of reported authorization,
attempt, suspension, metric, evidence and coverage observations in one decoded
window. Preserve every event and its input ordinal, including duplicates and
conflicts. Show correlation, predecessor links, gaps, contradictory states,
unknown effects, resource coverage and evidence references without treating
source assertions as facts. The other thirteen record kinds remain visible as
`not_assessed` rows and through earlier inspectors. Reuse the existing
structural projection, relationship, delegation and approval results; do not
modify their v1 schemas or reimplement policy, approval or execution.

`inspect_lifecycle(&DecodedWindow) -> Result<LifecycleInspection,
LifecycleError>` has no I/O, clock, randomness, credentials, database,
provider, network, process or host path access. The returned document has
fixed `authority: "none"`, `authenticity: "unverified"`,
`execution: "unavailable"`, `authoritativeEligibility: "unknown"`,
`authoritativeOutcome: "unknown"` and `qualifiedCoverage: "unknown"`.
`reported_issued`, `reported_completed`, `reported_acknowledged` and
`fully_mediated` are source labels only. No boolean or color in this output
grants permission to run, certifies a result or proves complete mediation.
Without authenticated ingress and execution-time consumption, there is no
trusted active state. Renderer code must display these provenance limits.

## Exact inspection model

Use a separate schema `rangoon.ops.lifecycle.v1`. Keep existing decoded,
structural, relationship, delegation and approval canonical bytes unchanged.
Expose canonical output bytes and SHA-256 digest, with a payload-free Debug
implementation. Ordered top-level fields: `schemaVersion`,
`inputCanonicalDigest`, `structuralProjectionDigest`, `relationshipDigest`,
`delegationDigest`, `approvalDigest`, `authority`, `authenticity`, `execution`,
`semanticAssessment`, `authoritativeEligibility`, `authoritativeOutcome`,
`qualifiedCoverage`, `diagnosticCounts`, `eventAssessments`,
`reportedChains`, `coverageAssessments`. `semanticAssessment` is
`linked_unverified`, `incomplete` or `conflict`; it summarizes inspection
coherence, not operational safety. An empty window is `linked_unverified`,
not evidence of healthy activity.

`eventAssessments` retains exactly one row per input event, ordered by input
ordinal. Fields: `inputOrdinal`, `eventId`, `recordId`, `recordKind`,
`assessment`, `reportedState`, `reportedAttemptId`, `diagnostics`,
`references`. The six selected kinds above are assessed; all others are
`not_assessed` with null reported fields and empty diagnostics/references.
An assessed row's `assessment` is exactly `linked_unverified`, `incomplete`
or `conflict` under the diagnostic precedence below. No row has a
`success`, `allowed` or `verified` assessment.
Map selected-kind fields exactly:

| Kind | `reportedState` | `reportedAttemptId` | Outgoing references beyond `evidenceIds` |
| --- | --- | --- | --- |
| `authorization` | `state` | null | `evaluationId`, optional `planId`, each `decisionIds` entry, optional `previousObservationId` |
| `attempt` | `state` | `attemptId` | optional `authorizationId`, optional `previousObservationId` |
| `suspension` | `state` | null | `gatewayRevisionId`, optional `previousObservationId` |
| `metric` | null | nullable `attemptId` | optional `attemptId` correlation and, when cost exists, optional `cost.sourceEvidenceId` |
| `evidence` | `state` (always `unverified`) | null | none |
| `coverage` | null | null | `gatewayRevisionId` |

Only authorization, attempt, suspension and coverage have `evidenceIds`;
resolve each entry only for those kinds. A metric's null latency/cost remains
null in the original window; no zero or billing total is derived. A null
attempt authorization is unknown, not proof of unauthorized execution.
Evidence `state: unverified` remains unverified even when a digest is present.

`references` use the existing `ReferenceLink` shape and statuses:
`unique_unverified`, `missing`, `ambiguous`, `wrong_kind`, with all matching
event ordinals in ascending order. Resolve exactly the fields in the table
and the kinds' `evidenceIds` where present. Metric `attemptId` is a
correlation key, not a record ID: `ReferenceLink.recordId` carries that
selector, `expectedKind` is `attempt`, and resolution matches every
`attempt.attemptId`, not `attempt.id`. Multiple observations in one coherent
attempt chain remain one `unique_unverified` target with all ordinals.
No matching group is `missing`; independent roots, conflicting bindings or
an otherwise unresolved attempt group are `ambiguous`, never first-match.
Money `sourceEvidenceId` is an evidence reference when cost is present.
Emit every declared reference, including broken ones; never choose first of
multiple matches. A unique target's bytes and context must still be checked.

`reportedChains` groups authorization by exact stable binding, attempt by
`attemptId`, and suspension by gateway revision plus exact scope. Fields:
`kind`, `groupId`, `matchingEventOrdinals`, `recordIds`,
`reportedChainStatus`, `reportedTipRecordId`, `reportedTipState`,
`diagnostics`. `recordIds` are root-to-tip only for a coherent single chain;
otherwise distinct IDs sort lexicographically. `matchingEventOrdinals`
always retain repeats. `reportedChainStatus` is `single_chain_unverified` or
`unknown_or_conflict`; the latter has null tip and state. A reported single
chain still has no authoritative eligibility or outcome. No arrival time,
array position, matching text or implied sequence repairs a broken link.

`groupId` is always an `ops:` ID. For attempt it is the supplied `attemptId`.
For authorization and suspension it is `ops:` followed by lowercase SHA-256
of one compact UTF-8 JSON array. Authorization array elements, in order, are
`"authorization"`, the seven continuity-key values in the order named below,
`evaluationId`, nullable `planId`, and `scope`. Suspension elements are
`"suspension"`, `gatewayRevisionId`, and `scope`. `scope` is a JSON object
with ordered fields `systemId`, `environmentIds`, `operationClasses`,
`resourceIds`, `policyRevisionIds`; arrays retain their input order. All IDs
and tokens retain exact bytes, and null is literal JSON `null`. This hash is
only a grouping label, not an authenticated identity or grant. Each event
belongs to the group computed from its own record, even if its predecessor
is missing, ambiguous, cyclic or in another group; such links do not merge
groups. A conflicting same-ID record in another group taints every occurrence
of that ID. Rows sort by kind order authorization, attempt, suspension, then
`groupId` bytewise. Each group's matching event ordinals sort ascending.

For each kind, a null predecessor declares an initial observation. A nonnull
predecessor must uniquely resolve to the same record kind and group, have a
lower producer sequence in the same producer/fence, and preserve stable
binding. Authorization stable binding uses the seven continuity keys
`workspaceId`, `systemId`, `environmentId`, `engineId`, `requestId`,
`traceId`, `policyRevisionId` from `FlowContext`, plus `evaluationId`,
`planId` and exact `scope`. `spanId` may differ by observation and is not a
continuity key. Attempt uses those same seven context keys plus `attemptId`,
`authorizationId` and `executorId`.
Suspension stable binding is `gatewayRevisionId` and exact `ActionScope`;
it has no FlowContext, so do not fabricate a request/trace. Distinct roots,
forks, cycles, missing or ambiguous predecessors, cross-group binding,
different stream/fence, reverse sequence, contradictory same-ID records or
structurally tainted events yield `unknown_or_conflict`. Byte-identical
repeated IDs remain one node with every ordinal visible, not an extra effect.
For byte-identical repeats in one producer/fence, compare each node's
earliest sequence: successor earliest sequence must exceed predecessor
earliest sequence. A later repeated observation of the same ID does not
change this comparison. Repeats across different producer/fence pairs are
`stream_mismatch` and cannot supply a chosen sequence.
The existing structural stream/scope/correlation diagnostics taint affected
selected observations; a graph-only layout diagnostic does not.

Interpret succession conservatively. Authorization roots may report
`reported_issued`, `reported_refused` or `unknown`. `reported_issued` may be
followed by `reported_expired`, `reported_revoked` or `unknown`; refusal,
expiry and revocation are terminal. `unknown` never becomes issuance proof
and has no permitted successor in this source inspector. Attempt roots may
report `reported_accepted` or `outcome_unknown`; a terminal-only root is
incomplete, not an inferred prior acceptance. `reported_accepted` may progress
to `reported_active` or a reported completed/failed/cancelled/unknown outcome;
`reported_active` may progress to a reported completed/failed/cancelled/unknown
outcome. Completed, failed and cancelled are terminal source labels. Only
`outcome_unknown` may be followed by `reported_reconciled_completed` or
`reported_reconciled_failed`, with at least one uniquely resolved evidence
reference; reconciliation remains source-reported. Cancellation is not proof
that no effect happened. Suspension `requested` may progress to
`reported_acknowledged`, `reported_partial` or `unknown`; a root other than
`requested` is incomplete. `reported_acknowledged` or `reported_partial` may
be followed by `reported_resumed`; `unknown` has no trusted successor. A
`reported_resumed` label cannot imply uncovered
executors are safe. Invalid or skipped transitions are explicit diagnostics,
not silently normalized. A repeated `attemptId` with independent roots is
ambiguous; it does not prove replay defense, duplicate side effects or safety
to retry. Unknown effects never trigger an automatic retry.

`coverageAssessments` has one row per coverage observation, in input order.
Fields: `inputOrdinal`, `recordId`, `reportedLevel`, `reportedFreshness`,
`observedMs`, `expiresMs`, `targetOs`, `checks`, `assessment`,
`diagnostics`, `references`, `qualifiedLevel`. Copy all eight checks as
reported, preserving `unknown`; `qualifiedLevel` is always `unknown`.
Missing observation time or expiry, unknown/stale freshness, expired report
as of the caller-supplied snapshot `asOfMs`, absent evidence IDs, broken gateway/evidence
references, and contradictory rows for the same exact
system/engine/gateway/operation/environment/OS/resource selector produce
explicit incomplete/conflict diagnostics. For this comparison, ignore only
record `id` and `observedMs`; compare exact `reportedLevel`, `freshness`,
`expiresMs`, all eight checks, both artifact digests and `evidenceIds` in
input order. Any difference conflicts. Identical payloads with different IDs
or observation times remain separate unverified rows without choosing a
latest winner. Byte-identical repeated IDs retain all ordinals.
`reported_current` and a future
expiry do not authenticate coverage. Never average a coverage score or infer
that a system is fully mediated from this window. Differing resources are
separate scopes; absence of a row is unavailable/unknown, not compliant.

`diagnosticCounts` is an ordered array of `{code,count}` rows in the fixed
code order below, including zero counts. Each row's `diagnostics` is a
deduplicated array in that same order. Count a code once per affected
event, chain or coverage-assessment row; a coverage event and its separate
coverage-assessment row are distinct counted rows.

Diagnostic codes are fixed and payload-free: `structural_incomplete`, `reference_missing`,
`reference_ambiguous`, `reference_kind_mismatch`, `context_mismatch`,
`binding_mismatch`, `predecessor_missing`, `predecessor_ambiguous`,
`predecessor_wrong_kind`, `sequence_mismatch`, `stream_mismatch`,
`multiple_roots`, `branch`, `cycle`, `state_transition_invalid`,
`effect_unknown`, `reconciliation_evidence_missing`,
`coverage_freshness_unknown`, `coverage_stale`, `coverage_expired`,
`coverage_conflict`, `coverage_evidence_missing`. Count order is this order,
including zero entries. Precedence is conflict over incomplete over
linked_unverified. Missing/unknown effect, missing evidence and freshness
gaps are incomplete; contradictory binding, branch/cycle/transition and
coverage conflicts are conflict. `effect_unknown` is never green. Do not
include raw reason, payload, path, URL, token or provider prose in
diagnostics. Exact incomplete codes are `structural_incomplete`,
`reference_missing`, `reference_kind_mismatch`, `predecessor_missing`,
`predecessor_wrong_kind`, `effect_unknown`,
`reconciliation_evidence_missing`, `coverage_freshness_unknown`,
`coverage_stale`, `coverage_expired`, `coverage_evidence_missing`.
All other codes in the closed list are conflicts. A uniquely resolved
evaluation/authorization/attempt target with differing seven-key flow context
gets `context_mismatch`; an ambiguous target does not become a first-match
context comparison. Coverage evidence absence or unresolved evidence adds
`coverage_evidence_missing` alongside the reference diagnostic. An
`outcome_unknown` or `reported_cancelled` attempt adds `effect_unknown`;
no terminal report changes fixed authoritative outcome `unknown`.

## Bounds, tests and next gate

Input inherits the decoder's 2 MiB, 1,024-event and 32-item list ceilings.
The inspector caps output at 1 MiB, 8,192 reference rows, 8,192 matching
ordinals and 8,192 predecessor walk steps. Check counters before growth;
failure is atomic with fixed error codes and unchanged input. Use one
invocation-local immutable identity index and iterative walks. No persistent
cache. For each group in the specified row order, validate each distinct
record ID in bytewise order by walking from that ID through declared
predecessors. Charge one step for the starting ID and one for each followed
predecessor ID, including a missing, wrong-kind, cross-group, or repeated
cycle-closing ID. Null predecessor charges nothing. Byte-identical repeated
observations of one ID do not create another starting walk; separate groups
charge independently. No cross-walk cache suppresses a step. If a coherent
root-to-tip path is emitted, charge each emitted record ID once more. The
counter permits exactly 8,192 charged steps and fails before step 8,193;
iterative loop bookkeeping itself is free. `LifecycleError` variants/codes are `SerializationFailed` /
`lifecycle_serialization_failed`, `StructuralLimit` /
`lifecycle_structural_limit`, `RelationshipLimit` /
`lifecycle_relationship_limit`, `DelegationLimit` /
`lifecycle_delegation_limit`, `ApprovalLimit` /
`lifecycle_approval_limit`, and `Limit` / `lifecycle_limit`. Run upstream
projection, relationship, delegation and approval inspections in that order.
Map each upstream limit to its matching class; approval's delegated
`StructuralLimit`, `RelationshipLimit` and `DelegationLimit` map to those
classes, its own `Limit` maps to `ApprovalLimit`. Map any upstream
serialization failure to `SerializationFailed`. The inspector's own output,
reference, matching and walk ceilings map to `Limit`; its own serialization
failure maps to `SerializationFailed`. Do not collapse a cap into a successful
partial document.

Independent literal fixtures must cover empty/one-per-kind retention and
canonical order, valid reported chains, repeated identical IDs, changed
same-ID records, missing/wrong/ambiguous references, stable-binding and
sequence/fence breaks, branch/cycle/root collisions, all state transitions,
cancel/unknown/reconciliation with and without evidence, nullable metrics,
independent attempt roots, suspension partial/resume, coverage missing/stale/
expired/conflicting scopes/checks, stream taint, exact cap errors and Debug
privacy. Named Rust format/test/warnings-denied Clippy, artifact/reference
and whitespace checks plus fresh independent contract/source/fixture review
are required. Implementation stays within `rangoon-ops` and its tests;
README and canonical ledger record actual source results before publication.

O1b-2b3 does not create an event collector, authority call, approval router,
live graph, audit exporter or enforcement path. O2 builds read-only views;
O3 qualifies authenticated minimized ingress, provenance and retention;
O4 qualifies one complete execution-time enforcement and evidence path.
OpenShell/LNSAT runtime integration remains separately pinned and tested.
