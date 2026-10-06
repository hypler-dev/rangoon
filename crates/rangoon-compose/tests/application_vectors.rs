//! Independent destination vectors and public-API regression cases.
use rangoon_compose::application::{self, Request, Target, TargetHead};
use rangoon_compose::{InputReference, Operation, Piece, ResolvedInput};
use rangoon_domain::{Authority, capability::revision_id};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/composition/application-v0.json"
    ))
    .unwrap()
}

fn inputs(fixture: &Value) -> Vec<ResolvedInput> {
    fixture["resolvedInputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| ResolvedInput::Source {
            source_id: input["reference"]["sourceId"].as_str().unwrap().into(),
            sha256: input["reference"]["sha256"].as_str().unwrap().into(),
            content: input["content"].as_str().unwrap().into(),
        })
        .collect()
}

fn request(fixture: &Value, case: &Value) -> Request {
    application::decode_request_json(
        &serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.composition-application-request.v0",
            "draft": fixture["draft"],
            "targets": case["targets"],
        }))
        .unwrap(),
    )
    .unwrap()
}

fn heads(case: &Value) -> Vec<TargetHead> {
    case["targetHeads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|head| TargetHead {
            capability_id: head["capabilityId"].as_str().unwrap().into(),
            revision_id: head["revisionId"].as_str().unwrap().into(),
        })
        .collect()
}

#[test]
fn independent_vectors_pin_new_append_and_mixed_destinations() {
    let fixture = fixture();
    let inputs = inputs(&fixture);
    for case in fixture["cases"].as_array().unwrap() {
        let request = request(&fixture, case);
        let result = application::preview(&request, &inputs, &heads(case)).unwrap();
        let bytes =
            application::application_identity_envelope_bytes(&request.draft, &request.targets)
                .unwrap();
        assert_eq!(bytes, case["expectedEnvelope"].as_str().unwrap().as_bytes());
        assert_eq!(
            bytes.len() as u64,
            case["expectedEnvelopeByteLength"].as_u64().unwrap()
        );
        let result_json = serde_json::to_value(&result).unwrap();
        assert_eq!(result_json["applicationId"], case["expectedApplicationId"]);
        assert_eq!(
            application::application_id(&request.draft, &request.targets).unwrap(),
            case["expectedApplicationId"].as_str().unwrap()
        );
        assert_eq!(
            result_json["appliedOutputs"],
            case["expectedAppliedOutputs"]
        );
        assert_eq!(
            result_json["schemaVersion"],
            "rangoon.composition-application-preview.v0"
        );
        assert_eq!(result_json["authority"], "none");
        assert_eq!(result_json["saveable"], true);
        assert_eq!(
            result.core,
            rangoon_compose::preview(&request.draft, &inputs).unwrap()
        );
        assert_eq!(result.core.authority, Authority::None);
        let pretty = serde_json::to_string_pretty(&request).unwrap();
        assert_eq!(
            application::decode_request_json(pretty.as_bytes()).unwrap(),
            request
        );
        let whitespace = pretty.replace(": [", ": \r\n\t[");
        assert_eq!(
            application::decode_request_json(whitespace.as_bytes()).unwrap(),
            request
        );
    }
}

#[test]
fn destinations_and_expected_parent_bind_identity_without_changing_recipe() {
    let fixture = fixture();
    let case = &fixture["cases"][1];
    let mut request = request(&fixture, case);
    let inputs = inputs(&fixture);
    let mut heads = heads(case);
    let original = application::preview(&request, &inputs, &heads).unwrap();
    let Target::Append {
        expected_revision_id,
        ..
    } = &mut request.targets[0]
    else {
        panic!("fixture target must append");
    };
    *expected_revision_id = format!("revision:{}", "e".repeat(64));
    heads[0].revision_id = expected_revision_id.clone();
    let changed_parent = application::preview(&request, &inputs, &heads).unwrap();
    assert_eq!(original.core, changed_parent.core);
    assert_ne!(original.application_id, changed_parent.application_id);
    assert_ne!(
        original.applied_outputs[0].revision_id,
        changed_parent.applied_outputs[0].revision_id
    );
    let Target::Append { capability_id, .. } = &mut request.targets[0] else {
        unreachable!();
    };
    *capability_id = format!("capability:{}", "f".repeat(64));
    heads[0].capability_id = capability_id.clone();
    let changed_destination = application::preview(&request, &inputs, &heads).unwrap();
    assert_eq!(original.core, changed_destination.core);
    assert_ne!(
        changed_parent.application_id,
        changed_destination.application_id
    );
    assert_ne!(
        changed_parent.applied_outputs[0].revision_id,
        changed_destination.applied_outputs[0].revision_id
    );
}

#[test]
fn same_text_with_different_recipe_keeps_distinct_derivation() {
    let fixture = fixture();
    let case = &fixture["cases"][1];
    let mut request = request(&fixture, case);
    let inputs = inputs(&fixture);
    let heads = heads(case);
    let original = application::preview(&request, &inputs, &heads).unwrap();
    request.draft.outputs[0].pieces.push(Piece::Authored {
        content: String::new(),
        reason: "Explicit empty separator".into(),
    });
    let changed = application::preview(&request, &inputs, &heads).unwrap();
    assert!(changed.saveable);
    assert_eq!(
        original.core.outputs[0].content,
        changed.core.outputs[0].content
    );
    assert_ne!(original.application_id, changed.application_id);
    assert_ne!(
        original.applied_outputs[0].revision_id,
        changed.applied_outputs[0].revision_id
    );
    let output = &changed.applied_outputs[0];
    assert_ne!(
        output.revision_id,
        revision_id(
            &output.capability_id,
            output.parent_revision_id.as_deref(),
            &changed.core.outputs[0].title,
            &changed.core.outputs[0].content
        )
    );
}

#[test]
fn rejects_cross_kind_destination_alias_and_stale_or_extra_heads() {
    let fixture = fixture();
    let case = &fixture["cases"][2];
    let inputs = inputs(&fixture);
    let mut request = request(&fixture, case);
    let mut heads = heads(case);
    let original = application::preview(&request, &inputs, &heads).unwrap();
    let Target::Append { capability_id, .. } = &mut request.targets[1] else {
        panic!("fixture target must append");
    };
    *capability_id = original.applied_outputs[0].capability_id.clone();
    heads[0].capability_id = capability_id.clone();
    assert!(application::preview(&request, &inputs, &heads).is_err());

    let request = self::request(&fixture, case);
    let mut heads = self::heads(case);
    heads[0].revision_id = format!("revision:{}", "0".repeat(64));
    assert!(application::preview(&request, &inputs, &heads).is_err());
    assert!(application::preview(&request, &inputs, &[]).is_err());
    let mut heads = self::heads(case);
    heads.push(heads[0].clone());
    assert!(application::preview(&request, &inputs, &heads).is_err());
}

#[test]
fn direct_values_reject_forged_reordered_and_duplicate_destinations() {
    let fixture = fixture();
    let case = &fixture["cases"][1];
    let inputs = inputs(&fixture);
    let request = request(&fixture, case);
    let mut heads = heads(case);
    heads.swap(0, 1);
    assert!(application::preview(&request, &inputs, &heads).is_err());

    let mut heads = self::heads(case);
    heads[0].capability_id = "private path and source marker".into();
    let error = application::preview(&request, &inputs, &heads).unwrap_err();
    assert!(!error.to_string().contains("private path and source marker"));

    let heads = self::heads(case);
    let mut duplicate = request.clone();
    duplicate.targets[1] = duplicate.targets[0].clone();
    assert!(matches!(
        application::preview(&duplicate, &inputs, &heads),
        Err(application::ApplicationError::DuplicateDestination { output_index: 1 })
    ));
    for invalid in [String::new(), format!("capability:{}", "A".repeat(64))] {
        let mut malformed = request.clone();
        let Target::Append { capability_id, .. } = &mut malformed.targets[0] else {
            unreachable!();
        };
        *capability_id = invalid;
        assert!(application::preview(&malformed, &inputs, &heads).is_err());
    }
    let mut missing = request.clone();
    missing.targets.clear();
    assert!(application::preview(&missing, &inputs, &[]).is_err());
    let mut future = request;
    future.schema_version = "rangoon.composition-application-request.future".into();
    assert!(application::preview(&future, &inputs, &heads).is_err());
}

#[test]
fn public_identity_helpers_require_exact_distinct_recipe_destinations() {
    let fixture = fixture();
    let case = &fixture["cases"][2];
    let request = request(&fixture, case);
    for invalid in [
        vec![],
        vec![Target::New {}],
        vec![Target::New {}; 3],
        vec![Target::New {}; 17],
    ] {
        assert!(
            application::application_identity_envelope_bytes(&request.draft, &invalid).is_err()
        );
        assert!(application::application_id(&request.draft, &invalid).is_err());
    }
    let result = application::preview(&request, &inputs(&fixture), &heads(case)).unwrap();
    let alias = vec![
        Target::New {},
        Target::Append {
            capability_id: result.applied_outputs[0].capability_id.clone(),
            expected_revision_id: format!("revision:{}", "b".repeat(64)),
        },
    ];
    assert!(matches!(
        application::application_identity_envelope_bytes(&request.draft, &alias),
        Err(application::ApplicationError::DuplicateDestination { output_index: 1 })
    ));
    assert!(application::application_id(&request.draft, &alias).is_err());
    let mut invalid_draft = request.draft;
    invalid_draft.schema_version = "unsupported".into();
    assert!(application::application_id(&invalid_draft, &request.targets).is_err());
}

#[test]
fn historical_input_can_append_to_current_head_of_same_capability() {
    let fixture = fixture();
    let case = &fixture["cases"][2];
    let mut request = request(&fixture, case);
    let heads = heads(case);
    let historical_id = format!("revision:{}", "e".repeat(64));
    let source = &fixture["resolvedInputs"][0];
    let content = source["content"].as_str().unwrap().to_string();
    let sha256 = source["reference"]["sha256"].as_str().unwrap().to_string();
    request.draft.operation = Operation::Split;
    request.draft.inputs = vec![InputReference::Revision {
        capability_id: heads[0].capability_id.clone(),
        revision_id: historical_id.clone(),
        sha256: sha256.clone(),
    }];
    let input = ResolvedInput::Revision {
        capability_id: heads[0].capability_id.clone(),
        revision_id: historical_id.clone(),
        sha256,
        content,
    };
    let result = application::preview(&request, std::slice::from_ref(&input), &heads).unwrap();
    assert!(result.saveable);
    assert_eq!(
        result.applied_outputs[1].parent_revision_id.as_deref(),
        Some(heads[0].revision_id.as_str())
    );
    let wrong_head = TargetHead {
        capability_id: heads[0].capability_id.clone(),
        revision_id: historical_id,
    };
    assert!(application::preview(&request, &[input], &[wrong_head]).is_err());
}

#[test]
fn incomplete_append_outputs_remain_editable_blocked_previews() {
    let fixture = fixture();
    let case = &fixture["cases"][1];
    let mut request = request(&fixture, case);
    request.draft.outputs[0].title.clear();
    request.draft.outputs[1].pieces.clear();
    let result = application::preview(&request, &inputs(&fixture), &heads(case)).unwrap();
    assert!(!result.saveable);
    assert!(!result.core.saveable);
    assert!(!result.core.diagnostics.is_empty());
    assert_eq!(result.applied_outputs.len(), 2);

    for invalid_title in ["x".repeat(161), "Two\nlines".into(), "Nul\0title".into()] {
        request.draft.outputs[0].title = invalid_title;
        let result = application::preview(&request, &inputs(&fixture), &heads(case)).unwrap();
        assert!(!result.saveable);
        assert_eq!(result.applied_outputs.len(), 2);
    }
}

#[test]
fn wire_shape_rejects_unknown_missing_duplicate_and_oversized_data() {
    let fixture = fixture();
    let mut value = serde_json::to_value(request(&fixture, &fixture["cases"][2])).unwrap();
    let base = serde_json::to_string(&value).unwrap();
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("targets");
    assert!(application::decode_request_json(&serde_json::to_vec(&missing).unwrap()).is_err());
    let mut unknown = value.clone();
    unknown["targets"][0]["unexpected"] = json!("private source marker");
    let error =
        application::decode_request_json(&serde_json::to_vec(&unknown).unwrap()).unwrap_err();
    assert!(!error.to_string().contains("private source marker"));
    let duplicate = base.replacen("\"targets\":", "\"targets\":[],\"targets\":", 1);
    assert!(application::decode_request_json(duplicate.as_bytes()).is_err());
    let escaped_duplicate = base.replacen("\"targets\":", "\"targ\\u0065ts\":[],\"targets\":", 1);
    assert!(application::decode_request_json(escaped_duplicate.as_bytes()).is_err());
    value["targets"] = Value::Array(vec![json!({"kind":"new"}); 17]);
    assert!(application::decode_request_json(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(
        application::decode_request_json(&vec![
            b' ';
            rangoon_compose::MAX_DRAFT_BYTES + 16 * 1024 + 1
        ])
        .is_err()
    );

    let mut request = request(&fixture, &fixture["cases"][2]);
    request.draft.outputs[0].title = "x".repeat(rangoon_compose::MAX_DRAFT_BYTES + 1);
    let encoded = serde_json::to_vec(&request).unwrap();
    assert!(encoded.len() < rangoon_compose::MAX_DRAFT_BYTES + 16 * 1024);
    assert!(application::decode_request_json(&encoded).is_err());
    assert!(
        application::preview(&request, &inputs(&fixture), &heads(&fixture["cases"][2])).is_err()
    );
}
