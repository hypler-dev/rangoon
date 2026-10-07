use super::*;
use serde_json::{Value, json};
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut seed = [0; 32];
        getrandom::fill(&mut seed).unwrap();
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-native-workflow-{}",
            rangoon_domain::byte_digest(&seed)
        )))
    }
    fn store(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.0.join("workspace.sqlite3")).unwrap()
    }
    fn skill(&self) -> (String, String) {
        let report =
            rangoon_import::analyze("AGENTS.md", b"# Rules\nKeep exact inputs.\n").unwrap();
        let store = self.store();
        store.save_v1(&report).unwrap();
        let skill = store
            .create_capability_v1(&report.source.id, &report.fragments[0].id, "Rules")
            .unwrap()
            .capability;
        (skill.id, skill.latest_revision_id)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn body(value: Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(&value).unwrap())
}
fn id(prefix: &str, digit: char) -> String {
    format!("{prefix}{}", digit.to_string().repeat(64))
}
fn definition(reference: Option<(&str, &str)>) -> Value {
    let mut value = json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Local workflow 🦀",
        "nodes":[
            {"id":"start","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"finish","toPort":"text"}]});
    if let Some((cap, rev)) = reference {
        value["nodes"].as_array_mut().unwrap().insert(1, json!({"id":"work","title":"Skill","operation":{"kind":"capability","capabilityId":cap,"revisionId":rev},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]}));
        value["controlEdges"] = json!([{"fromNode":"start","outlet":"next","toNode":"work"},{"fromNode":"work","outlet":"next","toNode":"finish"}]);
        value["dataEdges"] = json!([{"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},{"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}]);
    }
    value
}
fn request(draft: &str, definition: Value, intent: &str) -> InspectRequest {
    decode_inspect(&body(json!({"schemaVersion":"rangoon.workflow-inspect.v1","draftId":draft,"intent":intent,"definition":definition,"layout":{"positions":[{"nodeIndex":0,"x":-17,"y":29}]}}))).unwrap()
}
fn begin(session: &WorkflowSession, store: &Workspace, target: Target) -> (String, String) {
    let result = session.begin(store, target).unwrap();
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(
        encoded["schemaVersion"],
        "rangoon.workflow-draft-session.v1"
    );
    assert_eq!(encoded["outcome"], "draft_ready");
    (
        encoded["draftId"].as_str().unwrap().into(),
        encoded["workflowId"].as_str().unwrap().into(),
    )
}
fn inspected(
    session: &WorkflowSession,
    store: &Workspace,
    request: InspectRequest,
) -> (Confirmation, WorkflowRevision, WorkflowSavePlan) {
    let result = session.inspect(store, request).unwrap();
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(encoded["schemaVersion"], "rangoon.workflow-save-session.v1");
    assert_eq!(encoded["report"]["authority"], "none");
    assert_eq!(encoded["report"]["executionStatus"], "unavailable");
    assert_eq!(encoded["report"]["referenceStatus"], "unverified");
    match result {
        WorkflowResult::SaveReady {
            preview_id,
            revision,
            plan,
            ..
        } => {
            let confirmation = Confirmation {
                schema_version: "rangoon.workflow-commit.v1".into(),
                preview_id,
                expected_state_id: plan.expected_state_id.clone(),
                acknowledged: true,
            };
            (confirmation, *revision, plan)
        }
        _ => panic!("expected retained candidate"),
    }
}

#[test]
fn raw_requests_are_closed_bounded_required_and_duplicate_safe() {
    type Decoder = fn(&InvokeBody) -> bool;
    let cases: Vec<(Value, usize, Decoder)> = vec![
        (
            json!({"schemaVersion":"rangoon.workflow-open.v1","workflowId":id("workflow:",'a'),"selection":{"kind":"head"}}),
            SMALL_LIMIT,
            |b| decode_open(b).is_ok(),
        ),
        (
            json!({"schemaVersion":"rangoon.workflow-draft.v1","target":{"kind":"new"}}),
            SMALL_LIMIT,
            |b| decode_begin(b).is_ok(),
        ),
        (
            json!({"schemaVersion":"rangoon.workflow-inspect.v1","draftId":id("workflow-draft:",'b'),"intent":"draft","definition":definition(None),"layout":{"positions":[]}}),
            INSPECT_LIMIT,
            |b| decode_inspect(b).is_ok(),
        ),
        (
            json!({"schemaVersion":"rangoon.workflow-commit.v1","previewId":id("workflow-preview:",'c'),"expectedStateId":id("workspace:",'d'),"acknowledged":true}),
            SMALL_LIMIT,
            |b| decode_commit(b).is_ok(),
        ),
        (
            json!({"schemaVersion":"rangoon.workflow-clear.v1","draftId":id("workflow-draft:",'b')}),
            SMALL_LIMIT,
            |b| decode_clear(b).is_ok(),
        ),
    ];
    for (value, limit, decode) in cases {
        assert!(decode(&body(value.clone())));
        assert!(!decode(&InvokeBody::Json(value.clone())));
        assert!(!decode(&InvokeBody::Raw(vec![255])));
        assert!(!decode(&InvokeBody::Raw(vec![b' '; limit + 1])));
        let mut padded = serde_json::to_vec(&value).unwrap();
        padded.resize(limit, b' ');
        assert!(decode(&InvokeBody::Raw(padded)));
        for field in value.as_object().unwrap().keys() {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(!decode(&body(missing)), "missing {field}");
        }
        for (field, same_value) in value.as_object().unwrap() {
            let mut duplicate = serde_json::to_string(&value).unwrap();
            duplicate.pop();
            duplicate.push_str(&format!(
                ",{}:{} }}",
                serde_json::to_string(field).unwrap(),
                same_value
            ));
            assert!(
                !decode(&InvokeBody::Raw(duplicate.into_bytes())),
                "same-value duplicate {field}"
            );
        }
        let mut unknown = value.clone();
        unknown["rendererPlan"] = json!({});
        assert!(!decode(&body(unknown)));
        let mut wrong = value.clone();
        wrong["schemaVersion"] = "unknown".into();
        assert!(!decode(&body(wrong)));
        let mut duplicate = serde_json::to_string(&value).unwrap();
        duplicate.pop();
        duplicate.push_str(",\"schemaVersion\":\"unknown\"}");
        assert!(!decode(&InvokeBody::Raw(duplicate.into_bytes())));
        let mut trailing = serde_json::to_vec(&value).unwrap();
        trailing.extend_from_slice(b"{}");
        assert!(!decode(&InvokeBody::Raw(trailing)));
    }
    assert!(decode_begin(&body(json!({"schemaVersion":"rangoon.workflow-draft.v1","target":{"kind":"append","workflowId":id("workflow:",'a'),"expectedHeadId":id("workflow-revision:",'b')}}))).is_ok());
    assert!(decode_open(&body(json!({"schemaVersion":"rangoon.workflow-open.v1","workflowId":id("workflow:",'a'),"selection":{"kind":"historical","revisionId":id("workflow-revision:",'b')}}))).is_ok());
    for target in [
        json!({"kind":"new","workflowId":id("workflow:",'a')}),
        json!({"kind":"append","workflowId":id("workflow:",'a'),"expectedHeadId":"bad"}),
        json!({"kind":"other"}),
    ] {
        assert!(
            decode_begin(&body(
                json!({"schemaVersion":"rangoon.workflow-draft.v1","target":target})
            ))
            .is_err()
        );
    }
    for selection in [
        json!({"kind":"head","revisionId":id("workflow-revision:",'b')}),
        json!({"kind":"historical","revisionId":"bad"}),
        json!({"kind":"other"}),
    ] {
        assert!(decode_open(&body(json!({"schemaVersion":"rangoon.workflow-open.v1","workflowId":id("workflow:",'a'),"selection":selection}))).is_err());
    }
    assert!(decode_commit(&body(json!({"schemaVersion":"rangoon.workflow-commit.v1","previewId":id("workflow-preview:",'c'),"expectedStateId":"bad","acknowledged":true}))).is_err());
    assert!(decode_commit(&body(json!({"schemaVersion":"rangoon.workflow-commit.v1","previewId":id("workflow-preview:",'c'),"expectedStateId":id("workspace:",'d'),"acknowledged":false}))).is_err());
}

#[test]
fn embedded_raw_values_preserve_duplicate_checks_depth_and_byte_limits() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    for (definition_json, layout_json) in [
        ("{\"schemaVersion\":\"rangoon.workflow-definition.v1\",\"schemaVersion\":\"rangoon.workflow-definition.v1\",\"title\":\"Draft\",\"nodes\":[],\"controlEdges\":[],\"dataEdges\":[]}".into(), "{\"positions\":[]}".into()),
        (definition(None).to_string(), "{\"positions\":[],\"positions\":[]}".into()),
        (format!("{{{}\"schemaVersion\":\"rangoon.workflow-definition.v1\",\"title\":\"Draft\",\"nodes\":[],\"controlEdges\":[],\"dataEdges\":[]}}", " ".repeat(128 * 1024)), "{\"positions\":[]}".into()),
        (definition(None).to_string(), format!("{{{}\"positions\":[]}}", " ".repeat(16 * 1024))),
        (format!("{{\"a\":{}0{}}}", "[".repeat(33), "]".repeat(33)), "{\"positions\":[]}".into()),
        ("null".into(), "{\"positions\":[]}".into()),
        (definition(None).to_string(), "null".into()),
    ] {
        let wire = format!("{{\"schemaVersion\":\"rangoon.workflow-inspect.v1\",\"draftId\":\"{draft}\",\"intent\":\"draft\",\"definition\":{definition_json},\"layout\":{layout_json}}}");
        let request = decode_inspect(&InvokeBody::Raw(wire.into_bytes())).unwrap();
        assert!(matches!(session.inspect(&store, request), Err(StoreError::WorkflowInvalid)));
    }
    assert!(!fixture.0.exists());
}

#[test]
fn new_identity_replacement_clear_and_entropy_failure_preserve_custody() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (first, one) = begin(&session, &store, Target::New {});
    let (confirmation, _, _) =
        inspected(&session, &store, request(&first, definition(None), "draft"));
    assert!(matches!(
        session.begin_with(&store, Target::New {}, |_| Err(StoreError::Unavailable)),
        Err(StoreError::Unavailable)
    ));
    assert!(session.context(&first).is_ok());
    assert!(session.0.lock().unwrap().prepared.is_some());
    let (second, two) = begin(&session, &store, Target::New {});
    assert_ne!(one, two);
    assert_ne!(first, second);
    assert!(matches!(
        session.claim(&confirmation),
        Err(StoreError::WorkspaceChanged)
    ));
    assert!(matches!(
        session.clear(&first),
        Err(StoreError::WorkspaceChanged)
    ));
    assert!(session.context(&second).is_ok());
    assert!(matches!(
        session.clear(&second),
        Ok(WorkflowResult::Cleared)
    ));
    assert!(session.context(&second).is_err());
    assert!(store.list_workflows().unwrap().is_empty());
    assert!(!fixture.0.exists());
}

#[test]
fn malformed_or_failed_inspection_keeps_prior_exact_candidate() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    let (confirmation, candidate, plan) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    assert!(matches!(
        session.inspect(&store, request(&draft, json!({"bad":true}), "draft")),
        Err(StoreError::WorkflowInvalid)
    ));
    assert!(matches!(
        session.stage(&session.context(&draft).unwrap(), candidate, plan, |_| Err(
            StoreError::Unavailable
        )),
        Err(StoreError::Unavailable)
    ));
    let saved = session.commit(&store, &confirmation).unwrap();
    assert_eq!(saved.workflow.history.len(), 1);
}

#[test]
fn matching_confirmation_is_one_attempt_on_success_refusal_and_uncertainty() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    let (mut confirmation, _, _) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    let original = confirmation.expected_state_id.clone();
    confirmation.expected_state_id = id("workspace:", '0');
    assert!(matches!(
        session.claim(&confirmation),
        Err(StoreError::WorkspaceChanged)
    ));
    confirmation.expected_state_id = original;
    confirmation.acknowledged = false;
    assert!(matches!(
        session.claim(&confirmation),
        Err(StoreError::WorkflowInvalid)
    ));
    confirmation.acknowledged = true;
    let mut actions = 0;
    assert_eq!(
        session
            .commit_with(&confirmation, |_| {
                actions += 1;
                Err(StoreError::Unavailable)
            })
            .unwrap_err(),
        StoreError::Unavailable
    );
    assert_eq!(
        session
            .commit_with(&confirmation, |_| {
                actions += 1;
                Err(StoreError::Unavailable)
            })
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(actions, 1);
    assert!(session.context(&draft).is_ok());
    assert!(!fixture.0.exists());
    let (confirmation, _, _) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    session.commit(&store, &confirmation).unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn draft_save_restart_append_history_and_read_only_open_preserve_bytes() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, owner) = begin(&session, &store, Target::New {});
    let incomplete = json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Incomplete","nodes":[],"controlEdges":[],"dataEdges":[]});
    let mut invalid = request(&draft, incomplete.clone(), "draft");
    invalid.layout = RawValue::from_string("{\"positions\":[]}".into()).unwrap();
    let (confirmation, root, _) = inspected(&session, &store, invalid);
    assert!(!root.inspection().report().structurally_valid);
    let first = session.commit(&store, &confirmation).unwrap();
    assert_eq!(
        first.workflow.revision.serialized_bytes().unwrap(),
        root.serialized_bytes().unwrap()
    );
    assert_eq!(
        fixture.store().open_workflow(&owner, None).unwrap(),
        first.workflow
    );
    let before = fixture.bytes();
    assert!(matches!(
        session.begin(
            &store,
            Target::Append {
                workflow_id: owner.clone(),
                expected_head_id: id("workflow-revision:", '0')
            }
        ),
        Err(StoreError::WorkflowConflict)
    ));
    assert_eq!(fixture.bytes(), before);
    let (append, same) = begin(
        &session,
        &store,
        Target::Append {
            workflow_id: owner.clone(),
            expected_head_id: root.id().into(),
        },
    );
    assert_eq!(same, owner);
    let (confirmation, child, _) = inspected(
        &session,
        &store,
        request(&append, definition(None), "validated"),
    );
    assert_eq!(child.parent_revision_id(), Some(root.id()));
    let receipt = session.commit(&store, &confirmation).unwrap();
    assert_eq!(receipt.workflow.history.len(), 2);
    assert_eq!(receipt.workflow.revision.layout().positions()[0].x, -17);
    let before = fixture.bytes();
    assert_eq!(
        store
            .open_workflow(&owner, Some(root.id()))
            .unwrap()
            .revision,
        root
    );
    assert_eq!(
        store.list_workflows().unwrap()[0].latest_revision_id,
        child.id()
    );
    assert_eq!(fixture.bytes(), before);
    assert!(session.context(&append).is_ok());
}

#[test]
fn stale_state_and_moved_head_refuse_without_touching_current_records() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, owner) = begin(&session, &store, Target::New {});
    let (confirmation, root, _) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    let source = rangoon_import::analyze("CLAUDE.md", b"# Changed\nA new source.\n").unwrap();
    store.save_v1(&source).unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    let (confirmation, _, _) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    session.commit(&store, &confirmation).unwrap();
    let (append, _) = begin(
        &session,
        &store,
        Target::Append {
            workflow_id: owner.clone(),
            expected_head_id: root.id().into(),
        },
    );
    let (confirmation, _, _) = inspected(
        &session,
        &store,
        request(&append, definition(None), "validated"),
    );
    let mut other = definition(None);
    other["title"] = "Another writer".into();
    let candidate = prepare_revision(
        &owner,
        Some(root.id()),
        SaveIntent::Draft,
        &serde_json::to_vec(&other).unwrap(),
        b"{\"positions\":[]}",
    )
    .unwrap();
    let plan = store.inspect_workflow_save(&candidate).unwrap();
    store
        .save_workflow(&candidate, &plan.expected_state_id)
        .unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkflowConflict
    );
    assert_eq!(fixture.bytes(), before);
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn concurrent_exact_candidate_allows_only_unchanged_noop_then_moved_head_refuses() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, owner) = begin(&session, &store, Target::New {});
    let (confirmation, candidate, original) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    let concurrent = store
        .save_workflow(&candidate, &original.expected_state_id)
        .unwrap();
    let before = fixture.bytes();
    let receipt = session.commit(&store, &confirmation).unwrap();
    assert!(receipt.already_saved);
    assert_eq!(receipt.workflow, concurrent.workflow);
    assert_eq!(fixture.bytes(), before);
    let (confirmation, _, plan) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    assert!(plan.already_saved);
    let child = prepare_revision(
        &owner,
        Some(candidate.id()),
        SaveIntent::Draft,
        &serde_json::to_vec(&definition(None)).unwrap(),
        b"{\"positions\":[]}",
    )
    .unwrap();
    let next = store.inspect_workflow_save(&child).unwrap();
    store
        .save_workflow(&child, &next.expected_state_id)
        .unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkflowConflict
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn validated_pins_missing_dependency_refusal_and_input_preservation() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let (cap, revision) = fixture.skill();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    assert!(matches!(
        session.inspect(
            &store,
            request(
                &draft,
                definition(Some((&cap, &id("revision:", 'f')))),
                "validated"
            )
        ),
        Err(StoreError::WorkflowDependencyMissing)
    ));
    let before = fixture.bytes();
    let (confirmation, candidate, _) = inspected(
        &session,
        &store,
        request(&draft, definition(Some((&cap, &revision))), "validated"),
    );
    assert_eq!(fixture.bytes(), before);
    let saved = session.commit(&store, &confirmation).unwrap();
    assert!(saved.workflow.head.structurally_valid);
    assert_eq!(saved.workflow.head.unresolved_references, 0);
    assert_eq!(saved.workflow.revision, candidate);
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.list_capabilities_v1().unwrap().len(), 1);
    assert_eq!(
        serde_json::to_value(&saved.workflow.references).unwrap()[0]["status"],
        "resolved"
    );
}

#[test]
fn deleted_owner_does_not_gain_retry_permission() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, owner) = begin(&session, &store, Target::New {});
    let (confirmation, candidate, original) =
        inspected(&session, &store, request(&draft, definition(None), "draft"));
    store
        .save_workflow(&candidate, &original.expected_state_id)
        .unwrap();
    let deletion = store
        .inspect_workflow_deletion(rangoon_store::WorkflowRecordKind::Workflow, &owner)
        .unwrap();
    store.delete_workflow_record(&deletion).unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fixture.bytes(), before);
    assert!(store.list_workflows().unwrap().is_empty());
}

#[test]
fn exact_local_permissions_and_command_registration_cover_only_main_workbench() {
    let permissions: Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    assert_eq!(permissions["windows"], json!(["main"]));
    assert_eq!(permissions["local"], true);
    assert!(permissions.get("remote").is_none());
    let manifest = include_str!("../build.rs");
    let main = include_str!("main.rs");
    for command in [
        "list_workflows",
        "open_workflow",
        "begin_workflow_draft",
        "inspect_workflow_save",
        "commit_workflow_save",
        "clear_workflow_draft",
    ] {
        assert_eq!(manifest.matches(&format!("\"{command}\"")).count(), 1);
        assert!(main.contains(&format!("            {command},")));
        let permission = format!("allow-{}", command.replace('_', "-"));
        assert_eq!(
            permissions["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| **p == permission)
                .count(),
            1
        );
        for restricted in [
            include_str!("../capabilities/model-confirmation.json"),
            include_str!("../capabilities/cloud-model-confirmation.json"),
            include_str!("../capabilities/cloud-credential-entry.json"),
        ] {
            assert!(!restricted.contains(&permission));
        }
    }
    assert!(main.contains(".manage(WorkflowSession::default())"));
}

#[test]
fn exact_noop_refuses_changed_unresolved_count_but_reports_fresh_equal_count_status() {
    let seed = Fixture::new();
    let (cap, revision) = seed.skill();
    for (pin, expected_status, should_succeed) in [
        (revision.as_str(), "resolved", false),
        (id("revision:", 'f').as_str(), "missing_revision", true),
    ] {
        let fixture = Fixture::new();
        let store = fixture.store();
        let session = WorkflowSession::default();
        let (draft, _) = begin(&session, &store, Target::New {});
        let (confirmation, candidate, plan) = inspected(
            &session,
            &store,
            request(&draft, definition(Some((&cap, pin))), "draft"),
        );
        assert_eq!(plan.unresolved_references, 1);
        let concurrent = store
            .save_workflow(&candidate, &plan.expected_state_id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&concurrent.workflow.references).unwrap()[0]["status"],
            "missing_capability"
        );
        let actual = fixture.skill();
        assert_eq!(actual, (cap.clone(), revision.clone()));
        let before = fixture.bytes();
        if should_succeed {
            let receipt = session.commit(&store, &confirmation).unwrap();
            assert!(receipt.already_saved);
            assert_eq!(receipt.workflow.head.unresolved_references, 1);
            assert_eq!(
                serde_json::to_value(&receipt.workflow.references).unwrap()[0]["status"],
                expected_status
            );
        } else {
            assert_eq!(
                session.commit(&store, &confirmation).unwrap_err(),
                StoreError::WorkspaceChanged
            );
        }
        assert_eq!(fixture.bytes(), before);
        assert_eq!(
            session.commit(&store, &confirmation).unwrap_err(),
            StoreError::WorkspaceChanged
        );
    }
}

#[test]
fn replacement_preview_and_failed_append_begin_preserve_only_current_handles() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    let (old, _, _) = inspected(&session, &store, request(&draft, definition(None), "draft"));
    let mut next = definition(None);
    next["title"] = "Revised local draft".into();
    let (current, candidate, _) = inspected(&session, &store, request(&draft, next, "draft"));
    assert_eq!(
        session.claim(&old).err(),
        Some(StoreError::WorkspaceChanged)
    );
    assert!(matches!(
        session.begin(
            &store,
            Target::Append {
                workflow_id: id("workflow:", 'f'),
                expected_head_id: id("workflow-revision:", 'f')
            }
        ),
        Err(StoreError::WorkflowNotFound)
    ));
    assert!(session.context(&draft).is_ok());
    let receipt = session.commit(&store, &current).unwrap();
    assert_eq!(receipt.workflow.revision, candidate);
    assert_eq!(receipt.workflow.history.len(), 1);
}

#[test]
fn dependency_deleted_after_validated_preview_refuses_and_consumes_attempt() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let (cap, revision) = fixture.skill();
    let session = WorkflowSession::default();
    let (draft, _) = begin(&session, &store, Target::New {});
    let (confirmation, _, _) = inspected(
        &session,
        &store,
        request(&draft, definition(Some((&cap, &revision))), "validated"),
    );
    let deletion = store
        .inspect_workflow_deletion(rangoon_store::WorkflowRecordKind::Capability, &cap)
        .unwrap();
    store.delete_workflow_record(&deletion).unwrap();
    let before = fixture.bytes();
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkflowDependencyMissing
    );
    assert_eq!(
        session.commit(&store, &confirmation).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fixture.bytes(), before);
    assert!(session.context(&draft).is_ok());
    assert_eq!(store.list().unwrap().len(), 1);
}
