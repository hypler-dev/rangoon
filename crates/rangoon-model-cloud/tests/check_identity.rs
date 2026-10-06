use rangoon_model_cloud::{CheckRequest, CloudProfile, Diagnostic};
use serde_json::{Value, json};

fn profile(model: &str) -> CloudProfile {
    CloudProfile::parse(
        &serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.cloud-profile-request.v1",
            "profileId": "check.fixture",
            "model": model,
            "maxOutputTokens": 512,
        }))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn check_identity_is_deterministic_and_binds_profile_and_revision() {
    let first_profile = profile("synthetic-model-v1");
    let same_profile = profile("synthetic-model-v1");
    let first = CheckRequest::new(&first_profile, &"a".repeat(64)).unwrap();
    let repeated = CheckRequest::new(&same_profile, &"a".repeat(64)).unwrap();
    let changed_revision = CheckRequest::new(&first_profile, &"b".repeat(64)).unwrap();
    let changed_model = CheckRequest::new(&profile("synthetic-model-v2"), &"a".repeat(64)).unwrap();

    // Independently calculated with Python hashlib over the documented framing.
    assert_eq!(
        first.request_id(),
        "3975c845f9d5a539ac7f4c09a9168b168d09c67b66d3b5483bcd91c0e0d1ef90"
    );
    assert_eq!(
        first_profile.profile_sha256(),
        "69bd0bc19526aaaefb62951fe475d6e7ba93304553a9f83353b21641afdab26b"
    );
    assert_eq!(first.request_id(), repeated.request_id());
    assert_ne!(first.request_id(), changed_revision.request_id());
    assert_ne!(first.request_id(), changed_model.request_id());
    assert_eq!(first.path(), "/v1/models/synthetic-model-v1");
    assert_eq!(first.credential_revision(), "a".repeat(64));

    let serialized: Value = serde_json::to_value(&first).unwrap();
    assert_eq!(
        serialized["binding"]["schemaVersion"],
        "rangoon.cloud-check-request.v1"
    );
    assert_eq!(serialized["binding"]["adapter"], "openai-responses.v1");
    assert_eq!(serialized["binding"]["origin"], "https://api.openai.com");
    assert_eq!(serialized["binding"]["method"], "GET");
    assert_eq!(
        serialized["binding"]["path"],
        "/v1/models/synthetic-model-v1"
    );
    assert_eq!(serialized["binding"]["bodyBytes"], 0);
    assert_eq!(
        serialized["binding"]["bodySha256"],
        rangoon_domain::byte_digest(b"")
    );
    assert!(serialized.get("profile").is_none());
    assert!(serialized.to_string().contains(&"a".repeat(64)));
    assert!(!serialized.to_string().contains("source"));
}

#[test]
fn check_identity_rejects_noncanonical_revisions() {
    let profile = profile("synthetic-model-v1");
    for revision in ["A".repeat(64), "a".repeat(63), "a".repeat(65)] {
        assert!(matches!(
            CheckRequest::new(&profile, &revision),
            Err(Diagnostic::InvalidCredential)
        ));
    }
}
