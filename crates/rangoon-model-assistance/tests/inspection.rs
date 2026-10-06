use rangoon_domain::byte_digest;
use rangoon_model_assistance::{
    InspectionDiagnostic, inspect_proposal, prepare_pack, validate_response,
};
use serde_json::{Value, json};

fn identifier(kind: &str, byte: char) -> String {
    format!("{kind}:{}", byte.to_string().repeat(64))
}

fn source(content: &str, byte: char) -> Value {
    json!({
        "kind": "source",
        "sourceId": identifier("source", byte),
        "sha256": byte_digest(content.as_bytes()),
    })
}

fn input(content: &str, byte: char, scope: &str, selections: Value) -> Value {
    json!({
        "input": source(content, byte),
        "scope": scope,
        "content": content,
        "selections": selections,
        "requiredProtectedRanges": [],
    })
}

fn pack(
    task: &str,
    inputs: Vec<Value>,
    model: &str,
    profile: &str,
) -> rangoon_model_assistance::ContextPack {
    prepare_pack(
        &serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.context-pack-request.v1",
            "task": task,
            "target": {
                "profileId": profile,
                "profileSha256": "a".repeat(64),
                "model": model,
                "maxOutputTokens": 512,
            },
            "maxBodyBytes": 262_144,
            "inputs": inputs,
        }))
        .unwrap(),
    )
    .unwrap()
}

fn response(task: &str, content: &str, source_byte: char, titles: &[&str]) -> Vec<u8> {
    let kind = match task {
        "classify_v1" => "classification",
        "decompose_v1" => "capability",
        "compare_v1" => "difference",
        _ => unreachable!(),
    };
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.analysis-proposals.v1",
        "task": task,
        "proposals": titles.iter().map(|title| json!({
            "kind": kind,
            "title": title,
            "authoredText": "<a href='https://untrusted.invalid'>inert</a> rm -rf /",
            "explanation": "Only exact selected bytes support this proposal.",
            "citations": [{
                "input": source(content, source_byte),
                "startByte": 0,
                "endByte": content.len(),
            }],
        })).collect::<Vec<_>>(),
        "uncertainties": ["No semantic authority is inferred."],
    }))
    .unwrap()
}

fn inspection_value(
    pack: &rangoon_model_assistance::ContextPack,
    response_bytes: &[u8],
    proposal_index: u32,
) -> Value {
    let response = validate_response(pack, response_bytes).unwrap();
    serde_json::to_value(inspect_proposal(pack, &response, proposal_index).unwrap()).unwrap()
}

#[test]
fn inspection_uses_independent_fixed_framing_and_exact_field_order() {
    let content = "selected model evidence";
    let context = pack(
        "classify_v1",
        vec![input(
            content,
            'a',
            "operator-selected",
            json!([{"startByte": 0, "endByte": content.len(), "protected": false}]),
        )],
        "model-v1",
        "local.profile-1",
    );
    let response_bytes = response("classify_v1", content, 'a', &["First"]);
    let response = validate_response(&context, &response_bytes).unwrap();
    let inspection = inspect_proposal(&context, &response, 0).unwrap();
    let serialized = serde_json::to_string(&inspection).unwrap();
    let value: Value = serde_json::from_str(&serialized).unwrap();
    let id = value["inspectionId"].as_str().unwrap();

    assert!(
        serialized
            .starts_with("{\"schemaVersion\":\"rangoon.proposal-inspection.v1\",\"inspectionId\":")
    );
    let omitted_id = serialized.replacen(&format!("\"inspectionId\":\"{id}\","), "", 1);
    let mut framed = b"rangoon.proposal-inspection.v1\0".to_vec();
    framed.extend_from_slice(&(omitted_id.len() as u64).to_be_bytes());
    framed.extend_from_slice(omitted_id.as_bytes());
    assert_eq!(id, format!("inspection:{}", byte_digest(&framed)));
    assert_eq!(value["responseSha256"], byte_digest(&response_bytes));
    assert_eq!(value["contentKind"], "model_authored");
    assert_eq!(value["authority"], "none");
    assert!(serialized.len() <= 512 * 1024);
}

#[test]
fn inspection_identity_binds_pack_metadata_input_response_and_selector() {
    let content = "same selected content";
    let base_input = || {
        input(
            content,
            'a',
            "scope-a",
            json!([{"startByte": 0, "endByte": content.len(), "protected": false}]),
        )
    };
    let base = pack(
        "classify_v1",
        vec![base_input()],
        "model-v1",
        "local.profile-1",
    );
    let baseline = inspection_value(
        &base,
        &response("classify_v1", content, 'a', &["One", "Two"]),
        0,
    )["inspectionId"]
        .as_str()
        .unwrap()
        .to_owned();

    let cases = [
        inspection_value(
            &pack("classify_v1", vec![base_input()], "model-v2", "local.profile-1"),
            &response("classify_v1", content, 'a', &["One", "Two"]),
            0,
        ),
        inspection_value(
            &pack("classify_v1", vec![base_input()], "model-v1", "local.profile-2"),
            &response("classify_v1", content, 'a', &["One", "Two"]),
            0,
        ),
        inspection_value(
            &pack(
                "classify_v1",
                vec![input(
                    content,
                    'a',
                    "scope-b",
                    json!([{"startByte": 0, "endByte": content.len() - 1, "protected": false}]),
                )],
                "model-v1",
                "local.profile-1",
            ),
            &serde_json::to_vec(&json!({
                "schemaVersion": "rangoon.analysis-proposals.v1", "task": "classify_v1",
                "proposals": [{"kind":"classification", "title":"One", "authoredText":"inert", "explanation":"selected", "citations":[{"input":source(content, 'a'), "startByte":0, "endByte":content.len() - 1}]}],
                "uncertainties": [],
            })).unwrap(),
            0,
        ),
        inspection_value(
            &base,
            &response("classify_v1", content, 'a', &["Changed", "Two"]),
            0,
        ),
        inspection_value(
            &base,
            &response("classify_v1", content, 'a', &["One", "Two"]),
            1,
        ),
    ];
    for changed in cases {
        assert_ne!(changed["inspectionId"], baseline);
    }

    let task_content = "task binds template";
    let decompose = pack(
        "decompose_v1",
        vec![input(
            task_content,
            'b',
            "scope-a",
            json!([{"startByte": 0, "endByte": task_content.len(), "protected": false}]),
        )],
        "model-v1",
        "local.profile-1",
    );
    let changed_task = inspection_value(
        &decompose,
        &response("decompose_v1", task_content, 'b', &["One"]),
        0,
    );
    assert_ne!(changed_task["inspectionId"], baseline);
    assert_ne!(
        changed_task["templateSha256"],
        inspection_value(
            &base,
            &response("classify_v1", content, 'a', &["One", "Two"]),
            0
        )["templateSha256"]
    );
}

#[test]
fn inspection_preserves_utf8_omission_and_scoped_aliases_without_source_reparse() {
    let content = "\u{feff}αXsame\r\nomit";
    let first_end = "\u{feff}α".len();
    let same_start = first_end + 1;
    let selected_end = same_start + "same".len();
    let mut first_input = input(
        content,
        'a',
        "operator-one",
        json!([
            {"startByte": 0, "endByte": first_end, "protected": true},
            {"startByte": same_start, "endByte": selected_end, "protected": false},
        ]),
    );
    first_input["requiredProtectedRanges"] = json!([{
        "startByte": 0,
        "endByte": first_end,
    }]);
    let context = pack(
        "classify_v1",
        vec![
            first_input,
            input(
                "same",
                'b',
                "operator-two",
                json!([{"startByte": 0, "endByte": 4, "protected": false}]),
            ),
        ],
        "model-v1",
        "local.profile-1",
    );
    let response = serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.analysis-proposals.v1", "task": "classify_v1",
        "proposals": [{
            "kind":"classification", "title":"UTF-8", "authoredText":"<script>inert</script>", "explanation":"exact ranges",
            "citations":[{"input":source(content, 'a'), "startByte":0, "endByte":first_end}],
        }], "uncertainties": ["omitted text remains omitted"],
    })).unwrap();
    let value = inspection_value(&context, &response, 0);
    assert_eq!(value["inputs"][0]["scope"], "operator-one");
    assert_eq!(value["inputs"][0]["byteLength"], content.len());
    assert_eq!(
        value["inputs"][0]["requiredProtectedRanges"],
        json!([{"startByte":0,"endByte":first_end}])
    );
    assert_eq!(
        value["inputs"][0]["selectedRanges"],
        json!([
            {"startByte":0,"endByte":first_end},
            {"startByte":same_start,"endByte":selected_end},
        ])
    );
    assert_eq!(
        value["inputs"][0]["protectedRanges"],
        json!([{"startByte":0,"endByte":first_end}])
    );
    assert_eq!(
        value["inputs"][0]["omittedRanges"],
        json!([
            {"startByte":first_end,"endByte":same_start},
            {"startByte":selected_end,"endByte":content.len()},
        ])
    );
    assert_eq!(
        value["blocks"],
        json!([
            {
                "text": "\u{feff}α",
                "aliases": [{
                    "inputIndex": 0,
                    "startByte": 0,
                    "endByte": first_end,
                    "protected": true,
                }],
            },
            {
                "text": "same",
                "aliases": [
                    {
                        "inputIndex": 0,
                        "startByte": same_start,
                        "endByte": selected_end,
                        "protected": false,
                    },
                    {
                        "inputIndex": 1,
                        "startByte": 0,
                        "endByte": 4,
                        "protected": false,
                    },
                ],
            },
        ])
    );
    assert_eq!(value["proposal"]["authoredText"], "<script>inert</script>");
}

#[test]
fn inspection_rejects_mismatched_packs_and_invalid_selectors_including_empty_proposals() {
    let content = "selected evidence";
    let one = pack(
        "classify_v1",
        vec![input(
            content,
            'a',
            "",
            json!([{"startByte":0,"endByte":content.len(),"protected":false}]),
        )],
        "model-v1",
        "local.profile-1",
    );
    let two = pack(
        "classify_v1",
        vec![input(
            content,
            'a',
            "",
            json!([{"startByte":0,"endByte":content.len(),"protected":false}]),
        )],
        "model-v2",
        "local.profile-1",
    );
    let valid = validate_response(&one, &response("classify_v1", content, 'a', &["One"])).unwrap();
    assert!(matches!(
        inspect_proposal(&two, &valid, 0),
        Err(InspectionDiagnostic::InspectionMismatch)
    ));
    assert!(matches!(
        inspect_proposal(&one, &valid, 1),
        Err(InspectionDiagnostic::ProposalNotFound)
    ));

    let empty = serde_json::to_vec(&json!({
        "schemaVersion":"rangoon.analysis-proposals.v1", "task":"classify_v1", "proposals":[], "uncertainties":[],
    })).unwrap();
    let valid_empty = validate_response(&one, &empty).unwrap();
    assert!(matches!(
        inspect_proposal(&one, &valid_empty, 0),
        Err(InspectionDiagnostic::ProposalNotFound)
    ));
}
