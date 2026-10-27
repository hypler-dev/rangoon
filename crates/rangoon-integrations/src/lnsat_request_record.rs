//! Bounded offline LNSAT action-request inspection.
#![forbid(unsafe_code)]

use rangoon_domain::byte_digest;

pub const MAX_INPUT_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 16;
const MAX_VALUES: usize = 16_384;
const MAX_OBJECT_FIELDS: usize = 32;
const MAX_ARRAY_ITEMS: usize = 64;
const MAX_OBSERVATIONS: usize = 2;
const MAX_STRING_BYTES: usize = 4_096;
const MAX_HOST_BYTES: usize = 131_072;
const MAX_SUMMARY_BYTES: usize = 8_192;
const WIRE: &str = "lnsat.action_request_record.v1";
const LINKED_WIRE: &str = "lnsat.action_request_linked_evidence.v1";
const SOURCE_COMMIT: &str = "35a41963ad698934c7ee00852c71355e329c34fa";

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ReadUnavailable {
    NotConfigured,
    TransportUnavailable,
    Cancelled,
    Timeout,
    Unauthenticated,
    UnsupportedTransport,
}
pub enum RequestRecordBody {
    Body200(Vec<u8>),
    Refused403(Vec<u8>),
    ReadUnavailable(ReadUnavailable),
}
pub struct RequestRecordInput {
    pub installation_id: String,
    pub environment: String,
    pub selected_request_id: String,
    pub expected_project_ref: String,
    pub expected_resource_ref: String,
    pub body: RequestRecordBody,
    pub read_at: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestRecordError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    DuplicateField,
    StructuralLimit,
    UnsupportedSchema,
    InvalidEnvelope,
    SelectionMismatch,
    ScopeMismatch,
    LineageConflict,
    UnsupportedState,
    OutputLimit,
    SerializationFailed,
}
impl RequestRecordError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputLimit => "input_limit",
            Self::InvalidEncoding => "invalid_encoding",
            Self::InvalidJson => "invalid_json",
            Self::DuplicateField => "duplicate_field",
            Self::StructuralLimit => "structural_limit",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::InvalidEnvelope => "invalid_envelope",
            Self::SelectionMismatch => "selection_mismatch",
            Self::ScopeMismatch => "scope_mismatch",
            Self::LineageConflict => "lineage_conflict",
            Self::UnsupportedState => "unsupported_state",
            Self::OutputLimit => "output_limit",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}
impl RequestRecordError {
    pub fn json_bytes(self) -> Vec<u8> {
        let t = truth();
        encode(&object(vec![("schemaVersion",s("rangoon.integrations.lnsat-request-error.v1")),("code",s(self.code())),("truth",t)])).unwrap_or_else(|_|b"{\"schemaVersion\":\"rangoon.integrations.lnsat-request-error.v1\",\"code\":\"serialization_failed\",\"truth\":{\"authenticity\":\"unverified\",\"authority\":\"none\",\"execution\":\"unavailable\",\"authoritativeOutcome\":\"unknown\",\"independentEffect\":\"not_observed\",\"currentness\":\"unverified\"}}".to_vec())
    }
}
impl core::fmt::Display for RequestRecordError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for RequestRecordError {}
impl core::fmt::Debug for RequestRecordInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("RequestRecordInput { payload omitted }")
    }
}
impl core::fmt::Debug for RequestRecordBody {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("RequestRecordBody { payload omitted }")
    }
}
pub struct RequestRecordInspection {
    host_json: Vec<u8>,
    summary_json: Vec<u8>,
    input_digest: Option<String>,
}
impl RequestRecordInspection {
    pub fn host_json(&self) -> &[u8] {
        &self.host_json
    }
    pub fn summary_json(&self) -> &[u8] {
        &self.summary_json
    }
    pub fn input_digest(&self) -> Option<&str> {
        self.input_digest.as_deref()
    }
}
impl core::fmt::Debug for RequestRecordInspection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("RequestRecordInspection { redacted: true }")
    }
}

pub fn inspect_request_record(
    input: RequestRecordInput,
) -> Result<RequestRecordInspection, RequestRecordError> {
    context(&input)?;
    match &input.body {
        RequestRecordBody::ReadUnavailable(r) => unavailable(&input, *r),
        RequestRecordBody::Refused403(b) => refusal(&input, b),
        RequestRecordBody::Body200(b) => body(&input, b),
    }
}

fn context(i: &RequestRecordInput) -> Result<(), RequestRecordError> {
    for x in [&i.installation_id, &i.environment] {
        text(x, 128)?;
    }
    for x in [&i.expected_project_ref, &i.expected_resource_ref] {
        text(x, 512)?;
    }
    id(&i.selected_request_id, "arq_")?;
    if let Some(t) = &i.read_at {
        time(t)?;
    }
    Ok(())
}
fn unavailable(
    i: &RequestRecordInput,
    r: ReadUnavailable,
) -> Result<RequestRecordInspection, RequestRecordError> {
    let f = match r {
        ReadUnavailable::NotConfigured => "read_not_configured",
        ReadUnavailable::TransportUnavailable => "read_transport_unavailable",
        ReadUnavailable::Cancelled => "read_cancelled",
        ReadUnavailable::Timeout => "read_timeout",
        ReadUnavailable::Unauthenticated => "read_unauthenticated",
        ReadUnavailable::UnsupportedTransport => "read_unsupported_transport",
    };
    emit(
        i,
        ("read_unavailable", "not_observed", "not_observed"),
        J::Null,
        J::Null,
        None,
        Some(f),
        None,
    )
}
fn refusal(
    i: &RequestRecordInput,
    b: &[u8],
) -> Result<RequestRecordInspection, RequestRecordError> {
    let v = parse(b)?;
    let o = obj(&v)?;
    keys(o, &["schema", "ok", "error"])?;
    eq(
        get(o, "schema")?,
        WIRE,
        RequestRecordError::UnsupportedSchema,
    )?;
    if boolean(get(o, "ok")?)? {
        return Err(RequestRecordError::InvalidEnvelope);
    }
    let e = obj(get(o, "error")?)?;
    keys(e, &["code"])?;
    eq(
        get(e, "code")?,
        "gateway.action_request_record.denied",
        RequestRecordError::InvalidEnvelope,
    )?;
    emit(
        i,
        ("read_rejected_unclassified", "not_observed", "not_observed"),
        J::Null,
        J::Null,
        Some(hash(b)),
        Some("native_read_rejected_unclassified"),
        None,
    )
}
fn body(i: &RequestRecordInput, b: &[u8]) -> Result<RequestRecordInspection, RequestRecordError> {
    let v = parse(b)?;
    let o = obj(&v)?;
    keys(
        o,
        &[
            "schema",
            "status",
            "request_id",
            "evaluation_state",
            "request",
            "evaluation",
            "observations",
            "evidence",
            "security_activity_touched",
        ],
    )?;
    eq(
        get(o, "schema")?,
        WIRE,
        RequestRecordError::UnsupportedSchema,
    )?;
    let request_id = string(get(o, "request_id")?)?;
    id(request_id, "arq_")?;
    if request_id != i.selected_request_id {
        return Err(RequestRecordError::SelectionMismatch);
    }
    let security = boolean(get(o, "security_activity_touched")?)?;
    let observations = observations(get(o, "observations")?, get(o, "request")?)?;
    let digest = Some(hash(b));
    match string(get(o, "status")?)? {
        "degraded" => {
            eq(
                get(o, "evaluation_state")?,
                "evaluation_unknown",
                RequestRecordError::InvalidEnvelope,
            )?;
            null(get(o, "request")?)?;
            null(get(o, "evaluation")?)?;
            null(get(o, "evidence")?)?;
            if !observations.is_empty() {
                return Err(RequestRecordError::InvalidEnvelope);
            }
            emit(
                i,
                ("degraded", "evaluation_unknown", "not_observed"),
                J::Null,
                J::Bool(security),
                digest,
                None,
                None,
            )
        }
        "complete" => complete(i, o, observations, security, digest),
        _ => Err(RequestRecordError::UnsupportedState),
    }
}
fn complete(
    i: &RequestRecordInput,
    o: &[(String, J)],
    obs: Vec<J>,
    security: bool,
    digest: Option<String>,
) -> Result<RequestRecordInspection, RequestRecordError> {
    let request = request(obj(get(o, "request")?)?, i)?;
    let eval = get(o, "evaluation")?;
    let evidence_value = get(o, "evidence")?;
    let (re, policy, ev_out, evidence_out, counts) = match eval {
        J::Null => {
            eq(
                get(o, "evaluation_state")?,
                "evaluation_unknown",
                RequestRecordError::InvalidEnvelope,
            )?;
            null(evidence_value)?;
            ("evaluation_unknown", "not_observed", J::Null, J::Null, None)
        }
        J::Obj(e) => {
            evaluation(e, &request)?;
            match string(get(e, "status")?)? {
                "failed" => {
                    eq(
                        get(o, "evaluation_state")?,
                        "evaluation_failed",
                        RequestRecordError::InvalidEnvelope,
                    )?;
                    null(get(e, "policy_decision_id")?)?;
                    let r = string(get(e, "reason_code")?)?;
                    if !matches!(r, "evaluation_rejected" | "persistence_failed") {
                        return Err(RequestRecordError::UnsupportedState);
                    }
                    null(evidence_value)?;
                    commitments(&request, e, &obs)?;
                    (
                        "evaluation_failed",
                        "not_observed",
                        J::Obj(canonical(
                            e,
                            &[
                                "status",
                                "policy_decision_id",
                                "reason_code",
                                "recorded_at",
                                "evidence_digest",
                            ],
                        )?),
                        J::Null,
                        None,
                    )
                }
                "recorded" => {
                    eq(
                        get(o, "evaluation_state")?,
                        "policy_recorded",
                        RequestRecordError::InvalidEnvelope,
                    )?;
                    id(string(get(e, "policy_decision_id")?)?, "pol_")?;
                    eq(
                        get(e, "reason_code")?,
                        "policy_recorded",
                        RequestRecordError::InvalidEnvelope,
                    )?;
                    let (x, p, c) = evidence(evidence_value, &request, e, &obs)?;
                    (
                        "policy_recorded",
                        p,
                        J::Obj(canonical(
                            e,
                            &[
                                "status",
                                "policy_decision_id",
                                "reason_code",
                                "recorded_at",
                                "evidence_digest",
                            ],
                        )?),
                        J::Obj(x),
                        Some(c),
                    )
                }
                _ => return Err(RequestRecordError::UnsupportedState),
            }
        }
        _ => return Err(RequestRecordError::InvalidEnvelope),
    };
    emit(
        i,
        ("complete", re, policy),
        object(vec![
            ("request", J::Obj(request)),
            ("evaluation", ev_out),
            ("observations", J::Arr(obs)),
            ("evidence", evidence_out),
        ]),
        J::Bool(security),
        digest,
        None,
        counts,
    )
}
fn request(
    o: &[(String, J)],
    i: &RequestRecordInput,
) -> Result<Vec<(String, J)>, RequestRecordError> {
    keys(
        o,
        &[
            "request_id",
            "requester_ref",
            "project_ref",
            "resource_ref",
            "packet_id",
            "packet_sha256",
            "action_digest",
            "target_digest",
            "configuration_digest",
            "created_at",
            "evidence_digest",
        ],
    )?;
    id(string(get(o, "request_id")?)?, "arq_")?;
    if string(get(o, "request_id")?)? != i.selected_request_id {
        return Err(RequestRecordError::SelectionMismatch);
    }
    if string(get(o, "project_ref")?)? != i.expected_project_ref
        || string(get(o, "resource_ref")?)? != i.expected_resource_ref
    {
        return Err(RequestRecordError::ScopeMismatch);
    }
    for n in ["requester_ref", "project_ref", "resource_ref", "packet_id"] {
        text(string(get(o, n)?)?, MAX_STRING_BYTES)?
    }
    for n in [
        "packet_sha256",
        "action_digest",
        "target_digest",
        "configuration_digest",
        "evidence_digest",
    ] {
        digest(string(get(o, n)?)?)?
    }
    time(string(get(o, "created_at")?)?)?;
    canonical(
        o,
        &[
            "request_id",
            "project_ref",
            "resource_ref",
            "packet_id",
            "packet_sha256",
            "action_digest",
            "target_digest",
            "configuration_digest",
            "created_at",
            "evidence_digest",
        ],
    )
}
fn evaluation(o: &[(String, J)], r: &[(String, J)]) -> Result<(), RequestRecordError> {
    keys(
        o,
        &[
            "status",
            "policy_decision_id",
            "reason_code",
            "recorded_at",
            "evidence_digest",
        ],
    )?;
    time(string(get(o, "recorded_at")?)?)?;
    if string(get(o, "recorded_at")?)? != string(get(r, "created_at")?)? {
        return Err(RequestRecordError::LineageConflict);
    }
    digest(string(get(o, "evidence_digest")?)?)?;
    Ok(())
}

fn observations(v: &J, r: &J) -> Result<Vec<J>, RequestRecordError> {
    let a = array(v)?;
    if a.len() > MAX_OBSERVATIONS {
        return Err(RequestRecordError::StructuralLimit);
    };
    let root = match r {
        J::Obj(x) => Some(x),
        J::Null => None,
        _ => return Err(RequestRecordError::InvalidEnvelope),
    };
    let mut replay = false;
    let mut conflict = false;
    let mut out = Vec::new();
    for row in a {
        let o = obj(row)?;
        keys(
            o,
            &["kind", "submitted_digest", "observed_at", "evidence_digest"],
        )?;
        let kind = string(get(o, "kind")?)?;
        digest(string(get(o, "submitted_digest")?)?)?;
        digest(string(get(o, "evidence_digest")?)?)?;
        time(string(get(o, "observed_at")?)?)?;
        if let Some(root) = root {
            if string(get(o, "observed_at")?)? < string(get(root, "created_at")?)? {
                return Err(RequestRecordError::LineageConflict);
            }
            let p = string(get(root, "packet_sha256")?)?;
            match kind {
                "replay" if !replay => {
                    replay = true;
                    if string(get(o, "submitted_digest")?)? != p {
                        return Err(RequestRecordError::LineageConflict);
                    }
                }
                "conflict" if !conflict => {
                    conflict = true;
                    if string(get(o, "submitted_digest")?)? == p {
                        return Err(RequestRecordError::LineageConflict);
                    }
                }
                "replay" | "conflict" => return Err(RequestRecordError::LineageConflict),
                _ => return Err(RequestRecordError::UnsupportedState),
            }
        } else if !matches!(kind, "replay" | "conflict") {
            return Err(RequestRecordError::UnsupportedState);
        }
        if let Some(root) = root {
            let encoded = compact(&[
                get(root, "request_id")?,
                get(o, "kind")?,
                get(o, "submitted_digest")?,
                get(o, "observed_at")?,
            ])?;
            if hash(format!("lnsat.action_request.observation.v1\n{encoded}").as_bytes())
                != string(get(o, "evidence_digest")?)?
            {
                return Err(RequestRecordError::LineageConflict);
            }
        }
        out.push(J::Obj(canonical(
            o,
            &["kind", "submitted_digest", "observed_at", "evidence_digest"],
        )?))
    }
    Ok(out)
}
const FAMILIES: &[&str] = &[
    "approvals",
    "approval_decisions",
    "authorization_attempts",
    "authorizations",
    "operations",
    "consumptions",
    "operation_attempts",
    "receipts",
    "reconciliations",
    "states",
];
type NativeObject = Vec<(String, J)>;
type EvidenceProjection = (NativeObject, &'static str, NativeObject);

fn evidence(
    v: &J,
    r: &[(String, J)],
    e: &[(String, J)],
    obs: &[J],
) -> Result<EvidenceProjection, RequestRecordError> {
    let o = obj(v)?;
    keys(
        o,
        &[
            "schema",
            "policy",
            "approvals",
            "approval_decisions",
            "authorization_attempts",
            "authorizations",
            "operations",
            "consumptions",
            "operation_attempts",
            "receipts",
            "reconciliations",
            "states",
        ],
    )?;
    eq(
        get(o, "schema")?,
        LINKED_WIRE,
        RequestRecordError::UnsupportedSchema,
    )?;
    let p = policy(obj(get(o, "policy")?)?, r, e)?;
    let decision = string(get(&p, "decision")?)?;
    let requires = integer(get(&p, "requires_approval")?)?;
    let valid = (decision == "approval_required" && requires == 1)
        || (matches!(decision, "allow" | "deny") && requires == 0);
    if !valid {
        return Err(RequestRecordError::LineageConflict);
    };
    let mut out = vec![
        ("schema".into(), s(LINKED_WIRE)),
        ("policy".into(), J::Obj(p.clone())),
    ];
    let mut counts = Vec::new();
    // Reserve a conservative escaped request/context/source/omission envelope
    // before retaining projected variable rows. This is a byte quota, not an
    // allocator, elapsed-time, or hostile-caller containment guarantee.
    let mut remaining_rows = MAX_HOST_BYTES - 49_152;
    for f in FAMILIES {
        let rows = array(get(o, f)?)?;
        family(f, rows, &p)?;
        let order = spec(f).ok_or(RequestRecordError::InvalidEnvelope)?.0;
        let projected = rows
            .iter()
            .map(|x| {
                let q = obj(x)?;
                let q = if *f == "approval_decisions" {
                    redact(q, &["reason"])
                } else {
                    canonical(q, order)?
                };
                if *f == "approval_decisions" {
                    let redacted_order = &[
                        "approval_decision_id",
                        "approval_request_id",
                        "policy_decision_id",
                        "decision",
                        "decided_at",
                        "expires_at",
                        "approval_gate_satisfied",
                    ];
                    let q = canonical(&q, redacted_order)?;
                    remaining_rows = remaining_rows
                        .checked_sub(encode(&J::Obj(q.clone()))?.len() + 1)
                        .ok_or(RequestRecordError::OutputLimit)?;
                    return Ok(q);
                }
                remaining_rows = remaining_rows
                    .checked_sub(encode(&J::Obj(q.clone()))?.len() + 1)
                    .ok_or(RequestRecordError::OutputLimit)?;
                Ok(q)
            })
            .collect::<Result<Vec<_>, RequestRecordError>>()?
            .into_iter()
            .map(J::Obj)
            .collect();
        out.push(((*f).into(), J::Arr(projected)));
        counts.push(((*f).into(), J::Num(rows.len().to_string())))
    }
    graph(o, &p)?;
    state_rules(o)?;
    commitments(r, e, obs)?;
    Ok((
        out,
        match decision {
            "allow" => "allow",
            "deny" => "deny",
            "approval_required" => "approval_required",
            _ => return Err(RequestRecordError::UnsupportedState),
        },
        counts,
    ))
}
fn policy(
    o: &[(String, J)],
    r: &[(String, J)],
    e: &[(String, J)],
) -> Result<Vec<(String, J)>, RequestRecordError> {
    keys(
        o,
        &[
            "decision_id",
            "evaluated_at",
            "expires_at",
            "decision",
            "requires_approval",
        ],
    )?;
    id(string(get(o, "decision_id")?)?, "pol_")?;
    if string(get(o, "decision_id")?)? != string(get(e, "policy_decision_id")?)? {
        return Err(RequestRecordError::LineageConflict);
    }
    for n in ["evaluated_at", "expires_at"] {
        time(string(get(o, n)?)?)?
    }
    if string(get(o, "evaluated_at")?)? != string(get(r, "created_at")?)?
        || string(get(o, "expires_at")?)? < string(get(o, "evaluated_at")?)?
    {
        return Err(RequestRecordError::LineageConflict);
    }
    canonical(
        o,
        &[
            "decision_id",
            "evaluated_at",
            "expires_at",
            "decision",
            "requires_approval",
        ],
    )
}
fn family(n: &str, a: &[J], p: &[(String, J)]) -> Result<(), RequestRecordError> {
    if a.len() > MAX_ARRAY_ITEMS {
        return Err(RequestRecordError::StructuralLimit);
    }
    let (k, idn, pfx) = spec(n).ok_or(RequestRecordError::InvalidEnvelope)?;
    let mut seen = Vec::new();
    for x in a {
        let o = obj(x)?;
        keys(o, k)?;
        let ident = string(get(o, idn)?)?;
        id(ident, pfx)?;
        if seen.contains(&ident) {
            return Err(RequestRecordError::LineageConflict);
        }
        seen.push(ident);
        row(n, o, p)?
    }
    Ok(())
}
fn spec(n: &str) -> Option<(&'static [&'static str], &'static str, &'static str)> {
    Some(match n {
        "approvals" => (
            &[
                "approval_request_id",
                "policy_decision_id",
                "status",
                "requested_at",
                "expires_at",
            ],
            "approval_request_id",
            "apr_",
        ),
        "approval_decisions" => (
            &[
                "approval_decision_id",
                "approval_request_id",
                "policy_decision_id",
                "decision",
                "reason",
                "decided_at",
                "expires_at",
                "approval_gate_satisfied",
            ],
            "approval_decision_id",
            "apd_",
        ),
        "authorization_attempts" => (
            &[
                "authorization_attempt_id",
                "approval_decision_id",
                "approval_request_id",
                "policy_decision_id",
                "result_status",
                "requested_at",
                "expires_at",
            ],
            "authorization_attempt_id",
            "aat_",
        ),
        "authorizations" => (
            &[
                "authorization_id",
                "authorization_attempt_id",
                "approval_decision_id",
                "policy_decision_id",
                "issued_at",
                "expires_at",
            ],
            "authorization_id",
            "xau_",
        ),
        "operations" => (
            &[
                "operation_id",
                "authorization_id",
                "created_at",
                "expires_at",
            ],
            "operation_id",
            "opn_",
        ),
        "consumptions" => (
            &[
                "consumption_id",
                "authorization_id",
                "operation_id",
                "consumed_at",
            ],
            "consumption_id",
            "cpc_",
        ),
        "operation_attempts" => (
            &[
                "operation_attempt_id",
                "operation_id",
                "attempt_sequence",
                "created_at",
            ],
            "operation_attempt_id",
            "opa_",
        ),
        "receipts" => (
            &[
                "receipt_id",
                "operation_id",
                "operation_attempt_id",
                "authorization_id",
                "consumption_id",
                "verification_status",
                "result_digest",
                "received_at",
            ],
            "receipt_id",
            "rcp_",
        ),
        "reconciliations" => (
            &[
                "reconciliation_id",
                "operation_id",
                "operation_attempt_id",
                "receipt_id",
                "status",
                "observed_result_digest",
                "recorded_at",
            ],
            "reconciliation_id",
            "rec_",
        ),
        "states" => (
            &[
                "state_event_id",
                "target_entity_id",
                "target_entity_kind",
                "state_sequence",
                "state",
                "prior_state_event_id",
                "prior_state_sequence",
                "effective_at",
                "state_digest",
            ],
            "state_event_id",
            "ste_",
        ),
        _ => return None,
    })
}
fn row(n: &str, o: &[(String, J)], p: &[(String, J)]) -> Result<(), RequestRecordError> {
    let pairs: &[(&str, &str)] = match n {
        "approvals" => &[("policy_decision_id", "pol_")],
        "approval_decisions" => &[
            ("approval_request_id", "apr_"),
            ("policy_decision_id", "pol_"),
        ],
        "authorization_attempts" => &[
            ("approval_decision_id", "apd_"),
            ("approval_request_id", "apr_"),
            ("policy_decision_id", "pol_"),
        ],
        "authorizations" => &[
            ("authorization_attempt_id", "aat_"),
            ("approval_decision_id", "apd_"),
            ("policy_decision_id", "pol_"),
        ],
        "operations" => &[("authorization_id", "xau_")],
        "consumptions" => &[("authorization_id", "xau_"), ("operation_id", "opn_")],
        "operation_attempts" => &[("operation_id", "opn_")],
        "receipts" => &[
            ("operation_id", "opn_"),
            ("operation_attempt_id", "opa_"),
            ("authorization_id", "xau_"),
            ("consumption_id", "cpc_"),
        ],
        "reconciliations" => &[("operation_id", "opn_"), ("operation_attempt_id", "opa_")],
        "states" => &[],
        _ => return Err(RequestRecordError::InvalidEnvelope),
    };
    for (f, x) in pairs {
        id(string(get(o, f)?)?, x)?
    }
    if matches!(
        n,
        "approvals" | "approval_decisions" | "authorization_attempts" | "authorizations"
    ) && string(get(o, "policy_decision_id")?)? != string(get(p, "decision_id")?)?
    {
        return Err(RequestRecordError::LineageConflict);
    }
    for (k, v) in o {
        if k.ends_with("_at") {
            time(string(v)?)?
        }
        if k.ends_with("_digest")
            && !(n == "reconciliations" && k == "observed_result_digest" && matches!(v, J::Null))
        {
            digest(string(v)?)?
        }
    }
    match n {
        "approvals" => eq(
            get(o, "status")?,
            "requested",
            RequestRecordError::UnsupportedState,
        )?,
        "approval_decisions" => {
            let d = string(get(o, "decision")?)?;
            let r = string(get(o, "reason")?)?;
            let g = integer(get(o, "approval_gate_satisfied")?)?;
            if !matches!(
                r,
                "approval.operator_approved"
                    | "approval.operator_denied"
                    | "approval.scope_rejected"
                    | "approval.evidence_insufficient"
                    | "approval.request_superseded"
            ) || !((d == "approved" && r == "approval.operator_approved" && g == 1)
                || (d == "denied" && r != "approval.operator_approved" && g == 0))
            {
                return Err(RequestRecordError::UnsupportedState);
            }
        }
        "authorization_attempts"
            if !matches!(
                string(get(o, "result_status")?)?,
                "persistence_prepared" | "rejected" | "failed"
            ) =>
        {
            return Err(RequestRecordError::UnsupportedState);
        }
        "operation_attempts" => {
            let q = integer(get(o, "attempt_sequence")?)?;
            if !(1..=64).contains(&q) {
                return Err(RequestRecordError::StructuralLimit);
            }
        }
        "receipts" => eq(
            get(o, "verification_status")?,
            "accepted",
            RequestRecordError::UnsupportedState,
        )?,
        "reconciliations" => {
            let s = string(get(o, "status")?)?;
            match s {
                "pending" | "unresolved" => {
                    null(get(o, "receipt_id")?)?;
                    null(get(o, "observed_result_digest")?)?
                }
                "matched" | "mismatch" => {
                    id(string(get(o, "receipt_id")?)?, "rcp_")?;
                    digest(string(get(o, "observed_result_digest")?)?)?
                }
                _ => return Err(RequestRecordError::UnsupportedState),
            }
        }
        "states" => state(o)?,
        _ => {}
    }
    Ok(())
}
fn state(o: &[(String, J)]) -> Result<(), RequestRecordError> {
    let kind = string(get(o, "target_entity_kind")?)?;
    let t = string(get(o, "target_entity_id")?)?;
    match kind {
        "execution_authorization" => id(t, "xau_")?,
        "operation" => id(t, "opn_")?,
        "operation_attempt" => id(t, "opa_")?,
        _ => return Err(RequestRecordError::UnsupportedState),
    }
    let n = integer(get(o, "state_sequence")?)?;
    if !(1..=64).contains(&n) {
        return Err(RequestRecordError::StructuralLimit);
    }
    if n == 1 {
        null(get(o, "prior_state_event_id")?)?;
        null(get(o, "prior_state_sequence")?)?
    } else {
        id(string(get(o, "prior_state_event_id")?)?, "ste_")?;
        if integer(get(o, "prior_state_sequence")?)? != n - 1 {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    let state = string(get(o, "state")?)?;
    let ok = match kind {
        "execution_authorization" => matches!(
            state,
            "active" | "cancelled" | "revoked" | "expired" | "consumed"
        ),
        "operation" => matches!(
            state,
            "prepared" | "dispatching" | "outcome_unknown" | "completed" | "failed"
        ),
        _ => matches!(
            state,
            "dispatching" | "outcome_unknown" | "completed" | "failed"
        ),
    };
    if !ok {
        return Err(RequestRecordError::UnsupportedState);
    }
    time(string(get(o, "effective_at")?)?)?;
    digest(string(get(o, "state_digest")?)?)?;
    Ok(())
}
fn graph(o: &[(String, J)], _p: &[(String, J)]) -> Result<(), RequestRecordError> {
    let approvals = objects(array(get(o, "approvals")?)?)?;
    let decisions = objects(array(get(o, "approval_decisions")?)?)?;
    let attempts = objects(array(get(o, "authorization_attempts")?)?)?;
    let auths = objects(array(get(o, "authorizations")?)?)?;
    let ops = objects(array(get(o, "operations")?)?)?;
    let cons = objects(array(get(o, "consumptions")?)?)?;
    let opas = objects(array(get(o, "operation_attempts")?)?)?;
    let receipts = objects(array(get(o, "receipts")?)?)?;
    let recs = objects(array(get(o, "reconciliations")?)?)?;
    let states = objects(array(get(o, "states")?)?)?;
    for d in &decisions {
        require(
            &approvals,
            "approval_request_id",
            string(get(d, "approval_request_id")?)?,
        )?;
    }
    for a in &attempts {
        let d = require(
            &decisions,
            "approval_decision_id",
            string(get(a, "approval_decision_id")?)?,
        )?;
        require(
            &approvals,
            "approval_request_id",
            string(get(a, "approval_request_id")?)?,
        )?;
        if string(get(d, "approval_request_id")?)? != string(get(a, "approval_request_id")?)?
            || string(get(d, "decision")?)? == "denied"
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for a in &auths {
        let q = require(
            &attempts,
            "authorization_attempt_id",
            string(get(a, "authorization_attempt_id")?)?,
        )?;
        if string(get(q, "result_status")?)? != "persistence_prepared"
            || string(get(q, "approval_decision_id")?)? != string(get(a, "approval_decision_id")?)?
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for x in &ops {
        require(
            &auths,
            "authorization_id",
            string(get(x, "authorization_id")?)?,
        )?;
    }
    for x in &cons {
        let au = string(get(x, "authorization_id")?)?;
        let op = string(get(x, "operation_id")?)?;
        let owner = require(&ops, "operation_id", op)?;
        if string(get(owner, "authorization_id")?)? != au
            || count(&cons, "authorization_id", au) > 1
            || count(&cons, "operation_id", op) > 1
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for x in &opas {
        require(&ops, "operation_id", string(get(x, "operation_id")?)?)?;
    }
    for x in &receipts {
        let op = string(get(x, "operation_id")?)?;
        let at = string(get(x, "operation_attempt_id")?)?;
        let operation = require(&ops, "operation_id", op)?;
        let attempt = require(&opas, "operation_attempt_id", at)?;
        if string(get(operation, "authorization_id")?)? != string(get(x, "authorization_id")?)?
            || string(get(attempt, "operation_id")?)? != op
        {
            return Err(RequestRecordError::LineageConflict);
        }
        if count(&receipts, "operation_id", op) > 1
            || count(&receipts, "operation_attempt_id", at) > 1
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for x in &recs {
        let op = string(get(x, "operation_id")?)?;
        require(&ops, "operation_id", op)?;
        let attempt = require(
            &opas,
            "operation_attempt_id",
            string(get(x, "operation_attempt_id")?)?,
        )?;
        if string(get(attempt, "operation_id")?)? != op {
            return Err(RequestRecordError::LineageConflict);
        }
        if let J::Str(rcp) = get(x, "receipt_id")? {
            let z = require(&receipts, "receipt_id", rcp)?;
            if string(get(z, "operation_id")?)? != string(get(x, "operation_id")?)?
                || string(get(z, "operation_attempt_id")?)?
                    != string(get(x, "operation_attempt_id")?)?
            {
                return Err(RequestRecordError::LineageConflict);
            }
            let same =
                string(get(z, "result_digest")?)? == string(get(x, "observed_result_digest")?)?;
            if (string(get(x, "status")?)? == "matched") != same {
                return Err(RequestRecordError::LineageConflict);
            }
        }
    }
    for x in &opas {
        let op = string(get(x, "operation_id")?)?;
        let n = integer(get(x, "attempt_sequence")?)?;
        if count_le(&opas, "operation_id", op, "attempt_sequence", n) != n as usize {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for x in &states {
        let kind = string(get(x, "target_entity_kind")?)?;
        let idv = string(get(x, "target_entity_id")?)?;
        let present = match kind {
            "execution_authorization" => has(&auths, "authorization_id", idv),
            "operation" => has(&ops, "operation_id", idv),
            "operation_attempt" => has(&opas, "operation_attempt_id", idv),
            _ => false,
        };
        if !present {
            return Err(RequestRecordError::LineageConflict);
        }
        if integer(get(x, "state_sequence")?)? > 1 {
            let prev = string(get(x, "prior_state_event_id")?)?;
            if !states.iter().any(|q| {
                string(get(q, "state_event_id").unwrap()).unwrap() == prev
                    && string(get(q, "target_entity_id").unwrap()).unwrap() == idv
                    && integer(get(q, "state_sequence").unwrap()).unwrap() + 1
                        == integer(get(x, "state_sequence").unwrap()).unwrap()
            }) {
                return Err(RequestRecordError::LineageConflict);
            }
        }
    }
    Ok(())
}
fn require<'a>(
    a: &'a Vec<&'a [(String, J)]>,
    k: &str,
    v: &str,
) -> Result<&'a [(String, J)], RequestRecordError> {
    a.iter()
        .copied()
        .find(|x| string(get(x, k).unwrap()).ok() == Some(v))
        .ok_or(RequestRecordError::LineageConflict)
}
fn has(a: &Vec<&[(String, J)]>, k: &str, v: &str) -> bool {
    require(a, k, v).is_ok()
}
fn count(a: &Vec<&[(String, J)]>, k: &str, v: &str) -> usize {
    a.iter()
        .filter(|x| string(get(x, k).unwrap()).unwrap() == v)
        .count()
}
fn count_le(a: &Vec<&[(String, J)]>, g: &str, v: &str, nm: &str, n: i64) -> usize {
    a.iter()
        .filter(|x| {
            string(get(x, g).unwrap()).unwrap() == v && integer(get(x, nm).unwrap()).unwrap() <= n
        })
        .count()
}
fn state_rules(o: &[(String, J)]) -> Result<(), RequestRecordError> {
    let states = objects(array(get(o, "states")?)?)?;
    let auths = objects(array(get(o, "authorizations")?)?)?;
    let ops = objects(array(get(o, "operations")?)?)?;
    let attempts = objects(array(get(o, "operation_attempts")?)?)?;
    let cons = objects(array(get(o, "consumptions")?)?)?;
    let receipts = objects(array(get(o, "receipts")?)?)?;
    for state in &states {
        let kind = string(get(state, "target_entity_kind")?)?;
        let target = string(get(state, "target_entity_id")?)?;
        let seq = integer(get(state, "state_sequence")?)?;
        let at = string(get(state, "effective_at")?)?;
        let same: Vec<_> = states
            .iter()
            .filter(|x| {
                string(get(x, "target_entity_kind").unwrap()).unwrap() == kind
                    && string(get(x, "target_entity_id").unwrap()).unwrap() == target
            })
            .copied()
            .collect();
        if same.iter().enumerate().any(|(index, row)| {
            integer(get(row, "state_sequence").unwrap()).unwrap() != index as i64 + 1
        }) {
            return Err(RequestRecordError::LineageConflict);
        }
        if same.len() > 64
            || same
                .iter()
                .filter(|x| integer(get(x, "state_sequence").unwrap()).unwrap() == seq)
                .count()
                != 1
        {
            return Err(RequestRecordError::LineageConflict);
        }
        if seq == 1 {
            match kind {
                "execution_authorization" => {
                    let a = require(&auths, "authorization_id", target)?;
                    if string(get(a, "issued_at")?)? != at
                        || string(get(state, "state")?)? != "active"
                    {
                        return Err(RequestRecordError::LineageConflict);
                    }
                }
                "operation" => {
                    let x = require(&ops, "operation_id", target)?;
                    if string(get(x, "created_at")?)? != at
                        || string(get(state, "state")?)? != "prepared"
                    {
                        return Err(RequestRecordError::LineageConflict);
                    }
                }
                "operation_attempt" => {
                    let x = require(&attempts, "operation_attempt_id", target)?;
                    if string(get(x, "created_at")?)? != at
                        || string(get(state, "state")?)? != "dispatching"
                    {
                        return Err(RequestRecordError::LineageConflict);
                    }
                }
                _ => return Err(RequestRecordError::LineageConflict),
            }
        } else {
            let prev = require(
                &same,
                "state_event_id",
                string(get(state, "prior_state_event_id")?)?,
            )?;
            if integer(get(prev, "state_sequence")?)? != seq - 1
                || string(get(prev, "effective_at")?)? > at
            {
                return Err(RequestRecordError::LineageConflict);
            }
            let from = string(get(prev, "state")?)?;
            let to = string(get(state, "state")?)?;
            let good = match kind {
                "operation" => matches!(
                    (from, to),
                    ("prepared", "dispatching")
                        | ("dispatching", "outcome_unknown")
                        | ("dispatching", "completed")
                        | ("dispatching", "failed")
                        | ("outcome_unknown", "completed")
                ),
                "operation_attempt" => matches!(
                    (from, to),
                    ("dispatching", "outcome_unknown")
                        | ("dispatching", "completed")
                        | ("dispatching", "failed")
                        | ("outcome_unknown", "completed")
                ),
                "execution_authorization" => matches!(
                    (from, to),
                    ("active", "cancelled")
                        | ("active", "revoked")
                        | ("active", "expired")
                        | ("active", "consumed")
                ),
                _ => false,
            };
            if !good {
                return Err(RequestRecordError::LineageConflict);
            }
        }
    }
    for a in &auths {
        let idv = string(get(a, "authorization_id")?)?;
        let issued = string(get(a, "issued_at")?)?;
        let expires = string(get(a, "expires_at")?)?;
        if expires < issued {
            return Err(RequestRecordError::LineageConflict);
        }
        let ss: Vec<_> = states
            .iter()
            .filter(|x| {
                string(get(x, "target_entity_kind").unwrap()).unwrap() == "execution_authorization"
                    && string(get(x, "target_entity_id").unwrap()).unwrap() == idv
            })
            .copied()
            .collect();
        let terminal: Vec<_> = ss
            .iter()
            .filter(|x| {
                matches!(
                    string(get(x, "state").unwrap()).unwrap(),
                    "cancelled" | "revoked" | "expired" | "consumed"
                )
            })
            .collect();
        if ss.is_empty() || ss.len() > 2 || terminal.len() > 1 {
            return Err(RequestRecordError::LineageConflict);
        }
        let used = cons
            .iter()
            .any(|x| string(get(x, "authorization_id").unwrap()).unwrap() == idv);
        if used
            != terminal
                .iter()
                .any(|x| string(get(x, "state").unwrap()).unwrap() == "consumed")
        {
            return Err(RequestRecordError::LineageConflict);
        }
        if let Some(t) = terminal.first() {
            let name = string(get(t, "state")?)?;
            let time = string(get(t, "effective_at")?)?;
            if time < issued
                || (name == "expired" && time != expires)
                || (name != "expired" && time >= expires)
            {
                return Err(RequestRecordError::LineageConflict);
            }
        }
    }
    for operation in &ops {
        let idv = string(get(operation, "operation_id")?)?;
        let ss: Vec<_> = states
            .iter()
            .filter(|x| {
                string(get(x, "target_entity_kind").unwrap()).unwrap() == "operation"
                    && string(get(x, "target_entity_id").unwrap()).unwrap() == idv
            })
            .copied()
            .collect();
        if ss.is_empty() {
            return Err(RequestRecordError::LineageConflict);
        }
        let completed = ss
            .iter()
            .any(|x| string(get(x, "state").unwrap()).unwrap() == "completed");
        let r = receipts
            .iter()
            .filter(|x| string(get(x, "operation_id").unwrap()).unwrap() == idv)
            .count();
        if (completed && r != 1) || (!completed && r != 0) {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for attempt in &attempts {
        let idv = string(get(attempt, "operation_attempt_id")?)?;
        let ss: Vec<_> = states
            .iter()
            .filter(|x| {
                string(get(x, "target_entity_kind").unwrap()).unwrap() == "operation_attempt"
                    && string(get(x, "target_entity_id").unwrap()).unwrap() == idv
            })
            .copied()
            .collect();
        if ss.is_empty() {
            return Err(RequestRecordError::LineageConflict);
        }
        let completed = ss
            .iter()
            .any(|x| string(get(x, "state").unwrap()).unwrap() == "completed");
        let r = receipts
            .iter()
            .filter(|x| string(get(x, "operation_attempt_id").unwrap()).unwrap() == idv)
            .count();
        if (completed && r != 1) || (!completed && r != 0) {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    for r in &receipts {
        let c = require(&cons, "consumption_id", string(get(r, "consumption_id")?)?)?;
        if string(get(c, "authorization_id")?)? != string(get(r, "authorization_id")?)?
            || string(get(c, "operation_id")?)? != string(get(r, "operation_id")?)?
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    Ok(())
}
fn commitments(r: &[(String, J)], e: &[(String, J)], obs: &[J]) -> Result<(), RequestRecordError> {
    let v = compact(&[
        get(r, "request_id")?,
        get(r, "packet_id")?,
        get(r, "packet_sha256")?,
        get(r, "project_ref")?,
        get(r, "resource_ref")?,
        get(e, "status")?,
        get(e, "policy_decision_id")?,
        get(e, "reason_code")?,
        get(e, "recorded_at")?,
    ])?;
    if hash(format!("lnsat.action_request.evaluation.v1\n{v}").as_bytes())
        != string(get(e, "evidence_digest")?)?
    {
        return Err(RequestRecordError::LineageConflict);
    }
    for x in obs {
        let o = obj(x)?;
        let v = compact(&[
            get(r, "request_id")?,
            get(o, "kind")?,
            get(o, "submitted_digest")?,
            get(o, "observed_at")?,
        ])?;
        if hash(format!("lnsat.action_request.observation.v1\n{v}").as_bytes())
            != string(get(o, "evidence_digest")?)?
        {
            return Err(RequestRecordError::LineageConflict);
        }
    }
    Ok(())
}

fn emit(
    i: &RequestRecordInput,
    states: (&str, &str, &str),
    native: J,
    security: J,
    d: Option<String>,
    failure: Option<&str>,
    counts: Option<Vec<(String, J)>>,
) -> Result<RequestRecordInspection, RequestRecordError> {
    let (a, e, p) = states;
    let truth = truth();
    let omissions = omissions();
    let source = object(vec![
        ("repository", s("hypler-dev/LNSAT")),
        ("commit", s(SOURCE_COMMIT)),
        ("wireContract", s(WIRE)),
        ("compatibility", s("source_pinned_not_runtime_qualified")),
        ("transportAuthentication", s("unverified")),
    ]);
    let select = object(vec![
        ("installationId", s(&i.installation_id)),
        ("environment", s(&i.environment)),
        ("requestId", s(&i.selected_request_id)),
        ("expectedProjectRef", s(&i.expected_project_ref)),
        ("expectedResourceRef", s(&i.expected_resource_ref)),
        ("readAt", i.read_at.as_ref().map_or(J::Null, |x| s(x))),
    ]);
    let host = object(vec![
        (
            "schemaVersion",
            s("rangoon.integrations.lnsat-request-record.v1"),
        ),
        ("profileVersion", J::Num("1".into())),
        ("source", source),
        ("selection", select),
        ("recordAvailability", s(a)),
        ("reportedEvaluation", s(e)),
        ("reportedPolicy", s(p)),
        ("securityActivityTouched", security.clone()),
        ("inputDigest", d.as_ref().map_or(J::Null, |x| s(x))),
        ("native", native),
        ("readFailure", failure.map_or(J::Null, s)),
        ("truth", truth.clone()),
        ("omissions", omissions.clone()),
    ]);
    let summary = object(vec![
        (
            "schemaVersion",
            s("rangoon.integrations.lnsat-request-summary.v1"),
        ),
        ("recordAvailability", s(a)),
        ("reportedEvaluation", s(e)),
        ("reportedPolicy", s(p)),
        ("readFailure", failure.map_or(J::Null, s)),
        ("securityActivityTouched", security),
        ("familyCounts", counts.map_or(J::Null, J::Obj)),
        ("truth", truth),
        ("omissions", omissions),
    ]);
    let h = encode(&host)?;
    let sm = encode(&summary)?;
    if h.len() > MAX_HOST_BYTES || sm.len() > MAX_SUMMARY_BYTES {
        return Err(RequestRecordError::OutputLimit);
    }
    Ok(RequestRecordInspection {
        host_json: h,
        summary_json: sm,
        input_digest: d,
    })
}
struct Parser<'a> {
    s: &'a str,
    i: usize,
    n: usize,
}
fn parse(b: &[u8]) -> Result<J, RequestRecordError> {
    if b.len() > MAX_INPUT_BYTES {
        return Err(RequestRecordError::InputLimit);
    }
    let s = core::str::from_utf8(b).map_err(|_| RequestRecordError::InvalidEncoding)?;
    if s.as_bytes().starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(RequestRecordError::InvalidEncoding);
    }
    let mut p = Parser { s, i: 0, n: 0 };
    p.ws();
    let v = p.value(1)?;
    p.ws();
    if p.i != s.len() {
        return Err(RequestRecordError::InvalidJson);
    }
    Ok(v)
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self
            .s
            .as_bytes()
            .get(self.i)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.i += 1
        }
    }
    fn value(&mut self, d: usize) -> Result<J, RequestRecordError> {
        if d > MAX_DEPTH {
            return Err(RequestRecordError::StructuralLimit);
        }
        self.n += 1;
        if self.n > MAX_VALUES {
            return Err(RequestRecordError::StructuralLimit);
        }
        self.ws();
        match self.s.as_bytes().get(self.i) {
            Some(b'{') => self.object(d),
            Some(b'[') => self.array(d),
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b't') => {
                self.word("true")?;
                Ok(J::Bool(true))
            }
            Some(b'f') => {
                self.word("false")?;
                Ok(J::Bool(false))
            }
            Some(b'n') => {
                self.word("null")?;
                Ok(J::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(RequestRecordError::InvalidJson),
        }
    }
    fn object(&mut self, d: usize) -> Result<J, RequestRecordError> {
        self.i += 1;
        self.ws();
        let mut o = Vec::new();
        if self.s.as_bytes().get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(J::Obj(o));
        }
        loop {
            self.ws();
            if self.s.as_bytes().get(self.i) != Some(&b'"') {
                return Err(RequestRecordError::InvalidJson);
            }
            let k = self.string()?;
            if o.iter().any(|x: &(String, J)| x.0 == k) {
                return Err(RequestRecordError::DuplicateField);
            }
            self.ws();
            if self.s.as_bytes().get(self.i) != Some(&b':') {
                return Err(RequestRecordError::InvalidJson);
            }
            self.i += 1;
            let v = self.value(d + 1)?;
            o.push((k, v));
            if o.len() > MAX_OBJECT_FIELDS {
                return Err(RequestRecordError::StructuralLimit);
            }
            self.ws();
            match self.s.as_bytes().get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(RequestRecordError::InvalidJson),
            }
        }
        Ok(J::Obj(o))
    }
    fn array(&mut self, d: usize) -> Result<J, RequestRecordError> {
        self.i += 1;
        self.ws();
        let mut a = Vec::new();
        if self.s.as_bytes().get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(J::Arr(a));
        }
        loop {
            a.push(self.value(d + 1)?);
            if a.len() > MAX_ARRAY_ITEMS {
                return Err(RequestRecordError::StructuralLimit);
            }
            self.ws();
            match self.s.as_bytes().get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(RequestRecordError::InvalidJson),
            }
        }
        Ok(J::Arr(a))
    }
    fn word(&mut self, w: &str) -> Result<(), RequestRecordError> {
        if self.s[self.i..].starts_with(w) {
            self.i += w.len();
            Ok(())
        } else {
            Err(RequestRecordError::InvalidJson)
        }
    }
    fn number(&mut self) -> Result<J, RequestRecordError> {
        let st = self.i;
        let b = self.s.as_bytes();
        if b[self.i] == b'-' {
            self.i += 1
        }
        if b.get(self.i) == Some(&b'0') {
            self.i += 1
        } else {
            let x = self.i;
            while b.get(self.i).is_some_and(u8::is_ascii_digit) {
                self.i += 1
            }
            if x == self.i {
                return Err(RequestRecordError::InvalidJson);
            }
        }
        if b.get(self.i) == Some(&b'.') {
            self.i += 1;
            let x = self.i;
            while b.get(self.i).is_some_and(u8::is_ascii_digit) {
                self.i += 1
            }
            if x == self.i {
                return Err(RequestRecordError::InvalidJson);
            }
        }
        if matches!(b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1
            }
            let x = self.i;
            while b.get(self.i).is_some_and(u8::is_ascii_digit) {
                self.i += 1
            }
            if x == self.i {
                return Err(RequestRecordError::InvalidJson);
            }
        }
        let x = &self.s[st..self.i];
        if x.parse::<f64>().ok().filter(|x| x.is_finite()).is_none() {
            return Err(RequestRecordError::InvalidJson);
        }
        Ok(J::Num(x.into()))
    }
    fn string(&mut self) -> Result<String, RequestRecordError> {
        self.i += 1;
        let mut o = String::new();
        loop {
            let c = self.s[self.i..]
                .chars()
                .next()
                .ok_or(RequestRecordError::InvalidJson)?;
            self.i += c.len_utf8();
            match c {
                '"' => break,
                '\\' => {
                    let e = self.s[self.i..]
                        .chars()
                        .next()
                        .ok_or(RequestRecordError::InvalidJson)?;
                    self.i += e.len_utf8();
                    match e {
                        '"' => o.push('"'),
                        '\\' => o.push('\\'),
                        '/' => o.push('/'),
                        'b' => o.push('\u{08}'),
                        'f' => o.push('\u{0c}'),
                        'n' => o.push('\n'),
                        'r' => o.push('\r'),
                        't' => o.push('\t'),
                        'u' => {
                            let x = self.hex()?;
                            if (0xd800..=0xdbff).contains(&x) {
                                if !self.s[self.i..].starts_with("\\u") {
                                    return Err(RequestRecordError::InvalidJson);
                                }
                                self.i += 2;
                                let y = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&y) {
                                    return Err(RequestRecordError::InvalidJson);
                                }
                                o.push(
                                    char::from_u32(0x10000 + ((x - 0xd800) << 10) + (y - 0xdc00))
                                        .ok_or(RequestRecordError::InvalidJson)?,
                                )
                            } else if (0xdc00..=0xdfff).contains(&x) {
                                return Err(RequestRecordError::InvalidJson);
                            } else {
                                o.push(char::from_u32(x).ok_or(RequestRecordError::InvalidJson)?)
                            }
                        }
                        _ => return Err(RequestRecordError::InvalidJson),
                    }
                }
                x if x < '\u{20}' => return Err(RequestRecordError::InvalidJson),
                x => o.push(x),
            }
            if o.len() > MAX_STRING_BYTES {
                return Err(RequestRecordError::StructuralLimit);
            }
        }
        Ok(o)
    }
    fn hex(&mut self) -> Result<u32, RequestRecordError> {
        if self.i + 4 > self.s.len() {
            return Err(RequestRecordError::InvalidJson);
        }
        let bytes = self
            .s
            .as_bytes()
            .get(self.i..self.i + 4)
            .ok_or(RequestRecordError::InvalidJson)?;
        if !bytes.iter().all(u8::is_ascii_hexdigit) {
            return Err(RequestRecordError::InvalidJson);
        }
        let x = core::str::from_utf8(bytes).map_err(|_| RequestRecordError::InvalidJson)?;
        self.i += 4;
        u32::from_str_radix(x, 16).map_err(|_| RequestRecordError::InvalidJson)
    }
}
fn truth() -> J {
    object(vec![
        ("authenticity", s("unverified")),
        ("authority", s("none")),
        ("execution", s("unavailable")),
        ("authoritativeOutcome", s("unknown")),
        ("independentEffect", s("not_observed")),
        ("currentness", s("unverified")),
    ])
}
fn omissions() -> J {
    J::Arr(vec![
        object(vec![
            ("path", s("request.requester_ref")),
            ("reason", s("privacy_projection")),
            ("disposition", s("policy_omission_not_presence_claim")),
        ]),
        object(vec![
            ("path", s("evidence.approval_decisions[*].reason")),
            ("reason", s("privacy_projection")),
            ("disposition", s("policy_omission_not_presence_claim")),
        ]),
    ])
}
fn time(v: &str) -> Result<(), RequestRecordError> {
    let b = v.as_bytes();
    if b.len() != 24
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'.'
        || b[23] != b'Z'
        || !b
            .iter()
            .enumerate()
            .filter(|(i, _)| ![4, 7, 10, 13, 16, 19, 23].contains(i))
            .all(|(_, x)| x.is_ascii_digit())
    {
        return Err(RequestRecordError::InvalidEnvelope);
    }
    let y: u32 = v[0..4]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let m: u32 = v[5..7]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let d: u32 = v[8..10]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let h: u32 = v[11..13]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let min: u32 = v[14..16]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let sec: u32 = v[17..19]
        .parse()
        .map_err(|_| RequestRecordError::InvalidEnvelope)?;
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => return Err(RequestRecordError::InvalidEnvelope),
    };
    if d == 0 || d > max || h > 23 || min > 59 || sec > 59 {
        Err(RequestRecordError::InvalidEnvelope)
    } else {
        Ok(())
    }
}
fn compact(a: &[&J]) -> Result<String, RequestRecordError> {
    let mut x = String::from("[");
    for (i, v) in a.iter().enumerate() {
        if i > 0 {
            x.push(',')
        }
        x.push_str(
            core::str::from_utf8(&encode(v)?)
                .map_err(|_| RequestRecordError::SerializationFailed)?,
        )
    }
    x.push(']');
    Ok(x)
}
fn encode(v: &J) -> Result<Vec<u8>, RequestRecordError> {
    let mut x = String::new();
    write(v, &mut x)?;
    Ok(x.into_bytes())
}
fn write(v: &J, o: &mut String) -> Result<(), RequestRecordError> {
    match v {
        J::Null => o.push_str("null"),
        J::Bool(x) => o.push_str(if *x { "true" } else { "false" }),
        J::Num(x) => o.push_str(x),
        J::Str(x) => quoted(x, o),
        J::Arr(a) => {
            o.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    o.push(',')
                }
                write(x, o)?
            }
            o.push(']')
        }
        J::Obj(a) => {
            o.push('{');
            for (i, (k, x)) in a.iter().enumerate() {
                if i > 0 {
                    o.push(',')
                }
                quoted(k, o);
                o.push(':');
                write(x, o)?
            }
            o.push('}')
        }
    }
    if o.len() > MAX_HOST_BYTES {
        return Err(RequestRecordError::OutputLimit);
    }
    Ok(())
}
fn quoted(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{08}' => o.push_str("\\b"),
            '\u{0c}' => o.push_str("\\f"),
            x if x < '\u{20}' => {
                use core::fmt::Write;
                let _ = write!(o, "\\u{:04x}", x as u32);
            }
            x => o.push(x),
        }
    }
    o.push('"')
}
#[derive(Clone)]
enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}
fn s(x: &str) -> J {
    J::Str(x.into())
}
fn object(x: Vec<(&str, J)>) -> J {
    J::Obj(x.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn obj(v: &J) -> Result<&[(String, J)], RequestRecordError> {
    if let J::Obj(x) = v {
        Ok(x)
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn array(v: &J) -> Result<&[J], RequestRecordError> {
    if let J::Arr(x) = v {
        Ok(x)
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn get<'a>(o: &'a [(String, J)], k: &str) -> Result<&'a J, RequestRecordError> {
    o.iter()
        .find(|x| x.0 == k)
        .map(|x| &x.1)
        .ok_or(RequestRecordError::InvalidEnvelope)
}
fn string(v: &J) -> Result<&str, RequestRecordError> {
    if let J::Str(x) = v {
        Ok(x)
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn boolean(v: &J) -> Result<bool, RequestRecordError> {
    if let J::Bool(x) = v {
        Ok(*x)
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn integer(v: &J) -> Result<i64, RequestRecordError> {
    if let J::Num(x) = v {
        if x.contains(['.', 'e', 'E']) {
            Err(RequestRecordError::InvalidEnvelope)
        } else {
            x.parse().map_err(|_| RequestRecordError::InvalidEnvelope)
        }
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn null(v: &J) -> Result<(), RequestRecordError> {
    if matches!(v, J::Null) {
        Ok(())
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn eq(v: &J, x: &str, e: RequestRecordError) -> Result<(), RequestRecordError> {
    if string(v)? == x { Ok(()) } else { Err(e) }
}
fn keys(o: &[(String, J)], e: &[&str]) -> Result<(), RequestRecordError> {
    if o.len() != e.len() || o.iter().any(|x| !e.contains(&x.0.as_str())) {
        Err(RequestRecordError::InvalidEnvelope)
    } else {
        Ok(())
    }
}
fn redact(o: &[(String, J)], r: &[&str]) -> Vec<(String, J)> {
    o.iter()
        .filter(|x| !r.contains(&x.0.as_str()))
        .cloned()
        .collect()
}
fn canonical(o: &[(String, J)], order: &[&str]) -> Result<Vec<(String, J)>, RequestRecordError> {
    order
        .iter()
        .map(|k| Ok(((*k).into(), get(o, k)?.clone())))
        .collect()
}
fn objects(x: &[J]) -> Result<Vec<&[(String, J)]>, RequestRecordError> {
    x.iter().map(obj).collect()
}
fn text(v: &str, n: usize) -> Result<(), RequestRecordError> {
    if v.is_empty() {
        Err(RequestRecordError::InvalidEnvelope)
    } else if v.len() > n {
        Err(RequestRecordError::InputLimit)
    } else {
        Ok(())
    }
}
fn id(v: &str, p: &str) -> Result<(), RequestRecordError> {
    if v.len() == 68
        && v.starts_with(p)
        && v.as_bytes()[4..]
            .iter()
            .all(|x| matches!(x,b'0'..=b'9'|b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn digest(v: &str) -> Result<(), RequestRecordError> {
    if v.len() == 71
        && v.starts_with("sha256:")
        && v.as_bytes()[7..]
            .iter()
            .all(|x| matches!(x,b'0'..=b'9'|b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(RequestRecordError::InvalidEnvelope)
    }
}
fn hash(b: &[u8]) -> String {
    format!("sha256:{}", byte_digest(b))
}
