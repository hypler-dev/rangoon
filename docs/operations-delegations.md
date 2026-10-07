# O1b-2b1 reported identity and delegation inspection

Authority: accepted operations continuation in [intent.md](intent.md); canonical evidence [development.md](development.md). Owner Jeff; controller owns semantics/privacy/integration/publication. This packet supplies alias-aware identity and finite delegation inspection needed before agent approval duties. O1b-2b2 human duties/stage DAG/quorum/decision succession and O1b-2b3 lifecycle/outcome/coverage remain required; then O2 actual operations views, O3 qualified ingress and O4/O5 enforcement/expansion. Staging does not remove any accepted V1 requirement.

## Boundary and immutable API

`inspect_delegations(&DecodedWindow) -> Result<DelegationInspection, DelegationError>` is the only constructor. It calls project_window on that same immutable validated window; no caller projection/JSON trust flag, I/O, clock, network, authentication, engine, policy admission, authority or execution. Existing window/projection/relationships schemas and exact fixtures stay unchanged. Private-field serializable DTOs expose no public construction, deserialization or mutation. Wrapper accessors canonical_bytes(), canonical_digest(), document(); Debug reveals only digest/byte count.

Fixed output: authority none, authenticity unverified, execution unavailable, semanticAssessment reported_delegation_links, authoritativeEligibility unknown, authoritativeEffectiveEvaluationId null, subdelegationAdmission unavailable. Reported verified/current/active never alter these. Internally agreeing aliases/scopes are observations, not authenticated subjects or admitted grants. Default subdelegation remains unavailable even when an observed parent chain narrows. This stage does not count human approvals or certify an approver, current expiry, revocation, assignment/gateway admission, engine binding, workspace membership or transitive self-expansion through independently admitted configurations. Those require subsequent source and qualified authority contracts.

Errors are fixed and payload-free: delegation_serialization_failed, delegation_structural_limit (upstream projection Limit), delegation_limit. Input remains unchanged; no partial output. Output caps at 1,048,576 bytes using the identical existing bounded writer. Other upstream errors map to serialization_failed. Resource ceilings: 8,192 emitted reference rows; 8,192 combined matching ordinals across references and subject rows; 8,192 combined emitted path IDs/traversal steps, charged once per appended ID (the corresponding traversal step), including own and closing/dangling IDs. Check before cloning/pushing/incrementing past each ceiling. Maximum events/subjects 1,024, plus existing O1a limits; iterative walks, no recursion, clock or cross-call/persistent caches. Build an invocation-local immutable record-resolution index (first ordinal, kind, ambiguity) once, comparing each observation to its ID’s first record bytes once; reference/ancestor lookups read that metadata and the original ordinal vectors without rescanning record bytes. Discard the index before return. Exact defensive counter boundaries may be private tests; end-to-end fixtures must separately exercise real reference/match/path/output/upstream limits where reachable under O1a. A work ceiling dominated by the wire cap is not falsely claimed as full-output success.

## Exact output and order

Schema rangoon.ops.delegations.v1. Ordered top-level fields: schemaVersion, inputCanonicalDigest, structuralProjectionDigest, authority, authenticity, execution, semanticAssessment, authoritativeEligibility, authoritativeEffectiveEvaluationId, subdelegationAdmission, diagnosticCounts, subjects, eventAssessments. Digests bind exact bytes for internal consistency only.

subjects: one row for each reported (systemId,canonicalSubjectId) tuple in actor observations, sorted lexicographically by that pair. Ordered fields: systemId, canonicalSubjectId, actorRecordIds (distinct sorted IDs), reportedActorKinds (distinct sorted kind strings), matchingEventOrdinals (every actor occurrence for that tuple, ascending), assessment, diagnostics. An ambiguous actor ID that reports different tuples appears in both groups with conflict diagnostics; do not pick one tuple or drop variants. Equal canonicalSubjectId across different systems remains separate. Names, external IDs and identity-source references never create inferred cross-system aliases.

eventAssessments: exactly one row per event in inputOrdinal order, preserving repeats/conflicts and all unassessed kinds. Ordered fields: inputOrdinal, eventId, recordId, recordKind, assessment, reportedState, reportedVerification, reportedFreshness (each nullable), subject (nullable tuple object systemId,canonicalSubjectId; actor only), delegatorSubject, delegateSubject, changedBySubject (each nullable tuple; delegation only), ancestorRecordIds, diagnostics, references. Actor reportedState is null, and its reportedVerification/reportedFreshness copy reportedVerification/freshness. Delegation copies state/reportedVerification/freshness into those three reported fields. Every unassessed kind has all three null. Copied report labels never create eligibility. Only actor/delegation are assessed; other seventeen kinds have not_assessed, null report/subject fields and empty ancestors/diagnostics/references. Original observations remain in O1a/O1b-1; normalized rows do not erase unknown data.

Reference rows use the existing private read-only reference DTO: ordered field,index nullable,recordId,expectedKind,status,matchingEventOrdinals. Actor emits identityEvidenceId → evidence only when nonnull. Delegation emits delegatorId,delegateId,changedByActorId → actor, parentDelegationId → delegation when nonnull, then evidenceIds[] → evidence in list order. Status unique_unverified iff all same-ID typed record bytes match and expected kind agrees; missing if absent, ambiguous if differing bytes, wrong_kind otherwise. Ambiguity precedes wrong kind. Preserve every ordinal ascending, repeated list reference separately indexed. Unique actor tuple can be displayed even when tainted or role-conflicted; diagnostics remain attached and its display is not eligibility. Absent/ambiguous/wrong-kind actor gives null tuple.

## Structural and subject checks

Use the same exact stream-taint set as relationships: event_identity_conflict, sequence_conflict, record_identity_conflict, sequence_gap, predecessor_missing, predecessor_conflict, prefix_fence_mismatch, prefix_watermark_mismatch, prefix_identity_conflict, scope_mismatch, correlation_mismatch. Any such timeline flag taints its entire producer/fence stream. Snapshot prefix_watermark_mismatch/prefix_identity_conflict taint every assessed row. Duplicate_observation and graph-only diagnostics do not taint this stage. Own record identity disagreement flags reference_ambiguous; direct references inherit any matching observation stream taint.

Each subject aggregates all actor-member diagnostics, including reference diagnostics and stream taint. More than one reported actorKind in that tuple flags subject_kind_conflict. All its actor event rows inherit the aggregate; delegation actor dependencies inherit that aggregate too. Claimed reported_verified or human labels cannot turn an unverified subject into authenticated human review. Unique delegation record diagnostics aggregate all its byte-identical observation occurrences for ancestor inspection; a clean duplicate cannot hide a tainted occurrence.

## Finite scope, alias and parent checks

Every uniquely referenced delegation actor must have systemId equal delegation.scope.systemId; otherwise system_mismatch. Compare exact reported tuple (systemId,canonicalSubjectId), not only actor record ID. Delegator == delegate flags self_delegation. ChangedBy == delegate flags self_expansion even when different actor IDs claim the same tuple. Every tuple comparison requires all compared actor references to resolve uniquely. Missing/ambiguous/wrong-kind references retain their reference diagnostics and null tuples; null/null or null/tuple never creates self_delegation, self_expansion or parent_actor_mismatch. System comparison also requires a uniquely resolved actor. Ambiguous own delegation still displays its occurrence-level reported tuples, references and local diagnostics, but stops ancestry after its own ID. This is a reported alias check, not a complete admission or independent-root self-escalation proof.

Empty environmentIds/operationClasses/resourceIds/policyRevisionIds in any delegation scope flags scope_incomplete; empty never means wildcard or universal authority. Lists compare as finite sets for attenuation; originals/repeated refs remain unchanged. Root expiresMs null flags expiry_incomplete; finite root expiry remains a reported value, without comparing to time.

For each uniquely resolved child/parent pair: scope.systemId must match; when both actor references resolve uniquely, child's delegator tuple must equal parent's delegate tuple or parent_actor_mismatch. All four child scope sets must be subsets of parent sets; any added member flags scope_expansion. Missing dimensions remain incomplete, not implicit infinite scope. Both expiries must be finite to compare; null on either side flags expiry_incomplete. A finite child later than finite parent flags expiry_expansion. Strict attenuation needs at least one proper finite-set subset or strictly earlier finite child expiry. If all dimensions and expiries are present, no expansion and none strictly smaller, flag attenuation_missing. No expiry clock, renewal or current-validity inference occurs. Parent identity/record/reference conflicts remain visible and cannot be replaced by another compatible-looking parent.

## Bounded ancestry

Each delegation row starts ancestorRecordIds with its own ID, then follows exact parent IDs leaf-to-root. Include a missing/ambiguous/wrong-kind parent ID before stopping so the dangling link stays inspectable. Ambiguous own identity stops after own ID; never choose a variant. A repeated ID is appended once as the closing step, flags delegation_cycle and stops. Record/path work uses the explicit total budget above; maximum one walk is bounded by event count + one closing/dangling step. An over-budget traversal returns atomic delegation_limit, not a shorter path.

Union every uniquely reached ancestor's aggregate diagnostics into the leaf row. Parent-pair checks are done on every delegation before propagation. For subject_cycle, begin with leaf's unique delegate tuple, then walk the reported delegator tuples leaf-to-root; any repeated tuple flags subject_cycle. Missing/conflicting tuples retain unknown/incomplete references rather than proving acyclicity. Cycles across independent roots/assignments without declared parent links are not assessed here. Parent links and tuple checks never admit subdelegation.

## Closed diagnostics and assessment

Diagnostics and count fields use this exact order, zero keys retained; count affected subject rows plus eventAssessments once per code per entry:

1. structural_incomplete
2. reference_missing
3. reference_ambiguous
4. reference_kind_mismatch
5. subject_kind_conflict
6. system_mismatch
7. self_delegation
8. self_expansion
9. parent_actor_mismatch
10. scope_incomplete
11. scope_expansion
12. expiry_incomplete
13. expiry_expansion
14. attenuation_missing
15. delegation_cycle
16. subject_cycle

Assessment: not_assessed for other kinds; conflict if any of reference_ambiguous,subject_kind_conflict,system_mismatch,self_delegation,self_expansion,parent_actor_mismatch,scope_expansion,expiry_expansion,attenuation_missing,delegation_cycle,subject_cycle; incomplete if structural_incomplete,reference_missing,reference_kind_mismatch,scope_incomplete,expiry_incomplete; otherwise linked_unverified. There is no eligible/allowed/approved/verified assessment. Actor subject rows use the same priority.

## Ownership and acceptance

Controller owns new delegation_types.rs, delegations.rs, lib.rs exports/module integration, this contract, README, operations-control-plane status, continuation and ledger. Independent native fixture worker owns only delegation_tests.rs after fresh contract review; minimal secret-free contract/vocabulary packets, independent literal windows and manually authored output bytes/SHA. Not alone; preserve other edits. No manifests/dependencies/lockfiles, projection/relationships implementation or existing fixtures, native/UI/assets/store/model/engine/CI/parent-marketing changes. Two carry-forward PR42 receipt/continuation docs join this README-bearing packet.

Required independent fixtures: exact empty OUTPUT bytes/SHA/order/trust golden; all nineteen kinds retained and selected reporting fields; same-system aliases vs same-subject cross-system separation; role/record identity contradictions and exact duplicate ordinal retention; missing/ambiguous/wrong-kind evidence/actors/parents; actor subject taint propagation vs unrelated graph noise; alias self-delegation/changedBy self-expansion and system mismatch; proper subset in each scope dimension and empty/no wildcard/expansion; strict finite expiry shortening, equal/no attenuation, larger/nullable expiry; matching and mismatched parent subjects; direct/transitive ID and subject cycles, missing ancestry and ancestor diagnostics; repeated observation aggregate taint; full event retention and separately accounted row/match/path/wire/upstream limits with fixed error/Debug and unchanged decoded input. Source checks do not establish authenticated delegation, human-duty satisfaction, admitted controls, live ingestion or OS/runtime/release qualification. Run pinned source Rust, artifact/reference/intent/whitespace checks and fresh independent contract/source/fixture/claim review, then exact final-head hosted/publication/fetched-main gates. README update required every main publication. Full O1b/V1 remains open.
