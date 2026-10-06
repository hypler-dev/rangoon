use super::*;
use rangoon_domain::{byte_digest, capability_v1::CapabilityDetail};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-native-compile-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn store(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
    fn skill(&self) -> CapabilityDetail {
        let source =
            rangoon_import::analyze("AGENTS.md", b"# Rules\r\nKeep originals.\r\n").unwrap();
        self.store().save_v1(&source).unwrap();
        self.store()
            .create_capability_v1(&source.source.id, &source.fragments[0].id, "Rules")
            .unwrap()
            .capability
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.0.join("workspace.sqlite3")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request(skill: &CapabilityDetail, profile: Profile) -> Request {
    Request {
        schema_version: REQUEST_SCHEMA.into(),
        capability_id: skill.id.clone(),
        revision_id: skill.revision.id.clone(),
        profile,
    }
}
fn body(value: serde_json::Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(&value).unwrap())
}
fn encoded(store: &Workspace, request: &Request) -> serde_json::Value {
    serde_json::to_value(inspect(store, request).unwrap()).unwrap()
}

#[test]
fn closed_raw_request_rejects_untrusted_fields_and_invalid_selections() {
    let value = serde_json::json!({"schemaVersion": REQUEST_SCHEMA, "capabilityId": format!("capability:{}", "a".repeat(64)), "revisionId": format!("revision:{}", "b".repeat(64)), "profile": "agents_md_v1"});
    assert!(decode_request(&body(value.clone())).is_ok());
    assert!(decode_request(&InvokeBody::Json(value.clone())).is_err());
    assert!(decode_request(&InvokeBody::Raw(vec![255])).is_err());
    let mut padded = serde_json::to_vec(&value).unwrap();
    padded.resize(MAX_REQUEST_BYTES, b' ');
    assert!(decode_request(&InvokeBody::Raw(padded.clone())).is_ok());
    padded.push(b' ');
    assert!(decode_request(&InvokeBody::Raw(padded)).is_err());
    for key in ["schemaVersion", "capabilityId", "revisionId", "profile"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(decode_request(&body(missing)).is_err());
    }
    for key in [
        "content",
        "review",
        "requirements",
        "path",
        "candidateManifestJson",
    ] {
        let mut extra = value.clone();
        extra[key] = true.into();
        assert!(decode_request(&body(extra)).is_err());
    }
    for (key, invalid) in [
        ("schemaVersion", "future"),
        ("capabilityId", "../path"),
        ("revisionId", "revision:BAD"),
        ("profile", "future"),
    ] {
        let mut changed = value.clone();
        changed[key] = invalid.into();
        assert!(decode_request(&body(changed)).is_err());
    }
    let mut duplicate = serde_json::to_string(&value).unwrap();
    duplicate.pop();
    duplicate.push_str(",\"profile\":\"claude_md_v1\"}");
    assert!(decode_request(&InvokeBody::Raw(duplicate.into_bytes())).is_err());
}

#[test]
fn inspection_preserves_bytes_and_observes_review_without_mutation() {
    let dir = Fixture::new();
    let store = dir.store();
    let skill = dir.skill();
    let content = "\u{feff}# Rules\r\nKeep 🦀 exactly\r\nNo final newline";
    let skill = store
        .revise_capability_v1(&skill.id, &skill.latest_revision_id, "Exact", content)
        .unwrap()
        .capability;
    let selected = request(&skill, Profile::AgentsMdV1);
    let before = dir.bytes();
    let unreviewed = encoded(&store, &selected);
    assert_eq!(unreviewed["outcome"], "compiled");
    assert_eq!(unreviewed["schemaVersion"], RESPONSE_SCHEMA);
    assert_eq!(unreviewed["compilation"]["readiness"], "review_required");
    assert!(unreviewed["candidateManifestJson"].is_null());
    assert_eq!(unreviewed["compilation"]["artifact"]["content"], content);
    assert_eq!(dir.bytes(), before);
    store
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    let before = dir.bytes();
    let reviewed = encoded(&store, &selected);
    let manifest = reviewed["candidateManifestJson"].as_str().unwrap();
    assert_eq!(reviewed["compilation"]["readiness"], "candidate");
    assert_eq!(reviewed["compilation"]["authority"], "none");
    assert_eq!(
        reviewed["compilation"]["compilationId"],
        unreviewed["compilation"]["compilationId"]
    );
    assert_eq!(
        byte_digest(manifest.as_bytes()),
        reviewed["compilation"]["candidate"]["manifestSha256"]
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(manifest).unwrap(),
        reviewed["compilation"]["candidate"]["manifest"]
    );
    assert_eq!(dir.bytes(), before);
    let next = store
        .revise_capability_v1(&skill.id, &skill.latest_revision_id, "Next", "Changed")
        .unwrap()
        .capability;
    let before = dir.bytes();
    let historical = encoded(&store, &selected);
    assert_eq!(historical["observedCurrentHead"], next.latest_revision_id);
    assert_eq!(historical["compilation"], reviewed["compilation"]);
    assert_eq!(
        historical["candidateManifestJson"],
        reviewed["candidateManifestJson"]
    );
    assert_eq!(dir.bytes(), before);
}

#[test]
fn blocked_inspection_has_no_manifest_and_missing_selection_creates_nothing() {
    let dir = Fixture::new();
    let store = dir.store();
    let missing = Request {
        schema_version: REQUEST_SCHEMA.into(),
        capability_id: format!("capability:{}", "a".repeat(64)),
        revision_id: format!("revision:{}", "b".repeat(64)),
        profile: Profile::AgentsMdV1,
    };
    assert!(matches!(
        inspect(&store, &missing),
        Err(StoreError::CapabilityNotFound)
    ));
    assert!(!dir.0.exists());
    let skill = dir.skill();
    let content = "@local.md <!--keep-->";
    let skill = store
        .revise_capability_v1(&skill.id, &skill.latest_revision_id, "Includes", content)
        .unwrap()
        .capability;
    store
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    let before = dir.bytes();
    let blocked = encoded(&store, &request(&skill, Profile::ClaudeMdV1));
    assert_eq!(blocked["compilation"]["readiness"], "blocked");
    assert_eq!(blocked["compilation"]["artifact"]["content"], content);
    assert!(blocked["compilation"]["candidate"].is_null());
    assert!(blocked["candidateManifestJson"].is_null());
    assert_eq!(dir.bytes(), before);
}

#[test]
fn compile_permission_is_local_main_only_and_errors_are_static() {
    let permissions: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    assert_eq!(permissions["local"], true);
    assert_eq!(permissions["windows"], serde_json::json!(["main"]));
    assert!(permissions.get("remote").is_none());
    assert!(
        permissions["permissions"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("allow-compile-capability"))
    );
    assert!(include_str!("../build.rs").contains("\"compile_capability\""));
    let failure = serde_json::to_value(failure(StoreError::CompilationInvalid)).unwrap();
    assert_eq!(failure["outcome"], "failed");
    assert_eq!(failure["error"]["code"], "compilation_invalid");
    assert!(failure.get("compilation").is_none());
}
