use rangoon_domain::byte_digest;
use rangoon_model_assistance::{Diagnostic, prepare_pack, validate_response};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/context-v1.json"
    ))
    .unwrap()
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

#[test]
fn declared_body_and_output_limits_accept_maximum_and_reject_outside_bounds() {
    let mut request = fixture()["vectors"][0]["request"].clone();
    request["maxBodyBytes"] = json!(262_144);
    request["target"]["maxOutputTokens"] = json!(32_768);
    assert!(prepare_pack(&encode(&request)).is_ok());
    for (pointer, values) in [
        ("/maxBodyBytes", [0, 262_145]),
        ("/target/maxOutputTokens", [0, 32_769]),
    ] {
        for value in values {
            let mut invalid = request.clone();
            *invalid.pointer_mut(pointer).unwrap() = json!(value);
            assert!(matches!(
                prepare_pack(&encode(&invalid)),
                Err(Diagnostic::InputInvalid)
            ));
        }
    }
}

#[test]
fn every_request_object_rejects_unknown_missing_and_null_fields() {
    let request = fixture()["vectors"][0]["request"].clone();
    for pointer in [
        "",
        "/target",
        "/inputs/0",
        "/inputs/0/input",
        "/inputs/0/selections/0",
        "/inputs/0/requiredProtectedRanges/0",
    ] {
        let object = request.pointer(pointer).unwrap().as_object().unwrap();
        for key in object.keys() {
            let mut missing = request.clone();
            missing
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                prepare_pack(&encode(&missing)).is_err(),
                "missing {pointer}/{key}"
            );
            let mut null = request.clone();
            null.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), Value::Null);
            assert!(
                prepare_pack(&encode(&null)).is_err(),
                "null {pointer}/{key}"
            );
        }
        let mut unknown = request.clone();
        unknown
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("authority".into(), json!("granted"));
        assert!(
            matches!(
                prepare_pack(&encode(&unknown)),
                Err(Diagnostic::InputInvalid)
            ),
            "unknown field {pointer}"
        );
    }
}

#[test]
fn every_response_object_rejects_unknown_missing_and_null_fields() {
    let vector = fixture()["vectors"][0].clone();
    let pack = prepare_pack(&encode(&vector["request"])).unwrap();
    let response = &vector["validResponse"];
    for pointer in [
        "",
        "/proposals/0",
        "/proposals/0/citations/0",
        "/proposals/0/citations/0/input",
    ] {
        let object = response.pointer(pointer).unwrap().as_object().unwrap();
        for key in object.keys() {
            let mut missing = response.clone();
            missing
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                validate_response(&pack, &encode(&missing)).is_err(),
                "missing {pointer}/{key}"
            );
            let mut null = response.clone();
            null.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), Value::Null);
            assert!(
                validate_response(&pack, &encode(&null)).is_err(),
                "null {pointer}/{key}"
            );
        }
        let mut unknown = response.clone();
        unknown
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("execute".into(), json!(true));
        assert!(
            matches!(
                validate_response(&pack, &encode(&unknown)),
                Err(Diagnostic::ResponseInvalid)
            ),
            "unknown field {pointer}"
        );
    }
}

#[test]
fn huge_offsets_foreign_digests_and_task_confusion_never_return_success() {
    let vector = fixture()["vectors"][0].clone();
    let request = &vector["request"];
    let pack = prepare_pack(&encode(request)).unwrap();
    let mut huge = request.clone();
    huge["inputs"][0]["selections"][0]["endByte"] = json!(u64::MAX);
    assert!(matches!(
        prepare_pack(&encode(&huge)),
        Err(Diagnostic::RangeInvalid)
    ));
    for (pointer, bad) in [
        ("/task", json!("compare_v1")),
        ("/proposals/0/kind", json!("difference")),
        ("/proposals/0/citations/0/endByte", json!(u64::MAX)),
        (
            "/proposals/0/citations/0/input/sha256",
            json!("0".repeat(64)),
        ),
    ] {
        let mut response = vector["validResponse"].clone();
        *response.pointer_mut(pointer).unwrap() = bad;
        assert!(
            validate_response(&pack, &encode(&response)).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn exactly_fitting_body_is_accepted_but_one_byte_less_never_truncates() {
    let mut request = fixture()["vectors"][0]["request"].clone();
    // The configured budget is itself serialized, so find its stable decimal width.
    for _ in 0..3 {
        let pack = prepare_pack(&encode(&request)).unwrap();
        request["maxBodyBytes"] = serde_json::to_value(pack).unwrap()["bodyBytes"].clone();
    }
    let exact = serde_json::to_value(prepare_pack(&encode(&request)).unwrap()).unwrap();
    assert_eq!(exact["bodyBytes"], request["maxBodyBytes"]);
    request["maxBodyBytes"] = json!(request["maxBodyBytes"].as_u64().unwrap() - 1);
    assert!(matches!(
        prepare_pack(&encode(&request)),
        Err(Diagnostic::PackOverBudget)
    ));
}

#[test]
fn maximum_aliases_and_omissions_preserve_all_selected_ranges() {
    let mut request = fixture()["vectors"][0]["request"].clone();
    request["maxBodyBytes"] = json!(262_144);
    let mut inputs = Vec::new();
    for index in 0..16 {
        let count = if index == 0 { 241 } else { 1 };
        let content = format!("{}-", "-x".repeat(count));
        let selections: Vec<_> = (0..count)
            .map(|i| json!({"startByte":i*2+1,"endByte":i*2+2,"protected":false}))
            .collect();
        inputs.push(json!({
            "input":{"kind":"source","sourceId":format!("source:{index:064x}"),"sha256":byte_digest(content.as_bytes())},
            "scope":format!("scope-{index}"),"content":content,"requiredProtectedRanges":[],"selections":selections,
        }));
    }
    request["inputs"] = json!(inputs);
    let pack = serde_json::to_value(prepare_pack(&encode(&request)).unwrap()).unwrap();
    let body: Value = serde_json::from_str(pack["bodyJson"].as_str().unwrap()).unwrap();
    assert_eq!(pack["selectedBytes"], 256);
    assert_eq!(pack["uniqueTextBytes"], 1);
    assert_eq!(pack["omittedBytes"], 272);
    assert_eq!(body["blocks"].as_array().unwrap().len(), 1);
    assert_eq!(body["blocks"][0]["aliases"].as_array().unwrap().len(), 256);
    assert_eq!(
        body["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["omittedRanges"].as_array().unwrap().len())
            .sum::<usize>(),
        272
    );
    request["inputs"][0]["selections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"startByte":0,"endByte":1,"protected":false}));
    assert!(matches!(
        prepare_pack(&encode(&request)),
        Err(Diagnostic::InputLimit)
    ));
}

#[test]
fn maximum_unique_blocks_and_required_protection_remain_independent() {
    let mut request = fixture()["vectors"][0]["request"].clone();
    request["maxBodyBytes"] = json!(262_144);
    let mut content = String::new();
    let mut selections = Vec::new();
    let mut required = Vec::new();
    for index in 0..256 {
        let start = content.len();
        content.push(char::from_u32(0x100 + index).unwrap());
        let end = content.len();
        content.push('-');
        selections.push(json!({"startByte":start,"endByte":end,"protected":true}));
        required.push(json!({"startByte":start,"endByte":end}));
    }
    request["inputs"] = json!([{
        "input":{"kind":"source","sourceId":format!("source:{}", "a".repeat(64)),"sha256":byte_digest(content.as_bytes())},
        "scope":"all selected text is required", "content":content,
        "requiredProtectedRanges":required, "selections":selections,
    }]);
    let pack = serde_json::to_value(prepare_pack(&encode(&request)).unwrap()).unwrap();
    let body: Value = serde_json::from_str(pack["bodyJson"].as_str().unwrap()).unwrap();
    assert_eq!(body["blocks"].as_array().unwrap().len(), 256);
    assert_eq!(
        body["inputs"][0]["requiredProtectedRanges"]
            .as_array()
            .unwrap()
            .len(),
        256
    );
    assert_eq!(
        body["inputs"][0]["protectedRanges"]
            .as_array()
            .unwrap()
            .len(),
        256
    );
    assert_eq!(pack["selectedBytes"], pack["uniqueTextBytes"]);
    request["inputs"][0]["selections"][255]["protected"] = json!(false);
    assert!(matches!(
        prepare_pack(&encode(&request)),
        Err(Diagnostic::RangeInvalid)
    ));
}
