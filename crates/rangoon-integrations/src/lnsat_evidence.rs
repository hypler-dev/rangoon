//! Strict offline decoder for the pinned synthetic LNSAT read envelopes.
//! It accepts caller-owned bytes only. It has no network, credential, approval, or execution role.

use rangoon_domain::byte_digest;
use serde::Serialize;
use serde::de::{self, DeserializeSeed, Deserializer as _, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

pub const LNSAT_EVIDENCE_CONTRACT: &str = "lnsat.gateway.runtime_composition.v1_0";
pub const LNSAT_EVIDENCE_COMMIT: &str = "f669181a1eaf574b1891a5c3fcc838870097e176";
pub const MAX_LNSAT_EVIDENCE_BYTES: usize = 262_144;
pub const MAX_LNSAT_EVIDENCE_DEPTH: usize = 16;
pub const MAX_LNSAT_EVIDENCE_VALUES: usize = 4_096;
pub const MAX_LNSAT_EVIDENCE_FIELDS: usize = 48;
pub const MAX_LNSAT_EVIDENCE_ITEMS: usize = 128;
pub const MAX_LNSAT_EVIDENCE_STRING_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug)]
pub struct LnsatEvidenceInput<'a> {
    pub installation_id: &'a str,
    pub environment: &'a str,
    pub selected_authorization_id: Option<&'a str>,
    pub selected_operation_id: Option<&'a str>,
    pub selected_operation_attempt_id: Option<&'a str>,
    pub authorization: Option<&'a [u8]>,
    pub operation: Option<&'a [u8]>,
    pub attempt: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LnsatEvidenceError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    DuplicateField,
    DepthLimit,
    ValueLimit,
    CollectionLimit,
    UnknownField,
    UnsupportedContract,
    InvalidEnvelope,
    SelectionRequired,
    IdMismatch,
    ScopeMismatch,
    ReferenceMismatch,
    AttemptMismatch,
    OutputLimit,
    SerializationFailed,
}

impl LnsatEvidenceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputLimit => "lnsat_evidence_input_limit",
            Self::InvalidEncoding => "lnsat_evidence_invalid_encoding",
            Self::InvalidJson => "lnsat_evidence_invalid_json",
            Self::DuplicateField => "lnsat_evidence_duplicate_field",
            Self::DepthLimit => "lnsat_evidence_depth_limit",
            Self::ValueLimit => "lnsat_evidence_value_limit",
            Self::CollectionLimit => "lnsat_evidence_collection_limit",
            Self::UnknownField => "lnsat_evidence_unknown_field",
            Self::UnsupportedContract => "lnsat_evidence_unsupported_contract",
            Self::InvalidEnvelope => "lnsat_evidence_invalid_envelope",
            Self::SelectionRequired => "lnsat_evidence_selection_required",
            Self::IdMismatch => "lnsat_evidence_id_mismatch",
            Self::ScopeMismatch => "lnsat_evidence_scope_mismatch",
            Self::ReferenceMismatch => "lnsat_evidence_reference_mismatch",
            Self::AttemptMismatch => "lnsat_evidence_attempt_mismatch",
            Self::OutputLimit => "lnsat_evidence_output_limit",
            Self::SerializationFailed => "lnsat_evidence_serialization_failed",
        }
    }
}

impl fmt::Display for LnsatEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for LnsatEvidenceError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatEvidenceProjection {
    pub schema_version: &'static str,
    pub source: &'static str,
    pub pinned_source_commit: &'static str,
    pub availability: &'static str,
    pub scope: LnsatEvidenceScope,
    pub input_digests: Vec<LnsatEvidenceDigest>,
    pub authorization: Option<LnsatAuthorizationEvidence>,
    pub operation: Option<LnsatOperationEvidence>,
    pub attempt: Option<LnsatAttemptEvidence>,
    pub read_rejections: Vec<LnsatReadRejection>,
    pub losses: Vec<&'static str>,
    pub rangoon_authority: &'static str,
    pub approval: &'static str,
    pub execution: &'static str,
    pub verified_effect: &'static str,
    pub cross_engine_join: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatEvidenceScope {
    pub installation_id: String,
    pub environment: String,
    pub verification: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatEvidenceDigest {
    pub response: &'static str,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatReadRejection {
    pub response: &'static str,
    pub state: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatAuthorizationEvidence {
    pub authorization_id: String,
    pub audit_binding_id: String,
    pub project_ref: String,
    pub resource_ref: String,
    pub authorization_attempt_id: String,
    pub nonce_id: String,
    pub binding_digest: String,
    pub approval_decision_id: String,
    pub policy_decision_id: String,
    pub packet_id: String,
    pub packet_sha256: String,
    pub action_digest: String,
    pub target_digest: String,
    pub configuration_digest: String,
    pub adapter_ref: String,
    pub executable_digest: String,
    pub audience: String,
    pub authorization_profile: String,
    pub issued_at: String,
    pub expires_at: String,
    pub state_event_id: String,
    pub state_audit_binding_id: String,
    pub state_sequence: u32,
    pub native_state: String,
    pub state_projection: &'static str,
    pub state_effective_at: String,
    pub active: bool,
    pub operation_id: String,
    pub operation_audit_binding_id: String,
    pub operation_idempotency_key: String,
    pub operation_request_digest: String,
    pub omitted_fields: [&'static str; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatOperationEvidence {
    pub operation_id: String,
    pub operation_audit_binding_id: String,
    pub authorization_id: String,
    pub consumption_id: Option<String>,
    pub project_ref: String,
    pub resource_ref: String,
    pub state_event_id: String,
    pub state_audit_binding_id: String,
    pub state_sequence: u32,
    pub native_state: String,
    pub state_projection: &'static str,
    pub state_effective_at: String,
    pub receipt: Option<LnsatReceiptEvidence>,
    pub reconciliation: Option<LnsatReconciliationEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatAttemptEvidence {
    pub operation_attempt_id: String,
    pub audit_binding_id: String,
    pub operation_id: String,
    pub project_ref: String,
    pub resource_ref: String,
    pub attempt_sequence: u32,
    pub adapter_ref: String,
    pub protocol_version: String,
    pub tool_arguments_digest: String,
    pub created_at: String,
    pub state_event_id: String,
    pub state_audit_binding_id: String,
    pub state_sequence: u32,
    pub native_state: String,
    pub state_projection: &'static str,
    pub state_effective_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatReceiptEvidence {
    pub receipt_id: String,
    pub received_at: Option<String>,
    pub effect: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LnsatReconciliationEvidence {
    pub reconciliation_id: String,
    pub status: Option<String>,
    pub recorded_at: Option<String>,
    pub meaning: &'static str,
}

enum ParsedEnvelope {
    Authorization(LnsatAuthorizationEvidence),
    Operation(LnsatOperationEvidence, Option<LnsatAttemptEvidence>),
    Attempt(LnsatAttemptEvidence),
    Rejected,
}

/// Decodes only pinned, caller-supplied JSON envelopes. It never performs I/O or grants authority.
pub fn decode_lnsat_evidence(
    input: LnsatEvidenceInput<'_>,
) -> Result<LnsatEvidenceProjection, LnsatEvidenceError> {
    validate_scope_value(input.installation_id)?;
    validate_scope_value(input.environment)?;
    validate_selection_shape(input)?;

    let supplied = [input.authorization, input.operation, input.attempt];
    let total: usize = supplied.into_iter().flatten().map(<[u8]>::len).sum();
    if total > MAX_LNSAT_EVIDENCE_BYTES {
        return Err(LnsatEvidenceError::InputLimit);
    }

    let authorization = parse_envelope(input.authorization, EnvelopeKind::Authorization)?;
    let operation = parse_envelope(input.operation, EnvelopeKind::Operation)?;
    let attempt = parse_envelope(input.attempt, EnvelopeKind::Attempt)?;

    let mut digests = Vec::new();
    let mut rejections = Vec::new();
    for (response, bytes, parsed) in [
        ("authorization", input.authorization, &authorization),
        ("operation", input.operation, &operation),
        ("attempt", input.attempt, &attempt),
    ] {
        if let Some(bytes) = bytes {
            digests.push(LnsatEvidenceDigest {
                response,
                sha256: byte_digest(bytes),
            });
        }
        if matches!(parsed, Some(ParsedEnvelope::Rejected)) {
            rejections.push(LnsatReadRejection {
                response,
                state: "read_rejected_unclassified",
            });
        }
    }

    let authorization = match authorization {
        Some(ParsedEnvelope::Authorization(value)) => Some(value),
        Some(ParsedEnvelope::Rejected) | None => None,
        _ => return Err(LnsatEvidenceError::InvalidEnvelope),
    };
    let (operation, nested_attempt) = match operation {
        Some(ParsedEnvelope::Operation(value, nested)) => (Some(value), nested),
        Some(ParsedEnvelope::Rejected) | None => (None, None),
        _ => return Err(LnsatEvidenceError::InvalidEnvelope),
    };
    let standalone_attempt = match attempt {
        Some(ParsedEnvelope::Attempt(value)) => Some(value),
        Some(ParsedEnvelope::Rejected) | None => None,
        _ => return Err(LnsatEvidenceError::InvalidEnvelope),
    };

    validate_selected_ids(
        &input,
        authorization.as_ref(),
        operation.as_ref(),
        nested_attempt.as_ref(),
        standalone_attempt.as_ref(),
    )?;
    validate_joins(
        &input,
        authorization.as_ref(),
        operation.as_ref(),
        nested_attempt.as_ref(),
        standalone_attempt.as_ref(),
    )?;

    let attempt = standalone_attempt.or(nested_attempt);
    let availability = if digests.is_empty() {
        "provider_absent"
    } else {
        "offline_envelopes_supplied"
    };
    let mut losses = vec![
        "caller_declared_scope_unverified",
        "source_authenticity_unverified",
        "approval_identity_not_observed",
        "approval_chain_not_observed",
        "independent_effect_not_observed",
        "denial_history_unsupported_by_selected_read_surface",
        "cross_engine_join_not_observed",
    ];
    if !rejections.is_empty() {
        losses.push("read_rejection_cause_unclassified");
    }
    let output = LnsatEvidenceProjection {
        schema_version: "rangoon.lnsat.evidence.v0.1",
        source: "lnsat_phase8_public_main_offline",
        pinned_source_commit: LNSAT_EVIDENCE_COMMIT,
        availability,
        scope: LnsatEvidenceScope {
            installation_id: input.installation_id.to_owned(),
            environment: input.environment.to_owned(),
            verification: "caller_declared_unverified",
        },
        input_digests: digests,
        authorization,
        operation,
        attempt,
        read_rejections: rejections,
        losses,
        rangoon_authority: "none",
        approval: "not_inferred",
        execution: "not_inferred",
        verified_effect: "not_observed",
        cross_engine_join: "not_observed",
    };
    let bytes = serde_json::to_vec(&output).map_err(|_| LnsatEvidenceError::SerializationFailed)?;
    if bytes.len() > MAX_LNSAT_EVIDENCE_BYTES {
        Err(LnsatEvidenceError::OutputLimit)
    } else {
        Ok(output)
    }
}

#[derive(Clone, Copy)]
enum EnvelopeKind {
    Authorization,
    Operation,
    Attempt,
}

impl EnvelopeKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Authorization => "authorization",
            Self::Operation => "operation",
            Self::Attempt => "attempt",
        }
    }
}

fn validate_selection_shape(input: LnsatEvidenceInput<'_>) -> Result<(), LnsatEvidenceError> {
    if (input.authorization.is_some() || input.operation.is_some())
        && input.selected_authorization_id.is_none()
    {
        return Err(LnsatEvidenceError::SelectionRequired);
    }
    if (input.operation.is_some() || input.attempt.is_some())
        && input.selected_operation_id.is_none()
    {
        return Err(LnsatEvidenceError::SelectionRequired);
    }
    if input.attempt.is_some() && input.selected_operation_attempt_id.is_none() {
        return Err(LnsatEvidenceError::SelectionRequired);
    }
    if let Some(value) = input.selected_authorization_id {
        validate_selector(value, "xau_")?;
    }
    if let Some(value) = input.selected_operation_id {
        validate_selector(value, "opn_")?;
    }
    if let Some(value) = input.selected_operation_attempt_id {
        validate_selector(value, "opa_")?;
    }
    Ok(())
}

fn parse_envelope(
    bytes: Option<&[u8]>,
    kind: EnvelopeKind,
) -> Result<Option<ParsedEnvelope>, LnsatEvidenceError> {
    let Some(bytes) = bytes else { return Ok(None) };
    std::str::from_utf8(bytes).map_err(|_| LnsatEvidenceError::InvalidEncoding)?;
    reject_duplicate_fields(bytes)?;
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| LnsatEvidenceError::InvalidJson)?;
    validate_value_limits(&value, 1, &mut 0)?;
    let object = as_object(&value)?;
    if object.get("status") == Some(&Value::String("error".to_owned())) {
        validate_rejection(object)?;
        return Ok(Some(ParsedEnvelope::Rejected));
    }
    exact_keys(object, &["contract", "status", kind.name()])?;
    if required_string(object, "contract")? != LNSAT_EVIDENCE_CONTRACT {
        return Err(LnsatEvidenceError::UnsupportedContract);
    }
    if required_string(object, "status")? != "ok" {
        return Err(LnsatEvidenceError::InvalidEnvelope);
    }
    let parsed = match kind {
        EnvelopeKind::Authorization => ParsedEnvelope::Authorization(parse_authorization(
            as_object(required(object, "authorization")?)?,
        )?),
        EnvelopeKind::Operation => {
            let (operation, nested) = parse_operation(as_object(required(object, "operation")?)?)?;
            ParsedEnvelope::Operation(operation, nested)
        }
        EnvelopeKind::Attempt => {
            ParsedEnvelope::Attempt(parse_attempt(as_object(required(object, "attempt")?)?)?)
        }
    };
    Ok(Some(parsed))
}

fn validate_rejection(object: &serde_json::Map<String, Value>) -> Result<(), LnsatEvidenceError> {
    exact_keys(object, &["contract", "status", "code"])?;
    if required_string(object, "contract")? != LNSAT_EVIDENCE_CONTRACT
        || required_string(object, "status")? != "error"
        || required_string(object, "code")? != "gateway.runtime_composition.denied"
    {
        return Err(LnsatEvidenceError::InvalidEnvelope);
    }
    Ok(())
}

fn parse_authorization(
    object: &serde_json::Map<String, Value>,
) -> Result<LnsatAuthorizationEvidence, LnsatEvidenceError> {
    exact_keys(object, AUTHORIZATION_FIELDS)?;
    for field in [
        "requester_ref",
        "requester_session_ref",
        "approver_ref",
        "approver_session_ref",
    ] {
        required_string(object, field)?;
    }
    Ok(LnsatAuthorizationEvidence {
        authorization_id: required_string(object, "authorization_id")?.to_owned(),
        audit_binding_id: required_string(object, "audit_binding_id")?.to_owned(),
        project_ref: required_string(object, "project_ref")?.to_owned(),
        resource_ref: required_string(object, "resource_ref")?.to_owned(),
        authorization_attempt_id: required_string(object, "authorization_attempt_id")?.to_owned(),
        nonce_id: required_string(object, "nonce_id")?.to_owned(),
        binding_digest: required_native_digest(object, "binding_digest")?,
        approval_decision_id: required_string(object, "approval_decision_id")?.to_owned(),
        policy_decision_id: required_string(object, "policy_decision_id")?.to_owned(),
        packet_id: required_string(object, "packet_id")?.to_owned(),
        packet_sha256: required_native_digest(object, "packet_sha256")?,
        action_digest: required_native_digest(object, "action_digest")?,
        target_digest: required_native_digest(object, "target_digest")?,
        configuration_digest: required_native_digest(object, "configuration_digest")?,
        adapter_ref: required_string(object, "adapter_ref")?.to_owned(),
        executable_digest: required_native_digest(object, "executable_digest")?,
        audience: required_string(object, "audience")?.to_owned(),
        authorization_profile: required_string(object, "authorization_profile")?.to_owned(),
        issued_at: required_string(object, "issued_at")?.to_owned(),
        expires_at: required_string(object, "expires_at")?.to_owned(),
        state_event_id: required_string(object, "state_event_id")?.to_owned(),
        state_audit_binding_id: required_string(object, "state_audit_binding_id")?.to_owned(),
        state_sequence: required_u32(object, "state_sequence")?,
        native_state: required_string(object, "state")?.to_owned(),
        state_projection: state_projection(
            required_string(object, "state")?,
            &["active", "consumed", "cancelled", "expired", "revoked"],
        ),
        state_effective_at: required_string(object, "state_effective_at")?.to_owned(),
        active: required_bool(object, "active")?,
        operation_id: required_string(object, "operation_id")?.to_owned(),
        operation_audit_binding_id: required_string(object, "operation_audit_binding_id")?
            .to_owned(),
        operation_idempotency_key: required_string(object, "operation_idempotency_key")?.to_owned(),
        operation_request_digest: required_native_digest(object, "operation_request_digest")?,
        omitted_fields: [
            "requester_ref",
            "requester_session_ref",
            "approver_ref",
            "approver_session_ref",
        ],
    })
}

fn parse_operation(
    object: &serde_json::Map<String, Value>,
) -> Result<(LnsatOperationEvidence, Option<LnsatAttemptEvidence>), LnsatEvidenceError> {
    exact_keys(object, OPERATION_FIELDS)?;
    let nested = match required(object, "attempt")? {
        Value::Null => None,
        value => Some(parse_attempt(as_object(value)?)?),
    };
    let receipt = match required(object, "receipt")? {
        Value::Null => None,
        value => {
            let value = as_object(value)?;
            exact_keys(value, &["receipt_id", "received_at"])?;
            Some(LnsatReceiptEvidence {
                receipt_id: required_string(value, "receipt_id")?.to_owned(),
                received_at: optional_string(value, "received_at")?,
                effect: "reported_receipt_not_independent_effect_proof",
            })
        }
    };
    let reconciliation = match required(object, "reconciliation")? {
        Value::Null => None,
        value => {
            let value = as_object(value)?;
            exact_keys(value, &["reconciliation_id", "status", "recorded_at"])?;
            Some(LnsatReconciliationEvidence {
                reconciliation_id: required_string(value, "reconciliation_id")?.to_owned(),
                status: optional_string(value, "status")?,
                recorded_at: optional_string(value, "recorded_at")?,
                meaning: "already_recorded_not_invoked_by_rangoon",
            })
        }
    };
    let state = required_string(object, "state")?;
    Ok((
        LnsatOperationEvidence {
            operation_id: required_string(object, "operation_id")?.to_owned(),
            operation_audit_binding_id: required_string(object, "operation_audit_binding_id")?
                .to_owned(),
            authorization_id: required_string(object, "authorization_id")?.to_owned(),
            consumption_id: optional_string(object, "consumption_id")?,
            project_ref: required_string(object, "project_ref")?.to_owned(),
            resource_ref: required_string(object, "resource_ref")?.to_owned(),
            state_event_id: required_string(object, "state_event_id")?.to_owned(),
            state_audit_binding_id: required_string(object, "state_audit_binding_id")?.to_owned(),
            state_sequence: required_u32(object, "state_sequence")?,
            native_state: state.to_owned(),
            state_projection: state_projection(
                state,
                &[
                    "prepared",
                    "dispatching",
                    "outcome_unknown",
                    "completed",
                    "failed",
                ],
            ),
            state_effective_at: required_string(object, "state_effective_at")?.to_owned(),
            receipt,
            reconciliation,
        },
        nested,
    ))
}

fn parse_attempt(
    object: &serde_json::Map<String, Value>,
) -> Result<LnsatAttemptEvidence, LnsatEvidenceError> {
    exact_keys(object, ATTEMPT_FIELDS)?;
    let state = required_string(object, "state")?;
    Ok(LnsatAttemptEvidence {
        operation_attempt_id: required_string(object, "operation_attempt_id")?.to_owned(),
        audit_binding_id: required_string(object, "audit_binding_id")?.to_owned(),
        operation_id: required_string(object, "operation_id")?.to_owned(),
        project_ref: required_string(object, "project_ref")?.to_owned(),
        resource_ref: required_string(object, "resource_ref")?.to_owned(),
        attempt_sequence: required_u32(object, "attempt_sequence")?,
        adapter_ref: required_string(object, "adapter_ref")?.to_owned(),
        protocol_version: required_string(object, "protocol_version")?.to_owned(),
        tool_arguments_digest: required_native_digest(object, "tool_arguments_digest")?,
        created_at: required_string(object, "created_at")?.to_owned(),
        state_event_id: required_string(object, "state_event_id")?.to_owned(),
        state_audit_binding_id: required_string(object, "state_audit_binding_id")?.to_owned(),
        state_sequence: required_u32(object, "state_sequence")?,
        native_state: state.to_owned(),
        state_projection: state_projection(
            state,
            &["dispatching", "outcome_unknown", "completed", "failed"],
        ),
        state_effective_at: required_string(object, "state_effective_at")?.to_owned(),
    })
}

fn validate_selected_ids(
    input: &LnsatEvidenceInput<'_>,
    authorization: Option<&LnsatAuthorizationEvidence>,
    operation: Option<&LnsatOperationEvidence>,
    nested_attempt: Option<&LnsatAttemptEvidence>,
    standalone_attempt: Option<&LnsatAttemptEvidence>,
) -> Result<(), LnsatEvidenceError> {
    if let (Some(selected), Some(value)) = (input.selected_authorization_id, authorization)
        && selected != value.authorization_id
    {
        return Err(LnsatEvidenceError::IdMismatch);
    }
    if let (Some(selected), Some(value)) = (input.selected_operation_id, operation)
        && selected != value.operation_id
    {
        return Err(LnsatEvidenceError::IdMismatch);
    }
    if let (Some(selected), Some(value)) = (input.selected_authorization_id, operation)
        && selected != value.authorization_id
    {
        return Err(LnsatEvidenceError::IdMismatch);
    }
    if let Some(value) = nested_attempt
        && (input.selected_operation_id != Some(value.operation_id.as_str())
            || input
                .selected_operation_attempt_id
                .is_some_and(|selected| selected != value.operation_attempt_id))
    {
        return Err(LnsatEvidenceError::IdMismatch);
    }
    if let Some(value) = standalone_attempt
        && (input.selected_operation_id != Some(value.operation_id.as_str())
            || input.selected_operation_attempt_id != Some(value.operation_attempt_id.as_str()))
    {
        return Err(LnsatEvidenceError::IdMismatch);
    }
    Ok(())
}

fn validate_joins(
    input: &LnsatEvidenceInput<'_>,
    authorization: Option<&LnsatAuthorizationEvidence>,
    operation: Option<&LnsatOperationEvidence>,
    nested_attempt: Option<&LnsatAttemptEvidence>,
    standalone_attempt: Option<&LnsatAttemptEvidence>,
) -> Result<(), LnsatEvidenceError> {
    if let (Some(authorization), Some(selected_operation_id)) =
        (authorization, input.selected_operation_id)
        && authorization.operation_id != selected_operation_id
    {
        return Err(LnsatEvidenceError::ReferenceMismatch);
    }
    if let Some(authorization) = authorization {
        for attempt in [nested_attempt, standalone_attempt].into_iter().flatten() {
            if authorization.adapter_ref != attempt.adapter_ref {
                return Err(LnsatEvidenceError::ReferenceMismatch);
            }
        }
    }
    if operation.is_none()
        && let (Some(authorization), Some(attempt)) = (authorization, standalone_attempt)
        && (authorization.project_ref != attempt.project_ref
            || authorization.resource_ref != attempt.resource_ref)
    {
        return Err(LnsatEvidenceError::ScopeMismatch);
    }
    if let (Some(authorization), Some(operation)) = (authorization, operation) {
        if authorization.authorization_id != operation.authorization_id {
            return Err(LnsatEvidenceError::ReferenceMismatch);
        }
        if authorization.project_ref != operation.project_ref
            || authorization.resource_ref != operation.resource_ref
        {
            return Err(LnsatEvidenceError::ScopeMismatch);
        }
        if authorization.operation_id != operation.operation_id
            || authorization.operation_audit_binding_id != operation.operation_audit_binding_id
        {
            return Err(LnsatEvidenceError::ReferenceMismatch);
        }
    }
    if let Some(operation) = operation {
        for attempt in [nested_attempt, standalone_attempt].into_iter().flatten() {
            if attempt.operation_id != operation.operation_id {
                return Err(LnsatEvidenceError::ReferenceMismatch);
            }
            if attempt.project_ref != operation.project_ref
                || attempt.resource_ref != operation.resource_ref
            {
                return Err(LnsatEvidenceError::ScopeMismatch);
            }
        }
    }
    if let (Some(nested), Some(standalone)) = (nested_attempt, standalone_attempt)
        && nested != standalone
    {
        return Err(LnsatEvidenceError::AttemptMismatch);
    }
    Ok(())
}

fn state_projection(state: &str, known: &[&str]) -> &'static str {
    if known.contains(&state) {
        "reported_native_state"
    } else {
        "unsupported_native_state"
    }
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a Value, LnsatEvidenceError> {
    object.get(key).ok_or(LnsatEvidenceError::InvalidEnvelope)
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, LnsatEvidenceError> {
    let value = required(object, key)?
        .as_str()
        .ok_or(LnsatEvidenceError::InvalidEnvelope)?;
    validate_native_string(value)?;
    Ok(value)
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, LnsatEvidenceError> {
    match required(object, key)? {
        Value::Null => Ok(None),
        Value::String(value) => {
            validate_native_string(value)?;
            Ok(Some(value.clone()))
        }
        _ => Err(LnsatEvidenceError::InvalidEnvelope),
    }
}

fn required_native_digest(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, LnsatEvidenceError> {
    let value = required_string(object, key)?;
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(LnsatEvidenceError::InvalidEnvelope);
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(LnsatEvidenceError::InvalidEnvelope);
    }
    Ok(value.to_owned())
}

fn required_u32(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<u32, LnsatEvidenceError> {
    required(object, key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(LnsatEvidenceError::InvalidEnvelope)
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<bool, LnsatEvidenceError> {
    required(object, key)?
        .as_bool()
        .ok_or(LnsatEvidenceError::InvalidEnvelope)
}

fn as_object(value: &Value) -> Result<&serde_json::Map<String, Value>, LnsatEvidenceError> {
    value.as_object().ok_or(LnsatEvidenceError::InvalidEnvelope)
}

fn exact_keys(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
) -> Result<(), LnsatEvidenceError> {
    if object.len() != expected.len() || object.keys().any(|key| !expected.contains(&key.as_str()))
    {
        Err(LnsatEvidenceError::UnknownField)
    } else {
        Ok(())
    }
}

fn validate_scope_value(value: &str) -> Result<(), LnsatEvidenceError> {
    if value.is_empty() || value.len() > MAX_LNSAT_EVIDENCE_STRING_BYTES {
        Err(LnsatEvidenceError::InvalidEnvelope)
    } else {
        Ok(())
    }
}

fn validate_selector(value: &str, prefix: &str) -> Result<(), LnsatEvidenceError> {
    if !value.starts_with(prefix)
        || value.len() == prefix.len()
        || value
            .bytes()
            .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'%'))
    {
        Err(LnsatEvidenceError::InvalidEnvelope)
    } else {
        validate_scope_value(value)
    }
}

fn validate_native_string(value: &str) -> Result<(), LnsatEvidenceError> {
    if value.len() > MAX_LNSAT_EVIDENCE_STRING_BYTES {
        Err(LnsatEvidenceError::ValueLimit)
    } else {
        Ok(())
    }
}

fn validate_value_limits(
    value: &Value,
    depth: usize,
    count: &mut usize,
) -> Result<(), LnsatEvidenceError> {
    if depth > MAX_LNSAT_EVIDENCE_DEPTH {
        return Err(LnsatEvidenceError::DepthLimit);
    }
    *count += 1;
    if *count > MAX_LNSAT_EVIDENCE_VALUES {
        return Err(LnsatEvidenceError::ValueLimit);
    }
    match value {
        Value::String(value) if value.len() > MAX_LNSAT_EVIDENCE_STRING_BYTES => {
            Err(LnsatEvidenceError::ValueLimit)
        }
        Value::Array(values) => {
            if values.len() > MAX_LNSAT_EVIDENCE_ITEMS {
                return Err(LnsatEvidenceError::CollectionLimit);
            }
            for value in values {
                validate_value_limits(value, depth + 1, count)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > MAX_LNSAT_EVIDENCE_FIELDS {
                return Err(LnsatEvidenceError::CollectionLimit);
            }
            for (key, value) in values {
                if key.len() > MAX_LNSAT_EVIDENCE_STRING_BYTES {
                    return Err(LnsatEvidenceError::ValueLimit);
                }
                validate_value_limits(value, depth + 1, count)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn reject_duplicate_fields(bytes: &[u8]) -> Result<(), LnsatEvidenceError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    deserializer
        .deserialize_any(DuplicateFieldVisitor)
        .map_err(|error| {
            if error.to_string().starts_with("duplicate field") {
                LnsatEvidenceError::DuplicateField
            } else {
                LnsatEvidenceError::InvalidJson
            }
        })?;
    deserializer
        .end()
        .map_err(|_| LnsatEvidenceError::InvalidJson)
}

struct DuplicateFieldVisitor;

impl<'de> Visitor<'de> for DuplicateFieldVisitor {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_bool<E>(self, _: bool) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_str<E>(self, _: &str) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_borrowed_str<E>(self, _: &'de str) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_string<E>(self, _: String) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_none<E>(self) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E>
    where
        E: de::Error,
    {
        Ok(())
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        while seq.next_element_seed(DuplicateSeed)?.is_some() {}
        Ok(())
    }
    fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate field"));
            }
            map.next_value_seed(DuplicateSeed)?;
        }
        Ok(())
    }
}

struct DuplicateSeed;
impl<'de> DeserializeSeed<'de> for DuplicateSeed {
    type Value = ();
    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateFieldVisitor)
    }
}

const AUTHORIZATION_FIELDS: &[&str] = &[
    "authorization_id",
    "audit_binding_id",
    "project_ref",
    "resource_ref",
    "authorization_attempt_id",
    "nonce_id",
    "binding_digest",
    "approval_decision_id",
    "policy_decision_id",
    "packet_id",
    "packet_sha256",
    "requester_ref",
    "requester_session_ref",
    "approver_ref",
    "approver_session_ref",
    "action_digest",
    "target_digest",
    "configuration_digest",
    "adapter_ref",
    "executable_digest",
    "audience",
    "authorization_profile",
    "issued_at",
    "expires_at",
    "state_event_id",
    "state_audit_binding_id",
    "state_sequence",
    "state",
    "state_effective_at",
    "active",
    "operation_id",
    "operation_audit_binding_id",
    "operation_idempotency_key",
    "operation_request_digest",
];
const OPERATION_FIELDS: &[&str] = &[
    "operation_id",
    "operation_audit_binding_id",
    "authorization_id",
    "consumption_id",
    "project_ref",
    "resource_ref",
    "state_event_id",
    "state_audit_binding_id",
    "state_sequence",
    "state",
    "state_effective_at",
    "attempt",
    "receipt",
    "reconciliation",
];
const ATTEMPT_FIELDS: &[&str] = &[
    "operation_attempt_id",
    "audit_binding_id",
    "operation_id",
    "project_ref",
    "resource_ref",
    "attempt_sequence",
    "adapter_ref",
    "protocol_version",
    "tool_arguments_digest",
    "created_at",
    "state_event_id",
    "state_audit_binding_id",
    "state_sequence",
    "state",
    "state_effective_at",
];
