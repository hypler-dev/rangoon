//! Public synthetic inspection lifecycle. No I/O/provider/runtime.
use rangoon_domain::byte_digest;
use rangoon_integrations::lnsat_request_inspection::{
    CompletionDisposition, InspectionControlError, InspectionState, RequestInspectionHost,
};
use rangoon_integrations::lnsat_request_record::{
    ReadUnavailable, RequestRecordBody, RequestRecordInput,
};
use serde_json::{Value, json};
const R: &str = "arq_0000000000000000000000000000000000000000000000000000000000000001";
const P: &str = "project:synthetic";
const X: &str = "resource:synthetic";
const SECRET: &str = "requester-private-sentinel";
fn id(p: &str, n: u8) -> String {
    format!("{p}{n:064x}")
}
fn dg(n: u8) -> String {
    format!("sha256:{n:064x}")
}
fn at(n: u8) -> String {
    format!("2026-10-26T12:00:{n:02}.000Z")
}
fn cm(d: &str, x: Vec<Value>) -> String {
    let b = serde_json::to_vec(&Value::Array(x)).unwrap();
    let mut m = d.as_bytes().to_vec();
    m.push(b'\n');
    m.extend(b);
    format!("sha256:{}", byte_digest(&m))
}
fn root() -> Value {
    json!({"request_id":R,"requester_ref":SECRET,"project_ref":P,"resource_ref":X,"packet_id":"packet:synthetic","packet_sha256":dg(1),"action_digest":dg(2),"target_digest":dg(3),"configuration_digest":dg(4),"created_at":at(0),"evidence_digest":dg(5)})
}
fn policy(d: &str) -> Value {
    json!({"decision_id":id("pol_",1),"evaluated_at":at(0),"expires_at":at(59),"decision":d,"requires_approval":if d=="approval_required"{1}else{0}})
}
fn evaluation(s: &str, p: Option<String>, r: &str) -> Value {
    let q = root();
    json!({"status":s,"policy_decision_id":p.clone(),"reason_code":r,"recorded_at":at(0),"evidence_digest":cm("lnsat.action_request.evaluation.v1",vec![json!(R),q["packet_id"].clone(),q["packet_sha256"].clone(),json!(P),json!(X),json!(s),p.map(Value::String).unwrap_or(Value::Null),json!(r),json!(at(0))])})
}
fn evidence(d: &str) -> Value {
    json!({"schema":"lnsat.action_request_linked_evidence.v1","policy":policy(d),"approvals":[],"approval_decisions":[],"authorization_attempts":[],"authorizations":[],"operations":[],"consumptions":[],"operation_attempts":[],"receipts":[],"reconciliations":[],"states":[]})
}
fn complete(d: &str) -> Value {
    json!({"schema":"lnsat.action_request_record.v1","status":"complete","request_id":R,"evaluation_state":"policy_recorded","request":root(),"evaluation":evaluation("recorded",Some(id("pol_",1)),"policy_recorded"),"observations":[],"evidence":evidence(d),"security_activity_touched":false})
}
fn failed() -> Value {
    json!({"schema":"lnsat.action_request_record.v1","status":"complete","request_id":R,"evaluation_state":"evaluation_failed","request":root(),"evaluation":evaluation("failed",None,"evaluation_rejected"),"observations":[],"evidence":null,"security_activity_touched":false})
}
fn unknown() -> Value {
    json!({"schema":"lnsat.action_request_record.v1","status":"complete","request_id":R,"evaluation_state":"evaluation_unknown","request":root(),"evaluation":null,"observations":[],"evidence":null,"security_activity_touched":false})
}
fn degraded() -> Value {
    json!({"schema":"lnsat.action_request_record.v1","status":"degraded","request_id":R,"evaluation_state":"evaluation_unknown","request":null,"evaluation":null,"observations":[],"evidence":null,"security_activity_touched":true})
}
fn ctx(b: RequestRecordBody) -> RequestRecordInput {
    RequestRecordInput {
        installation_id: "test".into(),
        environment: "test".into(),
        selected_request_id: R.into(),
        expected_project_ref: P.into(),
        expected_resource_ref: X.into(),
        body: b,
        read_at: Some(at(58)),
    }
}
fn host(v: Value) -> RequestInspectionHost {
    let mut h = RequestInspectionHost::new();
    let w = h
        .begin(ctx(RequestRecordBody::Body200(
            serde_json::to_vec(&v).unwrap(),
        )))
        .unwrap();
    assert_eq!(h.complete(w.decode()), CompletionDisposition::Published);
    h
}
fn view(h: &mut RequestInspectionHost) -> Value {
    let l = h.presentation().unwrap();
    serde_json::from_slice(&l.json_bytes().unwrap()).unwrap()
}
fn assert_summary(p: &Value, a: &str, e: &str, pol: &str, security: Value, counts: Value) {
    assert_eq!(
        p,
        &json!({"schemaVersion":"rangoon.inspection.lnsat-request-summary-view.v1","epoch":1,"state":"summary_ready","error":null,"summary":{"schemaVersion":"rangoon.integrations.lnsat-request-summary.v1","recordAvailability":a,"reportedEvaluation":e,"reportedPolicy":pol,"readFailure":null,"securityActivityTouched":security,"familyCounts":counts,"truth":{"authenticity":"unverified","authority":"none","execution":"unavailable","authoritativeOutcome":"unknown","independentEffect":"not_observed","currentness":"unverified"},"omissions":[{"path":"request.requester_ref","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"},{"path":"evidence.approval_decisions[*].reason","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"}]}})
    );
    let text = serde_json::to_string(p).unwrap();
    for canary in [
        SECRET,
        R,
        P,
        X,
        "packet:synthetic",
        "2026-10-26T",
        "sha256:",
    ] {
        assert!(!text.contains(canary), "leaked {canary}");
    }
}
#[test]
fn i01_allow_full_projection() {
    let mut h = host(complete("allow"));
    assert_summary(
        &view(&mut h),
        "complete",
        "policy_recorded",
        "allow",
        json!(false),
        json!({"approvals":0,"approval_decisions":0,"authorization_attempts":0,"authorizations":0,"operations":0,"consumptions":0,"operation_attempts":0,"receipts":0,"reconciliations":0,"states":0}),
    )
}
#[test]
fn i02_context_mismatch_rejects() {
    for n in 0..3 {
        let mut c = ctx(RequestRecordBody::Body200(
            serde_json::to_vec(&complete("allow")).unwrap(),
        ));
        if n == 0 {
            c.selected_request_id = id("arq_", 9)
        }
        if n == 1 {
            c.expected_project_ref = "project:other".into()
        }
        if n == 2 {
            c.expected_resource_ref = "resource:other".into()
        }
        let mut h = RequestInspectionHost::new();
        let w = h.begin(c).unwrap();
        h.complete(w.decode());
        assert_eq!(view(&mut h)["state"], "rejected")
    }
}
#[test]
fn i04_allow_no_authority() {
    let mut h = host(complete("allow"));
    assert_eq!(view(&mut h)["summary"]["truth"]["authority"], "none")
}
#[test]
fn i05_deny_report() {
    let mut h = host(complete("deny"));
    assert_eq!(view(&mut h)["summary"]["reportedPolicy"], "deny")
}
#[test]
fn i06_approval_report() {
    let mut h = host(complete("approval_required"));
    assert_eq!(
        view(&mut h)["summary"]["reportedPolicy"],
        "approval_required"
    )
}
#[test]
fn i07_failed_unknown() {
    for (v, e) in [
        (failed(), "evaluation_failed"),
        (unknown(), "evaluation_unknown"),
    ] {
        let mut h = host(v);
        assert_summary(
            &view(&mut h),
            "complete",
            e,
            "not_observed",
            json!(false),
            Value::Null,
        )
    }
}
#[test]
fn i08_degraded() {
    let mut h = host(degraded());
    assert_summary(
        &view(&mut h),
        "degraded",
        "evaluation_unknown",
        "not_observed",
        json!(true),
        Value::Null,
    )
}
#[test]
fn i09_unavailable() {
    for (kind, failure) in [
        (ReadUnavailable::NotConfigured, "read_not_configured"),
        (
            ReadUnavailable::TransportUnavailable,
            "read_transport_unavailable",
        ),
        (ReadUnavailable::Cancelled, "read_cancelled"),
        (ReadUnavailable::Timeout, "read_timeout"),
        (ReadUnavailable::Unauthenticated, "read_unauthenticated"),
        (
            ReadUnavailable::UnsupportedTransport,
            "read_unsupported_transport",
        ),
    ] {
        let mut h = RequestInspectionHost::new();
        let w = h
            .begin(ctx(RequestRecordBody::ReadUnavailable(kind)))
            .unwrap();
        h.complete(w.decode());
        let mut p = view(&mut h);
        assert_eq!(p["summary"]["readFailure"], failure);
        p["summary"]["readFailure"] = Value::Null;
        assert_summary(
            &p,
            "read_unavailable",
            "not_observed",
            "not_observed",
            Value::Null,
            Value::Null,
        );
    }
}

#[test]
fn i10_refused_403() {
    let mut h = RequestInspectionHost::new();
    let w=h.begin(ctx(RequestRecordBody::Refused403(br#"{"schema":"lnsat.action_request_record.v1","ok":false,"error":{"code":"gateway.action_request_record.denied"}}"#.to_vec()))).unwrap();
    h.complete(w.decode());
    let mut p = view(&mut h);
    assert_eq!(
        p["summary"]["readFailure"],
        "native_read_rejected_unclassified"
    );
    p["summary"]["readFailure"] = Value::Null;
    assert_summary(
        &p,
        "read_rejected_unclassified",
        "not_observed",
        "not_observed",
        Value::Null,
        Value::Null,
    );
}

#[test]
fn i12_debug_opaque() {
    let h = RequestInspectionHost::new();
    assert!(!format!("{h:?}").contains(SECRET))
}
#[test]
fn i17_i18_busy() {
    let mut h = host(complete("allow"));
    assert!(matches!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        ))),
        Err(InspectionControlError::Busy)
    ));
    let t = h.discard().unwrap();
    h.acknowledge_purge(t).unwrap();
    assert!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .is_ok()
    )
}
#[test]
fn i19_i20_late_dropped() {
    let mut h = RequestInspectionHost::new();
    let d = h
        .begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout,
        )))
        .unwrap()
        .decode();
    let t = h.discard().unwrap();
    assert_eq!(h.complete(d), CompletionDisposition::Dropped);
    h.acknowledge_purge(t).unwrap();
    assert_eq!(h.state(), InspectionState::Discarded)
}
#[test]
fn i22_i23_cross_host_ticket() {
    let mut a = host(complete("allow"));
    let mut b = RequestInspectionHost::new();
    let t = a.discard().unwrap();
    assert!(matches!(
        b.acknowledge_purge(t.clone()),
        Err(InspectionControlError::StaleTicket)
    ));
    a.acknowledge_purge(t).unwrap()
}
#[test]
fn i24_i25_close_sticky() {
    let mut h = host(complete("allow"));
    let t = h.discard().unwrap();
    h.close();
    h.acknowledge_purge(t).unwrap();
    assert_eq!(h.state(), InspectionState::Closed);
    assert_eq!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .unwrap_err(),
        InspectionControlError::Closed
    );
    assert!(h.close().is_none());
    assert!(h.discard().is_none());
    assert!(h.presentation().is_err())
}
#[test]
fn i27_drop_waits_ack() {
    let mut h = RequestInspectionHost::new();
    let w = h
        .begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout,
        )))
        .unwrap();
    let t = h.discard().unwrap();
    drop(w);
    h.refresh_cleanup();
    assert_eq!(h.state(), InspectionState::DiscardPending);
    h.acknowledge_purge(t).unwrap();
    assert_eq!(h.state(), InspectionState::Discarded)
}

#[test]
fn i03_payload_cannot_choose_its_own_expected_scope() {
    let mut v = complete("allow");
    v["request_id"] = json!(id("arq_", 9));
    v["request"]["request_id"] = json!(id("arq_", 9));
    let mut h = host(v);
    let p = view(&mut h);
    assert_eq!(p["state"], "rejected");
    assert_eq!(p["error"]["code"], "selection_mismatch");
    assert!(p["summary"].is_null());
}
#[test]
fn i17_busy_during_active_decode_has_no_prior_payload() {
    let mut h = RequestInspectionHost::new();
    let held = h
        .begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout,
        )))
        .unwrap();
    assert_eq!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Cancelled
        )))
        .unwrap_err(),
        InspectionControlError::Busy
    );
    assert!(h.presentation().is_err());
    drop(held);
    assert_eq!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .unwrap_err(),
        InspectionControlError::Busy
    );
}
#[test]
fn i18_rejected_result_requires_completed_discard() {
    let mut h = host(json!({}));
    assert_eq!(view(&mut h)["state"], "rejected");
    assert_eq!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .unwrap_err(),
        InspectionControlError::Busy
    );
    let t = h.discard().unwrap();
    assert_eq!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .unwrap_err(),
        InspectionControlError::Busy
    );
    h.acknowledge_purge(t).unwrap();
    assert!(
        h.begin(ctx(RequestRecordBody::ReadUnavailable(
            ReadUnavailable::Timeout
        )))
        .is_ok()
    );
}
#[test]
fn i12_all_opaque_objects_keep_sensitive_context_out_of_debug() {
    let mut h = RequestInspectionHost::new();
    let mut i = ctx(RequestRecordBody::ReadUnavailable(ReadUnavailable::Timeout));
    i.installation_id = "PRIVATE_INSTALLATION".into();
    i.environment = "PRIVATE_ENVIRONMENT".into();
    let work = h.begin(i).unwrap();
    let w = format!("{work:?}");
    let done = work.decode();
    let d = format!("{done:?}");
    h.complete(done);
    let v = format!("{:?}", h.presentation().unwrap());
    let t = h.discard().unwrap();
    let text = format!("{h:?} {t:?} {w} {d} {v}");
    for canary in [
        "PRIVATE_INSTALLATION",
        "PRIVATE_ENVIRONMENT",
        R,
        P,
        X,
        SECRET,
        "read_timeout",
    ] {
        assert!(!text.contains(canary));
    }
}
