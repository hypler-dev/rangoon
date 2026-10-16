//! Offline OpenShell continuity inspection. Reported evidence grants no authority.
use serde::Serialize;
use serde_json::{Value, json};

pub const DESCRIPTOR_SCHEMA: &str = "rangoon.runtime.openshell.descriptor.v1";
pub const INSPECTION_SCHEMA: &str = "rangoon.runtime.openshell.inspection.v1";
pub const RESEARCH_VERSION: &str = "v0.1.4";
pub const OPERATION: &str = "HTTP_REQUEST";
pub const PHASE: &str = "PRE_CREDENTIALS";

/// Owned metadata, not a discovered installation or connection.
pub fn openshell_runtime_descriptor() -> Value {
    json!({
        "schemaVersion": DESCRIPTOR_SCHEMA,
        "product": "OpenShell",
        "documentedVersion": RESEARCH_VERSION,
        "observedVersion": null,
        "operation": OPERATION,
        "phase": PHASE,
        "state": "unavailable",
        "reason": "runtime_adapter_not_implemented",
        "coverage": "unqualified",
        "unqualifiedPaths": [
            "HTTP_RESPONSE/PRE_RETURN", "WEBSOCKET_MESSAGE/PRE_CREDENTIALS",
            "non_network_actions", "binary_or_uninspected_traffic", "direct_bypass"
        ],
        "authority": "none",
        "networkAttempted": false
    })
}

/// Declared binding at a proposed consequence boundary. Strings are never
/// returned in diagnostics or the inspection report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeBinding<'a> {
    pub sandbox_id: &'a str,
    pub policy_revision: &'a str,
    pub middleware_binding: &'a str,
    pub method: &'a str,
    pub destination: &'a str,
    pub body_digest: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayDecision {
    Allowed,
    Denied,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LnsatAuthorization {
    Unavailable,
    Denied,
    Granted,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    ReportedMediated,
    Uncovered,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    ReportedCurrent,
    Stale,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportedOutcome {
    NotSubmitted,
    Forwarded,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeAttempt<'a> {
    pub attempt_id: &'a str,
    pub authorization_id: &'a str,
}

/// Caller-supplied offline observation. None of these fields is authenticated.
#[derive(Clone, Debug)]
pub struct RuntimeTrace<'a> {
    pub declared_version: &'a str,
    pub operation: &'a str,
    pub phase: &'a str,
    pub expected: RuntimeBinding<'a>,
    pub observed: Option<RuntimeBinding<'a>>,
    pub gateway: GatewayDecision,
    pub lnsat: LnsatAuthorization,
    pub coverage: Coverage,
    pub freshness: Freshness,
    pub attempts: &'a [RuntimeAttempt<'a>],
    pub outcome: ReportedOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    UnsupportedVersion,
    UnsupportedOperation,
    InvalidBinding,
    InvalidAttempt,
    InputLimit,
}

impl RuntimeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedOperation => "unsupported_operation",
            Self::InvalidBinding => "invalid_binding",
            Self::InvalidAttempt => "invalid_attempt",
            Self::InputLimit => "input_limit",
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for RuntimeError {}

/// Safe projection of an unverified offline trace. No input IDs or destinations
/// are echoed, so the report can be shown without exposing request material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInspection {
    schema_version: &'static str,
    state: &'static str,
    reason: &'static str,
    authority: &'static str,
    execution: &'static str,
    authenticity: &'static str,
    network_attempted: bool,
    may_execute: bool,
    requires: [&'static str; 4],
}

impl RuntimeInspection {
    pub const fn state(&self) -> &'static str {
        self.state
    }

    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    pub const fn may_execute(&self) -> bool {
        self.may_execute
    }
}

const MISSING_PROOF: [&str; 4] = [
    "authenticated_openshell_transport",
    "lnsat_grant_verification",
    "effect_boundary_observation",
    "independent_reconciliation",
];

/// Classifies reported continuity; never validates a grant or admits execution.
pub fn inspect_openshell_trace(
    trace: &RuntimeTrace<'_>,
) -> Result<RuntimeInspection, RuntimeError> {
    if trace.declared_version != RESEARCH_VERSION {
        return Err(RuntimeError::UnsupportedVersion);
    }
    if trace.operation != OPERATION || trace.phase != PHASE {
        return Err(RuntimeError::UnsupportedOperation);
    }
    validate_binding(trace.expected)?;
    if let Some(observed) = trace.observed {
        validate_binding(observed)?;
    }
    if trace.attempts.len() > 8 {
        return Err(RuntimeError::InputLimit);
    }
    for attempt in trace.attempts {
        if !token(attempt.attempt_id, 128) || !token(attempt.authorization_id, 128) {
            return Err(RuntimeError::InvalidAttempt);
        }
    }

    let reason = if trace.coverage == Coverage::Uncovered {
        "uncovered_path"
    } else if trace.freshness == Freshness::Stale {
        "stale_evidence"
    } else if trace.coverage == Coverage::Unknown || trace.freshness == Freshness::Unknown {
        "incomplete_evidence"
    } else if trace.observed.is_none() {
        "missing_runtime_observation"
    } else if trace.observed != Some(trace.expected) {
        "changed_binding"
    } else if repeated(trace.attempts) {
        "reported_repeated_attempt"
    } else if trace.outcome == ReportedOutcome::Unknown {
        "outcome_unknown"
    } else if contradiction(trace) {
        "conflicting_evidence"
    } else if trace.gateway == GatewayDecision::Denied || trace.lnsat == LnsatAuthorization::Denied
    {
        "reported_denial"
    } else if trace.gateway == GatewayDecision::Allowed
        && trace.lnsat == LnsatAuthorization::Granted
        && trace.outcome == ReportedOutcome::Forwarded
        && trace.attempts.len() == 1
    {
        "reported_exact_path"
    } else {
        "incomplete_evidence"
    };
    Ok(RuntimeInspection {
        schema_version: INSPECTION_SCHEMA,
        state: if reason == "reported_exact_path" {
            "reported_only"
        } else {
            "incomplete"
        },
        reason,
        authority: "none",
        execution: "unavailable",
        authenticity: "unverified",
        network_attempted: false,
        may_execute: false,
        requires: MISSING_PROOF,
    })
}

fn contradiction(trace: &RuntimeTrace<'_>) -> bool {
    match trace.outcome {
        ReportedOutcome::Forwarded => {
            trace.gateway == GatewayDecision::Denied || trace.lnsat == LnsatAuthorization::Denied
        }
        ReportedOutcome::NotSubmitted => !trace.attempts.is_empty(),
        ReportedOutcome::Failed | ReportedOutcome::Unknown => false,
    }
}

fn repeated(attempts: &[RuntimeAttempt<'_>]) -> bool {
    attempts.iter().enumerate().any(|(i, attempt)| {
        attempts[..i].iter().any(|previous| {
            previous.attempt_id == attempt.attempt_id
                || previous.authorization_id == attempt.authorization_id
        })
    })
}

fn validate_binding(binding: RuntimeBinding<'_>) -> Result<(), RuntimeError> {
    if !token(binding.sandbox_id, 128)
        || !token(binding.policy_revision, 128)
        || !token(binding.middleware_binding, 128)
        || !token(binding.method, 16)
        || !printable(binding.destination, 512)
        || binding.body_digest.len() != 64
        || !binding.body_digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(RuntimeError::InvalidBinding);
    }
    Ok(())
}

fn token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.' | b'/'))
}

fn printable(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINDING: RuntimeBinding<'static> = RuntimeBinding {
        sandbox_id: "sandbox:1",
        policy_revision: "policy:1",
        middleware_binding: "middleware:1",
        method: "POST",
        destination: "https://example.invalid/actions",
        body_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    };

    fn trace() -> RuntimeTrace<'static> {
        RuntimeTrace {
            declared_version: RESEARCH_VERSION,
            operation: OPERATION,
            phase: PHASE,
            expected: BINDING,
            observed: Some(BINDING),
            gateway: GatewayDecision::Allowed,
            lnsat: LnsatAuthorization::Granted,
            coverage: Coverage::ReportedMediated,
            freshness: Freshness::ReportedCurrent,
            attempts: &[RuntimeAttempt {
                attempt_id: "attempt:1",
                authorization_id: "authorization:1",
            }],
            outcome: ReportedOutcome::Forwarded,
        }
    }

    #[test]
    fn exact_report_stays_unverified_and_inert() {
        let result = inspect_openshell_trace(&trace()).unwrap();
        assert_eq!(result.reason, "reported_exact_path");
        assert_eq!(result.authority, "none");
        assert_eq!(result.execution, "unavailable");
        assert_eq!(result.authenticity, "unverified");
        assert!(!result.may_execute);
        assert!(!result.network_attempted);
        assert_eq!(openshell_runtime_descriptor()["state"], "unavailable");
    }

    #[test]
    fn denied_cannot_look_approved_or_executed() {
        let mut input = trace();
        input.gateway = GatewayDecision::Denied;
        input.lnsat = LnsatAuthorization::Denied;
        input.attempts = &[];
        input.outcome = ReportedOutcome::NotSubmitted;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "reported_denial"
        );
        input.outcome = ReportedOutcome::Forwarded;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "conflicting_evidence"
        );
    }

    #[test]
    fn changed_request_or_context_is_incomplete() {
        for field in 0..6 {
            let mut input = trace();
            let mut changed = BINDING;
            match field {
                0 => changed.sandbox_id = "sandbox:2",
                1 => changed.policy_revision = "policy:2",
                2 => changed.middleware_binding = "middleware:2",
                3 => changed.method = "DELETE",
                4 => changed.destination = "https://example.invalid/other",
                _ => {
                    changed.body_digest =
                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                }
            }
            input.observed = Some(changed);
            assert_eq!(
                inspect_openshell_trace(&input).unwrap().reason,
                "changed_binding"
            );
        }
    }

    #[test]
    fn repeated_and_uncertain_actions_do_not_clear() {
        let mut input = trace();
        input.attempts = &[
            RuntimeAttempt {
                attempt_id: "attempt:1",
                authorization_id: "authorization:1",
            },
            RuntimeAttempt {
                attempt_id: "attempt:2",
                authorization_id: "authorization:1",
            },
        ];
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "reported_repeated_attempt"
        );
        input.attempts = &[
            RuntimeAttempt {
                attempt_id: "attempt:1",
                authorization_id: "authorization:1",
            },
            RuntimeAttempt {
                attempt_id: "attempt:1",
                authorization_id: "authorization:2",
            },
        ];
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "reported_repeated_attempt"
        );
        input.attempts = &[RuntimeAttempt {
            attempt_id: "attempt:1",
            authorization_id: "authorization:1",
        }];
        input.outcome = ReportedOutcome::Unknown;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "outcome_unknown"
        );
    }

    #[test]
    fn uncovered_stale_missing_and_absent_lnsat_remain_incomplete() {
        let mut input = trace();
        input.coverage = Coverage::Uncovered;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "uncovered_path"
        );
        input.coverage = Coverage::ReportedMediated;
        input.freshness = Freshness::Stale;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "stale_evidence"
        );
        input.freshness = Freshness::ReportedCurrent;
        input.observed = None;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "missing_runtime_observation"
        );
        input.observed = Some(BINDING);
        input.lnsat = LnsatAuthorization::Unavailable;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap().reason,
            "incomplete_evidence"
        );
    }

    #[test]
    fn malformed_and_unsupported_inputs_fail_closed() {
        let mut input = trace();
        input.declared_version = "v0.1.5";
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::UnsupportedVersion
        );
        input.declared_version = RESEARCH_VERSION;
        input.operation = "HTTP_RESPONSE";
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::UnsupportedOperation
        );
        input.operation = OPERATION;
        input.expected.body_digest = "not-a-digest";
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::InvalidBinding
        );
        input.expected = BINDING;
        input.attempts = &[RuntimeAttempt {
            attempt_id: "bad attempt",
            authorization_id: "authorization:1",
        }];
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::InvalidAttempt
        );
        input.attempts = &[RuntimeAttempt {
            attempt_id: "attempt:1",
            authorization_id: "bad authorization",
        }];
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::InvalidAttempt
        );
        const MANY: [RuntimeAttempt<'static>; 9] = [RuntimeAttempt {
            attempt_id: "attempt:1",
            authorization_id: "authorization:1",
        }; 9];
        input.attempts = &MANY;
        assert_eq!(
            inspect_openshell_trace(&input).unwrap_err(),
            RuntimeError::InputLimit
        );
    }
}
