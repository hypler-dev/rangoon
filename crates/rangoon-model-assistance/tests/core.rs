use rangoon_domain::byte_digest;
use rangoon_model_assistance::{Diagnostic, prepare_pack, validate_response};
use serde_json::{Value, json};

fn identifier(kind: &str, byte: char) -> String {
    format!("{kind}:{}", byte.to_string().repeat(64))
}

fn source_reference(content: &str, id_byte: char) -> Value {
    json!({
        "kind": "source",
        "sourceId": identifier("source", id_byte),
        "sha256": byte_digest(content.as_bytes()),
    })
}

fn request(task: &str, inputs: Vec<Value>, max_body_bytes: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.context-pack-request.v1",
        "task": task,
        "target": {
            "profileId": "local.profile-1",
            "profileSha256": "a".repeat(64),
            "model": "test-model.v1",
            "maxOutputTokens": 256,
        },
        "maxBodyBytes": max_body_bytes,
        "inputs": inputs,
    }))
    .unwrap()
}

fn input(
    content: &str,
    id_byte: char,
    scope: &str,
    selections: Value,
    required_protected_ranges: Value,
) -> Value {
    json!({
        "input": source_reference(content, id_byte),
        "scope": scope,
        "content": content,
        "selections": selections,
        "requiredProtectedRanges": required_protected_ranges,
    })
}

#[test]
fn canonical_pack_preserves_union_protection_omission_and_exact_deduplication() {
    let content = "αbcDEFghi";
    let raw = request(
        "classify_v1",
        vec![
            input(
                content,
                'a',
                "operator-selected",
                json!([
                    {"startByte": 0, "endByte": 4, "protected": false},
                    {"startByte": 2, "endByte": 4, "protected": true},
                    {"startByte": 4, "endByte": 6, "protected": false},
                ]),
                json!([{"startByte": 2, "endByte": 4}]),
            ),
            input(
                content,
                'b',
                "different-owner-context",
                json!([{"startByte": 0, "endByte": 6, "protected": false}]),
                json!([]),
            ),
        ],
        8_192,
    );

    let pack = prepare_pack(&raw).unwrap();
    let serialized = serde_json::to_value(&pack).unwrap();
    let body: Value = serde_json::from_str(serialized["bodyJson"].as_str().unwrap()).unwrap();

    assert_eq!(
        body["inputs"][0]["selectedRanges"],
        json!([{"startByte": 0, "endByte": 6}])
    );
    assert_eq!(
        body["inputs"][0]["protectedRanges"],
        json!([{"startByte": 2, "endByte": 4}])
    );
    assert_eq!(
        body["inputs"][0]["requiredProtectedRanges"],
        json!([{"startByte": 2, "endByte": 4}])
    );
    assert_eq!(
        body["inputs"][0]["omittedRanges"],
        json!([{"startByte": 6, "endByte": 10}])
    );
    assert_eq!(body["blocks"].as_array().unwrap().len(), 1);
    assert_eq!(body["blocks"][0]["text"], "αbcDE");
    assert_eq!(body["blocks"][0]["aliases"].as_array().unwrap().len(), 2);
    assert_eq!(body["blocks"][0]["aliases"][0]["protected"], true);
    assert_eq!(serialized["selectedBytes"], 12);
    assert_eq!(serialized["uniqueTextBytes"], 6);
    assert_eq!(serialized["omittedBytes"], 8);
    assert_eq!(serialized["tokenAccounting"], "unknown");
    assert_eq!(serialized["authority"], "none");
}

#[test]
fn templates_bind_each_task_and_change_pack_identity() {
    let content = "retain negative conditions";
    let mut ids = Vec::new();
    for task in ["classify_v1", "decompose_v1", "compare_v1"] {
        let inputs = if task == "compare_v1" {
            vec![
                json!({
                    "input": {
                        "kind": "revision",
                        "capabilityId": identifier("capability", 'c'),
                        "revisionId": identifier("revision", 'd'),
                        "sha256": byte_digest(content.as_bytes()),
                    },
                    "scope": "",
                    "content": content,
                    "selections": [{"startByte":0,"endByte":content.len(),"protected":false}],
                    "requiredProtectedRanges": [],
                }),
                json!({
                    "input": {
                        "kind": "revision",
                        "capabilityId": identifier("capability", 'e'),
                        "revisionId": identifier("revision", 'f'),
                        "sha256": byte_digest(content.as_bytes()),
                    },
                    "scope": "",
                    "content": content,
                    "selections": [{"startByte":0,"endByte":content.len(),"protected":false}],
                    "requiredProtectedRanges": [],
                }),
            ]
        } else {
            vec![input(
                content,
                'a',
                "",
                json!([{"startByte": 0, "endByte": content.len(), "protected": false}]),
                json!([]),
            )]
        };
        let pack = prepare_pack(&request(task, inputs, 8_192)).unwrap();
        let serialized = serde_json::to_value(pack).unwrap();
        let body: Value = serde_json::from_str(serialized["bodyJson"].as_str().unwrap()).unwrap();
        let expected_header = format!("Task: {task}");
        assert_eq!(
            body["instructions"].as_str().unwrap().lines().next(),
            Some(expected_header.as_str())
        );
        assert!(body["instructions"].as_str().unwrap().ends_with('\n'));
        ids.push(serialized["packId"].as_str().unwrap().to_owned());
    }
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
}

#[test]
fn response_is_bound_to_selected_bytes_and_local_pack_metadata() {
    let content = "selected omitted";
    let raw = request(
        "classify_v1",
        vec![input(
            content,
            'a',
            "",
            json!([{"startByte": 0, "endByte": 8, "protected": false}]),
            json!([]),
        )],
        8_192,
    );
    let pack = prepare_pack(&raw).unwrap();
    let response = serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.analysis-proposals.v1",
        "task": "classify_v1",
        "proposals": [{
            "kind": "classification",
            "title": "A title",
            "authoredText": "<em>inert</em>",
            "explanation": "Evidence is selected.",
            "citations": [{
                "input": source_reference(content, 'a'),
                "startByte": 0,
                "endByte": 8,
            }],
        }],
        "uncertainties": ["Only selected bytes were evaluated."],
    }))
    .unwrap();

    let validated = validate_response(&pack, &response).unwrap();
    let value = serde_json::to_value(validated).unwrap();
    let pack_value = serde_json::to_value(&pack).unwrap();
    assert_eq!(value["packId"], pack_value["packId"]);
    assert_eq!(value["responseSha256"], byte_digest(&response));
    assert_eq!(value["contentKind"], "model_authored");
    assert_eq!(value["authority"], "none");

    let omitted = serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.analysis-proposals.v1",
        "task": "classify_v1",
        "proposals": [{
            "kind": "classification", "title": "A", "authoredText": "", "explanation": "B",
            "citations": [{"input": source_reference(content, 'a'), "startByte": 9, "endByte": 14}],
        }],
        "uncertainties": [],
    }))
    .unwrap();
    assert!(matches!(
        validate_response(&pack, &omitted),
        Err(Diagnostic::ResponseInvalid)
    ));
}

#[test]
fn invalid_required_protection_identity_and_budget_fail_closed() {
    let content = "αbc";
    let required_not_selected = request(
        "classify_v1",
        vec![input(
            content,
            'a',
            "",
            json!([{"startByte": 0, "endByte": 2, "protected": false}]),
            json!([{"startByte": 2, "endByte": 4}]),
        )],
        8_192,
    );
    assert!(matches!(
        prepare_pack(&required_not_selected),
        Err(Diagnostic::RangeInvalid)
    ));

    let mut wrong_digest = input(
        content,
        'a',
        "",
        json!([{"startByte": 0, "endByte": 2, "protected": false}]),
        json!([]),
    );
    wrong_digest["input"]["sha256"] = json!("f".repeat(64));
    assert!(matches!(
        prepare_pack(&request("classify_v1", vec![wrong_digest], 8_192)),
        Err(Diagnostic::IdentityMismatch)
    ));

    let valid = input(
        content,
        'a',
        "",
        json!([{"startByte": 0, "endByte": 2, "protected": false}]),
        json!([]),
    );
    assert!(matches!(
        prepare_pack(&request("classify_v1", vec![valid], 1)),
        Err(Diagnostic::PackOverBudget)
    ));
}
