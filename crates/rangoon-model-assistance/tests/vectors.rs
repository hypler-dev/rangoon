//! Independent Python vectors bind canonical bytes, range accounting and identities.
use rangoon_model_assistance::{prepare_pack, validate_response};
use serde_json::Value;

#[test]
fn independent_context_and_response_vectors_match_exactly() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/context-v1.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["schemaVersion"],
        "rangoon.model-context-fixtures.v1"
    );
    let vectors = fixture["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 6);
    for vector in vectors {
        let name = vector["name"].as_str().unwrap();
        let request = serde_json::to_vec(&vector["request"]).unwrap();
        let pack = prepare_pack(&request).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            serde_json::to_value(&pack).unwrap(),
            vector["expectedPack"],
            "{name}: exact body bytes, framing, counters and pack identity"
        );
        let response_json = vector["validResponseJson"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(response_json).unwrap(),
            vector["validResponse"],
            "{name}: raw response fixture agrees with its readable record"
        );
        let response = validate_response(&pack, response_json.as_bytes())
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            vector["expectedResponse"],
            "{name}: exact received-byte digest and locally bound metadata"
        );
    }
}
