//! Independent fixture assertions for local profile and prepared-request identity.
use rangoon_model_assistance::prepare_pack;
use rangoon_model_local::{Diagnostic, LocalProfile, PreparedRequest};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/local-v1.json"
    ))
    .unwrap()
}

fn only_vector() -> Value {
    let fixture = fixture();
    assert_eq!(fixture["schemaVersion"], "rangoon.model-local-fixtures.v1");
    fixture["vectors"]
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .clone()
}

fn profile_from(vector: &Value) -> LocalProfile {
    LocalProfile::parse(&serde_json::to_vec(&vector["profileRequest"]).unwrap()).unwrap()
}

#[test]
fn independent_profile_pack_and_prepared_request_vectors_match_exact_bytes() {
    let vector = only_vector();
    let expected_profile = &vector["expectedProfile"];
    let profile = profile_from(&vector);
    assert_eq!(
        profile.profile_id(),
        expected_profile["profileId"].as_str().unwrap()
    );
    assert_eq!(
        profile.profile_sha256(),
        expected_profile["profileSha256"].as_str().unwrap()
    );
    assert_eq!(profile.model(), expected_profile["model"].as_str().unwrap());
    assert_eq!(
        profile.max_output_tokens(),
        expected_profile["maxOutputTokens"].as_u64().unwrap()
    );

    let pack = prepare_pack(&serde_json::to_vec(&vector["contextRequest"]).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&pack).unwrap(), vector["expectedPack"]);
    let prepared = PreparedRequest::new(&profile, pack).unwrap();
    assert_eq!(
        serde_json::to_value(&prepared).unwrap(),
        vector["expectedPreparedRequest"]
    );
}

#[test]
fn changed_profile_target_or_model_fails_closed_before_preparation() {
    let vector = only_vector();
    let profile = profile_from(&vector);
    for (field, value) in [
        ("profileId", json!("other-profile")),
        ("profileSha256", json!("a".repeat(64))),
        ("model", json!("other-model:v2")),
        ("maxOutputTokens", json!(513)),
    ] {
        let mut request = vector["contextRequest"].clone();
        request["target"][field] = value;
        let pack = prepare_pack(&serde_json::to_vec(&request).unwrap()).unwrap();
        assert!(matches!(
            PreparedRequest::new(&profile, pack),
            Err(Diagnostic::ProfileMismatch)
        ));
    }

    let pack = prepare_pack(&serde_json::to_vec(&vector["contextRequest"]).unwrap()).unwrap();
    let mut changed_profile_request = vector["profileRequest"].clone();
    changed_profile_request["model"] = json!("other-model:v2");
    let changed_profile =
        LocalProfile::parse(&serde_json::to_vec(&changed_profile_request).unwrap()).unwrap();
    assert!(matches!(
        PreparedRequest::new(&changed_profile, pack),
        Err(Diagnostic::ProfileMismatch)
    ));
}

#[test]
fn defined_profile_port_byte_and_field_limits_are_enforced() {
    let vector = only_vector();
    for port in [1, 65_535] {
        let mut request = vector["profileRequest"].clone();
        request["port"] = json!(port);
        assert!(LocalProfile::parse(&serde_json::to_vec(&request).unwrap()).is_ok());
    }

    let raw = serde_json::to_string(&vector["profileRequest"]).unwrap();
    let exact_1024 = format!("{}{}", " ".repeat(1024 - raw.len()), raw);
    assert_eq!(exact_1024.len(), 1024);
    assert!(LocalProfile::parse(exact_1024.as_bytes()).is_ok());
    let over_1024 = format!(" {exact_1024}");
    assert_eq!(over_1024.len(), 1025);
    assert!(matches!(
        LocalProfile::parse(over_1024.as_bytes()),
        Err(Diagnostic::InvalidProfile)
    ));

    for (field, accepted, rejected) in [
        ("profileId", json!("a".repeat(64)), json!("a".repeat(65))),
        (
            "model",
            json!(format!("{}:a", "a".repeat(126))),
            json!(format!("{}:a", "a".repeat(127))),
        ),
        ("maxOutputTokens", json!(32_768), json!(32_769)),
    ] {
        let mut request = vector["profileRequest"].clone();
        request[field] = accepted;
        assert!(LocalProfile::parse(&serde_json::to_vec(&request).unwrap()).is_ok());
        request[field] = rejected;
        assert!(matches!(
            LocalProfile::parse(&serde_json::to_vec(&request).unwrap()),
            Err(Diagnostic::InvalidProfile)
        ));
    }
}

#[test]
fn invalid_hosts_ports_models_and_closed_json_profiles_fail_closed() {
    let invalid = [
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"localhost","port":11434,"model":"fixture-model:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":0,"model":"fixture-model:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":65536,"model":"fixture-model:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":".dot:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":"namespace/.dot:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":"/absolute:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":"one:two:three","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":"fixture-model:v1","maxOutputTokens":512,"extra":true}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","profileId":"other","host":"127.0.0.1","port":11434,"model":"fixture-model:v1","maxOutputTokens":512}"#,
        r#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local-analysis","host":"127.0.0.1","port":11434,"model":"fixture-model:v1","maxOutputTokens":512} trailing"#,
    ];
    for raw in invalid {
        assert!(matches!(
            LocalProfile::parse(raw.as_bytes()),
            Err(Diagnostic::InvalidProfile)
        ));
    }
}

#[test]
fn final_wrapper_accepts_exact_budget_and_rejects_one_byte_over() {
    let vector = only_vector();
    let profile = profile_from(&vector);
    let prepared = prepared_ascii_request(&vector, &profile, 259_436).unwrap();
    assert_eq!(prepared.body_json().len(), 262_144);
    assert!(matches!(
        prepared_ascii_request(&vector, &profile, 259_437),
        Err(Diagnostic::RequestOverBudget)
    ));
}

fn prepared_ascii_request(
    vector: &Value,
    profile: &LocalProfile,
    content_bytes: usize,
) -> Result<PreparedRequest, Diagnostic> {
    let content = "a".repeat(content_bytes);
    let digest = rangoon_domain::byte_digest(content.as_bytes());
    let mut request = vector["contextRequest"].clone();
    request["inputs"][0]["input"]["sha256"] = json!(digest);
    request["inputs"][0]["content"] = json!(content);
    request["inputs"][0]["selections"] = json!([{
        "startByte": 0,
        "endByte": content_bytes,
        "protected": false
    }]);
    request["inputs"][0]["requiredProtectedRanges"] = json!([]);
    let pack = prepare_pack(&serde_json::to_vec(&request).unwrap()).unwrap();
    PreparedRequest::new(profile, pack)
}
