use rangoon_model_assistance::prepare_pack;
use rangoon_model_cloud::{CloudProfile, Diagnostic, PreparedRequest};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/cloud-v1.json"
    ))
    .unwrap()
}
fn profile(v: &Value) -> CloudProfile {
    CloudProfile::parse(&serde_json::to_vec(&v["profileRequest"]).unwrap()).unwrap()
}
#[test]
fn independent_python_profile_pack_and_wire_vectors() {
    let v = fixture();
    let p = profile(&v);
    assert_eq!(p.profile_sha256(), v["profileSha256"].as_str().unwrap());
    let pack = prepare_pack(&serde_json::to_vec(&v["contextRequest"]).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&pack).unwrap(), v["expectedPack"]);
    let prepared =
        PreparedRequest::new(&p, pack, v["credentialRevision"].as_str().unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&prepared).unwrap(),
        v["expectedPreparedRequest"]
    );
    assert!(!prepared.body_json().contains("credentialRevision"));
}
#[test]
fn credential_revision_and_all_target_fields_bind_the_request() {
    let v = fixture();
    let p = profile(&v);
    let pack = || prepare_pack(&serde_json::to_vec(&v["contextRequest"]).unwrap()).unwrap();
    let a = PreparedRequest::new(&p, pack(), &"a".repeat(64)).unwrap();
    let b = PreparedRequest::new(&p, pack(), &"b".repeat(64)).unwrap();
    assert_ne!(a.request_id(), b.request_id());
    assert_eq!(a.body_json(), b.body_json());
    for (k, value) in [
        ("profileId", json!("other")),
        ("profileSha256", json!("f".repeat(64))),
        ("model", json!("other")),
        ("maxOutputTokens", json!(513)),
    ] {
        let mut r = v["contextRequest"].clone();
        r["target"][k] = value;
        let pack = prepare_pack(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert!(matches!(
            PreparedRequest::new(&p, pack, &"a".repeat(64)),
            Err(Diagnostic::ProfileMismatch)
        ));
    }
    for revision in ["A".repeat(64), "a".repeat(63), "a".repeat(65)] {
        assert!(matches!(
            PreparedRequest::new(&p, pack(), &revision),
            Err(Diagnostic::InvalidCredential)
        ));
    }
}
#[test]
fn closed_profile_fields_and_exact_bounds() {
    let v = fixture();
    let r = v["profileRequest"].clone();
    let raw = serde_json::to_vec(&r).unwrap();
    let mut exact = raw.clone();
    exact.resize(1024, b' ');
    assert!(CloudProfile::parse(&exact).is_ok());
    exact.push(b' ');
    assert!(CloudProfile::parse(&exact).is_err());
    for (k, yes, no) in [
        ("profileId", json!("a".repeat(64)), json!("a".repeat(65))),
        ("model", json!("a".repeat(128)), json!("a".repeat(129))),
        ("maxOutputTokens", json!(32768), json!(32769)),
    ] {
        let mut r = r.clone();
        r[k] = yes;
        assert!(CloudProfile::parse(&serde_json::to_vec(&r).unwrap()).is_ok());
        r[k] = no;
        assert!(CloudProfile::parse(&serde_json::to_vec(&r).unwrap()).is_err());
    }
    for (k, value) in [
        ("origin", json!("https://wrong.example")),
        ("key", json!("synthetic")),
        ("model", json!("name\r\nHeader: x")),
        ("model", json!("a/b")),
        ("model", json!(".hidden")),
        ("model", json!("")),
        ("maxOutputTokens", json!(0)),
        ("maxOutputTokens", json!(1.5)),
    ] {
        let mut r = r.clone();
        r[k] = value;
        assert!(CloudProfile::parse(&serde_json::to_vec(&r).unwrap()).is_err());
    }
    let raw = String::from_utf8(raw).unwrap();
    let duplicate = raw.replacen('{', "{\"model\":\"other\",", 1);
    assert!(CloudProfile::parse(duplicate.as_bytes()).is_err());
    assert!(CloudProfile::parse(format!("{raw} trailing").as_bytes()).is_err());
    assert!(CloudProfile::parse(&[0xff]).is_err());
}
fn ascii(v: &Value, p: &CloudProfile, n: usize) -> Result<PreparedRequest, Diagnostic> {
    let content = "a".repeat(n);
    let mut r = v["contextRequest"].clone();
    r["inputs"][0]["input"]["sha256"] = json!(rangoon_domain::byte_digest(content.as_bytes()));
    r["inputs"][0]["content"] = json!(content);
    r["inputs"][0]["selections"] = json!([{"startByte":0,"endByte":n,"protected":false}]);
    r["inputs"][0]["requiredProtectedRanges"] = json!([]);
    PreparedRequest::new(
        p,
        prepare_pack(&serde_json::to_vec(&r).unwrap()).unwrap(),
        &"a".repeat(64),
    )
}
#[test]
fn final_serialized_body_exact_byte_cap() {
    let v = fixture();
    let p = profile(&v);
    let base = ascii(&v, &p, 250_000).unwrap();
    let n = 250_000 + 262_144 - base.body_json().len();
    assert_eq!(ascii(&v, &p, n).unwrap().body_json().len(), 262_144);
    assert!(matches!(
        ascii(&v, &p, n + 1),
        Err(Diagnostic::RequestOverBudget)
    ));
}
