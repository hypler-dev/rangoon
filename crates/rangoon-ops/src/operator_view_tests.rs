use super::*;
use serde_json::{Value, json};

fn oid(seed: u16) -> String {
    format!("ops:{seed:064x}")
}

fn event(id: u16, producer: &str, sequence: u64, previous: Option<String>, record: Value) -> Value {
    json!({
        "eventId": oid(id),
        "producerId": producer,
        "fenceId": oid(90),
        "sequence": sequence,
        "previousEventId": previous,
        "observedMs": null,
        "receivedMs": null,
        "correlation": null,
        "record": record,
    })
}

fn correlation(seed: u16) -> Value {
    json!({
        "workspaceId": oid(seed + 3), "systemId": oid(seed + 1),
        "environmentId": oid(seed + 40), "engineId": oid(seed + 41),
        "traceId": oid(seed + 42), "spanId": oid(seed + 43),
        "requestId": null, "evaluationId": null, "attemptId": null,
        "policyRevisionId": null,
    })
}

fn flow(seed: u16) -> Value {
    json!({
        "workspaceId": oid(seed + 1), "systemId": oid(seed + 2),
        "environmentId": oid(seed + 3), "engineId": oid(seed + 4),
        "requestId": oid(seed + 8), "traceId": oid(seed + 10),
        "spanId": oid(seed + 11), "policyRevisionId": oid(seed + 5),
    })
}

fn flow_correlation(seed: u16) -> Value {
    json!({
        "workspaceId": oid(seed + 1), "systemId": oid(seed + 2),
        "environmentId": oid(seed + 3), "engineId": oid(seed + 4),
        "requestId": oid(seed + 8), "evaluationId": oid(seed + 9),
        "attemptId": null, "traceId": oid(seed + 10), "spanId": oid(seed + 11),
        "policyRevisionId": oid(seed + 5),
    })
}

fn digest(seed: u16) -> String {
    format!("{seed:064x}")
}

fn request_evaluation_window(seed: u16) -> Value {
    let producer = oid(seed + 50);
    let records = vec![
        json!({
            "kind": "actor", "id": oid(seed + 7), "systemId": oid(seed + 2),
            "canonicalSubjectId": oid(seed + 12), "actorKind": "agent",
            "identitySourceRef": null, "identityEvidenceId": null,
            "reportedVerification": "reported_verified", "observedMs": null,
            "freshness": "reported_current",
        }),
        json!({
            "kind": "engine", "id": oid(seed + 4), "systemId": oid(seed + 2),
            "externalRef": {"systemId": oid(seed + 2), "contractVersion": "v1", "externalId": format!("engine-secret-{seed}")},
            "artifactDigest": digest(seed + 20),
        }),
        json!({
            "kind": "policy_revision", "id": oid(seed + 5), "gatewayId": oid(seed + 6),
            "previousRevisionId": null, "artifactDigest": digest(seed + 21), "externalRef": null,
        }),
        json!({
            "kind": "gateway_revision", "id": oid(seed + 6), "gatewayId": oid(seed + 6),
            "previousRevisionId": null, "policyRevisionId": oid(seed + 5),
            "scope": {"systemId": oid(seed + 2), "environmentIds": [oid(seed + 3)], "operationClasses": [], "resourceIds": [], "policyRevisionIds": [oid(seed + 5)]},
            "humanRequired": false, "changedByActorId": oid(seed + 7), "state": "reported_active", "evidenceIds": [],
        }),
        json!({
            "kind": "request", "id": oid(seed + 8), "context": flow(seed),
            "actorId": oid(seed + 7), "gatewayRevisionId": oid(seed + 6), "operationClass": "inspect",
            "actionDigest": digest(seed + 22), "contextDigest": digest(seed + 23), "resourceIds": [],
            "workflowRef": null, "expiresMs": null,
        }),
        json!({
            "kind": "evaluation", "id": oid(seed + 9), "context": flow(seed),
            "evaluationKind": "initial", "gatewayRevisionId": oid(seed + 6), "evaluatorId": oid(seed + 7),
            "evaluatorArtifactDigest": digest(seed + 24), "inputSnapshotId": oid(seed + 13),
            "inputCanonicalization": "json-v1", "inputDigest": digest(seed + 25),
            "contextSnapshotId": oid(seed + 14), "contextDigest": digest(seed + 26),
            "previousEvaluationId": null, "expectedEffectiveEvaluationId": null, "lineageFenceId": oid(90),
            "result": "allow", "expiresMs": null, "reason": "none",
            "externalRef": {"systemId": oid(seed + 2), "contractVersion": "v1", "externalId": format!("evaluation-secret-{seed}")},
            "evidenceIds": [],
        }),
    ];
    let tail = records
        .into_iter()
        .enumerate()
        .map(|(index, record)| {
            let mut row = event(
                seed + 100 + index as u16,
                &producer,
                (index + 1) as u64,
                (index > 0).then(|| oid(seed + 99 + index as u16)),
                record,
            );
            row["correlation"] = flow_correlation(seed);
            row
        })
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": "rangoon.ops.window.v1",
        "snapshot": {
            "snapshotId": oid(seed), "reducerVersion": "rangoon.ops.reducer.v1",
            "scope": {"workspaceId": oid(seed + 1), "systemIds": [oid(seed + 2)], "environmentIds": [oid(seed + 3)], "engineIds": [oid(seed + 4)]},
            "topology": {"revisionId": oid(seed + 15), "nodes": [], "edges": [], "policyRevisionIds": [oid(seed + 5)], "coverageRevisionIds": []},
            "prefixes": [], "asOfMs": 0,
        },
        "tail": tail,
    })
}

fn append_decision_event(window: &mut Value, seed: u16, record: Value) {
    let tail = window["tail"].as_array_mut().unwrap();
    let index = tail.len();
    let producer = tail[0]["producerId"].as_str().unwrap().to_owned();
    let previous = tail.last().unwrap()["eventId"].as_str().unwrap().to_owned();
    let mut row = event(
        seed + 100 + index as u16,
        &producer,
        (index + 1) as u64,
        Some(previous),
        record,
    );
    row["correlation"] = flow_correlation(seed);
    tail.push(row);
}

fn decision_counts_window(seed: u16) -> Value {
    let mut value = request_evaluation_window(seed);
    for (offset, result) in ["deny", "unknown"].into_iter().enumerate() {
        let mut evaluation = value["tail"][5]["record"].clone();
        evaluation["id"] = json!(oid(seed + 500 + offset as u16));
        evaluation["result"] = json!(result);
        append_decision_event(&mut value, seed, evaluation);
    }
    for (offset, decision) in ["approve", "deny", "abstain"].into_iter().enumerate() {
        append_decision_event(
            &mut value,
            seed,
            json!({
                "kind": "approval_decision", "id": oid(seed + 510 + offset as u16),
                "context": flow(seed), "evaluationId": oid(seed + 9), "planId": oid(seed + 60),
                "stageId": oid(seed + 61), "actorId": oid(seed + 7), "delegationId": null,
                "decision": decision, "supersedesDecisionId": null, "reason": "none", "evidenceIds": [],
            }),
        );
    }
    for (offset, state) in [
        "reported_issued",
        "reported_refused",
        "reported_expired",
        "reported_revoked",
        "unknown",
    ]
    .into_iter()
    .enumerate()
    {
        append_decision_event(
            &mut value,
            seed,
            json!({
                "kind": "authorization", "id": oid(seed + 520 + offset as u16),
                "context": flow(seed), "evaluationId": oid(seed + 9), "planId": null,
                "decisionIds": [], "previousObservationId": null, "state": state,
                "scope": {"systemId": oid(seed + 2), "environmentIds": [oid(seed + 3)], "operationClasses": [], "resourceIds": [], "policyRevisionIds": [oid(seed + 5)]},
                "expiresMs": null, "reason": "none", "externalRef": null, "evidenceIds": [],
            }),
        );
    }
    for (offset, state) in [
        "reported_accepted",
        "reported_active",
        "reported_completed",
        "reported_failed",
        "reported_cancelled",
        "outcome_unknown",
        "reported_reconciled_completed",
        "reported_reconciled_failed",
    ]
    .into_iter()
    .enumerate()
    {
        append_decision_event(
            &mut value,
            seed,
            json!({
                "kind": "attempt", "id": oid(seed + 530 + offset as u16),
                "context": flow(seed), "authorizationId": null, "executorId": oid(seed + 7),
                "previousObservationId": null, "attemptId": oid(seed + 540 + offset as u16),
                "state": state, "reason": "none", "evidenceIds": [],
            }),
        );
    }
    value
}

fn window(seed: u16, with_references: bool) -> Value {
    let producer = oid(seed + 10);
    let actor = oid(seed + 20);
    let evidence = oid(seed + 21);
    let tail = if with_references {
        let mut actor_event = event(
            seed + 30,
            &producer,
            1,
            None,
            json!({
                "kind": "actor", "id": actor, "systemId": oid(seed + 1),
                "canonicalSubjectId": oid(seed + 2), "actorKind": "agent",
                "identitySourceRef": null, "identityEvidenceId": evidence,
                "reportedVerification": "reported_verified", "observedMs": null,
                "freshness": "reported_current",
            }),
        );
        actor_event["correlation"] = correlation(seed);
        let mut evidence_event = event(
            seed + 31,
            &producer,
            2,
            Some(oid(seed + 30)),
            json!({
                "kind": "evidence", "id": evidence, "producerId": producer,
                "externalRef": {"systemId": oid(seed + 1), "contractVersion": "v1", "externalId": format!("secret-{seed}")},
                "artifactDigest": null,
                "state": "unverified", "reason": "unknown_outcome",
            }),
        );
        evidence_event["correlation"] = correlation(seed);
        vec![actor_event, evidence_event]
    } else {
        Vec::new()
    };
    json!({
        "schemaVersion": "rangoon.ops.window.v1",
        "snapshot": {
            "snapshotId": oid(seed), "reducerVersion": "rangoon.ops.reducer.v1",
            "scope": {
                "workspaceId": oid(seed + 3), "systemIds": [oid(seed + 1)],
                "environmentIds": [], "engineIds": []
            },
            "topology": {
                "revisionId": oid(seed + 4), "nodes": [], "edges": [],
                "policyRevisionIds": [], "coverageRevisionIds": []
            },
            "prefixes": [], "asOfMs": 0
        },
        "tail": tail,
    })
}

fn inspection_parts(
    value: &Value,
) -> (
    DecodedWindow,
    Projection,
    Relationships,
    DelegationInspection,
    ApprovalInspection,
    LifecycleInspection,
) {
    let bytes = serde_json::to_vec(value).expect("fixture serializes");
    let decoded = decode_window(&bytes).expect("fixture decodes");
    let projection = project_window(&decoded).expect("projection succeeds");
    let relationships = relate_window(&decoded).expect("relationships succeed");
    let delegations = inspect_delegations(&decoded).expect("delegations succeed");
    let approvals = inspect_approvals(&decoded).expect("approvals succeed");
    let lifecycle = inspect_lifecycle(&decoded).expect("lifecycle succeeds");
    (
        decoded,
        projection,
        relationships,
        delegations,
        approvals,
        lifecycle,
    )
}

fn inspect(value: &Value) -> OperatorView {
    let (decoded, projection, relationships, delegations, approvals, lifecycle) =
        inspection_parts(value);
    build_operator_view(
        &decoded,
        &projection,
        &relationships,
        &delegations,
        &approvals,
        &lifecycle,
    )
    .expect("operator view succeeds")
}

#[test]
fn empty_window_emits_only_the_closed_view_contract() {
    let view = inspect(&window(1, false));
    let value: Value = serde_json::from_slice(view.canonical_bytes()).expect("view is JSON");
    let object = value.as_object().expect("view is an object");
    assert_eq!(object.len(), 13);
    assert_eq!(value["schemaVersion"], OPERATOR_VIEW_SCHEMA);
    assert_eq!(value["authority"], "none");
    assert_eq!(value["authenticity"], "unverified");
    assert_eq!(value["eligibility"], "unknown");
    assert_eq!(value["execution"], "unavailable");
    assert_eq!(value["outcome"], "unknown");
    assert_eq!(value["coverage"], "unknown");
    assert_eq!(value["events"], json!([]));
    assert_eq!(
        value["decisionCounts"],
        json!({
            "evaluation": {"allow": 0, "deny": 0, "unknown": 0},
            "approvalDecision": {"approve": 0, "deny": 0, "abstain": 0},
            "authorization": {"reportedIssued": 0, "reportedRefused": 0, "reportedExpired": 0, "reportedRevoked": 0, "unknown": 0},
            "attempt": {"reportedAccepted": 0, "reportedActive": 0, "reportedCompleted": 0, "reportedFailed": 0, "reportedCancelled": 0, "outcomeUnknown": 0, "reportedReconciledCompleted": 0, "reportedReconciledFailed": 0},
        })
    );
    assert_eq!(
        value["sourceCounts"]["recordCounts"]
            .as_array()
            .unwrap()
            .len(),
        19
    );
    assert!(view.canonical_bytes().len() <= MAX_OPERATOR_VIEW_BYTES);
}

#[test]
fn source_identifier_variants_have_byte_identical_safe_views() {
    let first = inspect(&window(100, true));
    let second = inspect(&window(500, true));
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let output = std::str::from_utf8(first.canonical_bytes()).expect("view is UTF-8");
    for secret in [oid(100), oid(110), oid(120), oid(121), oid(130), oid(500)] {
        assert!(
            !output.contains(&secret),
            "source identifier leaked: {secret}"
        );
    }
    assert!(!output.contains("secret-100"));
    assert!(!output.contains("secret-500"));
    assert!(!output.contains("canonicalDigest"));
    assert!(!output.contains("externalRef"));
    assert_eq!(
        serde_json::from_slice::<Value>(first.canonical_bytes()).unwrap()["events"][0]
            ["references"]
            .as_array()
            .unwrap()
            .len(),
        0,
    );
}

#[test]
fn view_uses_only_closed_labels_and_ordinals() {
    let view = inspect(&window(100, true));
    let value: Value = serde_json::from_slice(view.canonical_bytes()).unwrap();
    let events = value["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    for (ordinal, event) in events.iter().enumerate() {
        assert_eq!(event["inputOrdinal"], ordinal);
        assert!(RECORD_KINDS.contains(&event["recordKind"].as_str().unwrap()));
        assert!(event["reportedValue"].is_null());
        assert!(
            event["issueStages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|stage| {
                    [
                        "structural",
                        "relationships",
                        "delegations",
                        "approvals",
                        "lifecycle",
                    ]
                    .contains(&stage.as_str().unwrap())
                })
        );
    }
}

#[test]
fn mixed_stage_windows_fail_atomically() {
    let first = decode_window(&serde_json::to_vec(&window(1, false)).unwrap()).unwrap();
    let second = decode_window(&serde_json::to_vec(&window(2, false)).unwrap()).unwrap();
    let projection = project_window(&first).unwrap();
    let relationships = relate_window(&second).unwrap();
    let delegations = inspect_delegations(&second).unwrap();
    let approvals = inspect_approvals(&second).unwrap();
    let lifecycle = inspect_lifecycle(&second).unwrap();
    assert_eq!(
        build_operator_view(
            &second,
            &projection,
            &relationships,
            &delegations,
            &approvals,
            &lifecycle,
        )
        .unwrap_err()
        .code(),
        "operator_view_stage_mismatch"
    );
}

#[test]
fn relationship_references_keep_only_ordinal_statuses_in_stable_source_order() {
    let view = inspect(&request_evaluation_window(1_000));
    let value: Value = serde_json::from_slice(view.canonical_bytes()).unwrap();
    let request_references = value["events"][4]["references"].as_array().unwrap();
    let evaluation_references = value["events"][5]["references"].as_array().unwrap();
    assert_eq!(
        request_references,
        &vec![
            json!({"referenceIndex": 0, "status": "unique_unverified", "matchingEventOrdinals": [1]}),
            json!({"referenceIndex": 1, "status": "unique_unverified", "matchingEventOrdinals": [2]}),
            json!({"referenceIndex": 2, "status": "unique_unverified", "matchingEventOrdinals": [0]}),
            json!({"referenceIndex": 3, "status": "unique_unverified", "matchingEventOrdinals": [3]}),
        ]
    );
    assert_eq!(
        evaluation_references,
        &vec![
            json!({"referenceIndex": 0, "status": "unique_unverified", "matchingEventOrdinals": [4]}),
            json!({"referenceIndex": 1, "status": "unique_unverified", "matchingEventOrdinals": [1]}),
            json!({"referenceIndex": 2, "status": "unique_unverified", "matchingEventOrdinals": [2]}),
            json!({"referenceIndex": 3, "status": "unique_unverified", "matchingEventOrdinals": [3]}),
            json!({"referenceIndex": 4, "status": "unique_unverified", "matchingEventOrdinals": [0]}),
        ]
    );
}

#[test]
fn referenced_source_identifier_variants_are_byte_identical_and_field_free() {
    let first = inspect(&request_evaluation_window(1_000));
    let second = inspect(&request_evaluation_window(2_000));
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let output = std::str::from_utf8(first.canonical_bytes()).unwrap();
    for source_only in [
        oid(1_004),
        oid(1_005),
        oid(1_006),
        oid(1_007),
        oid(1_008),
        oid(1_009),
        "engine-secret-1000".to_owned(),
        "evaluation-secret-1000".to_owned(),
        "context.engineId".to_owned(),
        "expectedKind".to_owned(),
        "recordId".to_owned(),
    ] {
        assert!(
            !output.contains(&source_only),
            "source field leaked: {source_only}"
        );
    }
}

#[test]
fn decision_counts_cover_each_closed_bucket_with_its_own_observed_denominator() {
    let view = inspect(&decision_counts_window(3_000));
    let value: Value = serde_json::from_slice(view.canonical_bytes()).unwrap();
    assert_eq!(
        value["decisionCounts"],
        json!({
            "evaluation": {"allow": 1, "deny": 1, "unknown": 1},
            "approvalDecision": {"approve": 1, "deny": 1, "abstain": 1},
            "authorization": {"reportedIssued": 1, "reportedRefused": 1, "reportedExpired": 1, "reportedRevoked": 1, "unknown": 1},
            "attempt": {"reportedAccepted": 1, "reportedActive": 1, "reportedCompleted": 1, "reportedFailed": 1, "reportedCancelled": 1, "outcomeUnknown": 1, "reportedReconciledCompleted": 1, "reportedReconciledFailed": 1},
        })
    );
    let source_counts = value["sourceCounts"]["recordCounts"].as_array().unwrap();
    for (kind, total) in [
        ("evaluation", 3),
        ("approval_decision", 3),
        ("authorization", 5),
        ("attempt", 8),
    ] {
        assert_eq!(
            source_counts
                .iter()
                .find(|count| count["kind"] == kind)
                .unwrap()["count"],
            total
        );
    }
}

#[test]
fn decision_counts_remain_byte_identical_when_all_source_ids_change() {
    let first = inspect(&decision_counts_window(3_000));
    let second = inspect(&decision_counts_window(4_000));
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let output = std::str::from_utf8(first.canonical_bytes()).unwrap();
    for source_only in [oid(3_007), oid(3_008), oid(3_009), oid(3_510), oid(3_530)] {
        assert!(
            !output.contains(&source_only),
            "source identifier leaked: {source_only}"
        );
    }
}

#[test]
fn reference_aggregate_ceiling_is_atomic_for_internal_malformed_stage_data() {
    let value = request_evaluation_window(1_000);
    let (decoded, projection, mut relationships, delegations, mut approvals, mut lifecycle) =
        inspection_parts(&value);
    let reference = relationships.document.event_links[4].references[0].clone();
    let other_reference_count = relationships
        .document
        .event_links
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 4)
        .map(|(_, event)| event.references.len())
        .sum::<usize>();
    relationships.document.event_links[4].references =
        vec![reference; 8_192 - other_reference_count];
    relationships.canonical = serde_json::to_vec(&relationships.document).unwrap();
    let relationship_digest = relationships.canonical_digest();
    approvals.document.relationship_digest = relationship_digest.clone();
    approvals.canonical = serde_json::to_vec(&approvals.document).unwrap();
    let approval_digest = approvals.canonical_digest();
    lifecycle.document.relationship_digest = relationship_digest;
    lifecycle.document.approval_digest = approval_digest;
    lifecycle.canonical = serde_json::to_vec(&lifecycle.document).unwrap();
    let accepted = build_operator_view(
        &decoded,
        &projection,
        &relationships,
        &delegations,
        &approvals,
        &lifecycle,
    )
    .expect("exact 8,192 references remain within the view cap");
    assert!(accepted.canonical_bytes().len() <= MAX_OPERATOR_VIEW_BYTES);

    let overflow_reference = relationships.document.event_links[4].references[0].clone();
    relationships.document.event_links[4]
        .references
        .push(overflow_reference);
    relationships.canonical = serde_json::to_vec(&relationships.document).unwrap();
    let relationship_digest = relationships.canonical_digest();
    approvals.document.relationship_digest = relationship_digest.clone();
    approvals.canonical = serde_json::to_vec(&approvals.document).unwrap();
    lifecycle.document.relationship_digest = relationship_digest;
    lifecycle.document.approval_digest = approvals.canonical_digest();
    lifecycle.canonical = serde_json::to_vec(&lifecycle.document).unwrap();
    assert_eq!(
        build_operator_view(
            &decoded,
            &projection,
            &relationships,
            &delegations,
            &approvals,
            &lifecycle,
        )
        .unwrap_err()
        .code(),
        "operator_view_invalid"
    );
}
