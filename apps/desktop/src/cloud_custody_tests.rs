use super::*;
#[test]
fn submission_is_closed_bounded_and_single_use() {
    let custody = Custody::default();
    let (lease, mut receiver) = custody.begin(Phase::Editing).unwrap();
    assert_eq!(custody.edit_id(), Some(lease.id.clone()));
    let body = serde_json::json!({"schemaVersion":"rangoon.cloud-credential-submit.v1","editId":lease.id,"secret":"synthetic-fixture"});
    let (id, secret) = parse_submission(body.to_string().as_bytes()).unwrap();
    custody.submit(&id, secret).unwrap();
    assert!(receiver.try_recv().unwrap().is_some());
    assert!(custody.edit_id().is_none());
    assert_eq!(
        custody.submit(&id, Secret::new(b"replay").unwrap()),
        Err(Error::InvalidRequest)
    );
    assert!(matches!(custody.begin(Phase::Editing), Err(Error::Busy)));
    drop(lease);
    assert!(custody.begin(Phase::Editing).is_ok());
}
#[test]
fn cancel_and_stale_ids_cannot_authorize_write() {
    let custody = Custody::default();
    let (lease, mut receiver) = custody.begin(Phase::Editing).unwrap();
    assert!(!custody.cancel(Some(&"a".repeat(64))));
    assert_eq!(custody.edit_id(), Some(lease.id.clone()));
    assert!(!custody.cancel(Some(&lease.id)));
    assert!(receiver.try_recv().unwrap().is_none());
    assert!(!custody.advance(&lease.id, Phase::Storage));
    let old = lease.id.clone();
    drop(lease);
    let (new, _) = custody.begin(Phase::Editing).unwrap();
    assert!(!custody.advance(&old, Phase::Storage));
    assert!(!custody.cancel(Some(&old)));
    assert_eq!(custody.edit_id(), Some(new.id.clone()));
}
#[test]
fn prompt_cancellation_preserves_parent_and_storage_is_not_undone() {
    let custody = Custody::default();
    let (lease, _) = custody.begin(Phase::Prompting).unwrap();
    assert!(custody.cancel(None));
    assert!(!custody.advance(&lease.id, Phase::Storage));
    drop(lease);
    let (lease, _) = custody.begin(Phase::Storage).unwrap();
    assert!(custody.cancel(None));
    assert!(custody.advance(&lease.id, Phase::Storage));
}
#[test]
fn hostile_json_and_exact_limits() {
    let schema = "rangoon.cloud-credential-submit.v1";
    let id = "a".repeat(64);
    let payload = |secret: &str| {
        serde_json::json!({"schemaVersion":schema,"editId":id,"secret":secret}).to_string()
    };
    assert!(parse_submission(payload(&"x".repeat(2048)).as_bytes()).is_ok());
    assert!(parse_submission(payload(&"x".repeat(2049)).as_bytes()).is_err());
    assert!(parse_submission(payload("\\\"!").as_bytes()).is_ok());
    for body in [
        payload(" "),
        payload("secret\n"),
        payload("é"),
        payload("").to_string(),
        format!(
            "{{\"schemaVersion\":\"{schema}\",\"editId\":\"{id}\",\"secret\":\"a\",\"secret\":\"b\"}}"
        ),
        format!(
            "{{\"schemaVersion\":\"{schema}\",\"editId\":\"{id}\",\"secret\":\"a\",\"authority\":\"admin\"}}"
        ),
        payload("a") + "null",
        " ".repeat(12545),
        payload("a").replace(&id, "short"),
    ] {
        assert!(matches!(
            parse_submission(body.as_bytes()),
            Err(Error::InvalidRequest)
        ));
    }
    assert!(parse_submission(&[255]).is_err());
    assert!(
        Decision::parse(
            serde_json::json!({"schemaVersion":"rangoon.cloud-credential-decision.v1","editId":id})
                .to_string()
                .as_bytes()
        )
        .is_ok()
    );
    assert!(Decision::parse(b"{}").is_err());
}

#[test]
fn cancel_during_initial_read_cannot_proceed_to_editor_or_mutation() {
    let custody = Custody::default();
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    assert!(custody.cancel(None));
    assert!(!custody.advance(&lease.id, Phase::Editing));
    assert!(!custody.advance(&lease.id, Phase::Prompting));
    assert!(!custody.advance(&lease.id, Phase::Storage));
}
#[test]
fn cancellation_and_storage_claim_are_serialized() {
    use std::sync::Barrier;
    for _ in 0..64 {
        let custody = Custody::default();
        let (lease, _) = custody.begin(Phase::Prompting).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let worker = custody.clone();
        let ready = barrier.clone();
        let id = lease.id.clone();
        let thread = std::thread::spawn(move || {
            ready.wait();
            worker.advance(&id, Phase::Storage)
        });
        barrier.wait();
        custody.cancel(None);
        let claimed = thread.join().unwrap();
        // A winning cancellation can never later become a successful claim.
        assert_eq!(custody.advance(&lease.id, Phase::Storage), claimed);
    }
}
