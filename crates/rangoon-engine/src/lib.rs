//! Inert application-owned boundary for a future LNSAT adapter.
//! This crate does not discover, connect to, or operate an engine.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const STATUS_SCHEMA_VERSION: &str = "rangoon.engine-status.v0";
pub const UNAVAILABLE_SCHEMA_VERSION: &str = "rangoon.engine-unavailable.v0";

#[derive(Clone, Copy, Debug, Default)]
pub struct LnsatPlaceholder;

/// Rangoon's future-engine boundary. A1 offers diagnostics only.
pub trait GovernancePort {
    fn status(&self) -> EngineStatus;

    fn invoke(&self, operation: EngineOperation) -> UnavailableDiagnostic;
}

impl GovernancePort for LnsatPlaceholder {
    fn status(&self) -> EngineStatus {
        EngineStatus {
            schema_version: STATUS_SCHEMA_VERSION,
            provider: Provider::Lnsat,
            adapter_state: AdapterState::Placeholder,
            connection_state: ConnectionState::NotAttempted,
            runtime_state: RuntimeState::NotChecked,
            installed_version: None,
            observed_contract: None,
            execution_authority: ExecutionAuthority::None,
            network_attempted: false,
            reference: EngineReference {
                repository: "https://github.com/hypler-dev/LNSAT",
                source_commit: "e09a6b02634b04a46f861ed8b092acc2c2e50fe8",
                product_version: "0.1.0",
                wire_contract: "lnsat.contracts.v1_0",
                default_product_surface: "lnsat.product_surface.v1",
                configuration_product_surface: "lnsat.product_surface.v2",
                release_qualified: false,
            },
            capabilities: EngineOperation::ALL
                .iter()
                .copied()
                .map(Capability::unavailable)
                .collect(),
        }
    }

    fn invoke(&self, operation: EngineOperation) -> UnavailableDiagnostic {
        UnavailableDiagnostic {
            schema_version: UNAVAILABLE_SCHEMA_VERSION,
            provider: Provider::Lnsat,
            operation,
            outcome: UnavailableOutcome::Unavailable,
            reason: UnavailableReason::AdapterNotImplemented,
            execution_authorized: false,
            mutation_authority: false,
            side_effects: Vec::new(),
            retry_advice: RetryAdvice::DoNotRetryAutomatically,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineOperation {
    NegotiateContract,
    InspectConfiguration,
    ReadEvidence,
    SubmitOperation,
    ReconcileOperation,
}

impl EngineOperation {
    pub const ALL: [Self; 5] = [
        Self::NegotiateContract,
        Self::InspectConfiguration,
        Self::ReadEvidence,
        Self::SubmitOperation,
        Self::ReconcileOperation,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub schema_version: &'static str,
    pub provider: Provider,
    pub adapter_state: AdapterState,
    pub connection_state: ConnectionState,
    pub runtime_state: RuntimeState,
    pub installed_version: Option<&'static str>,
    pub observed_contract: Option<&'static str>,
    pub execution_authority: ExecutionAuthority,
    pub network_attempted: bool,
    pub reference: EngineReference,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineReference {
    pub repository: &'static str,
    pub source_commit: &'static str,
    pub product_version: &'static str,
    pub wire_contract: &'static str,
    pub default_product_surface: &'static str,
    pub configuration_product_surface: &'static str,
    pub release_qualified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub operation: EngineOperation,
    pub state: CapabilityState,
    pub reason: UnavailableReason,
}

impl Capability {
    fn unavailable(operation: EngineOperation) -> Self {
        Self {
            operation,
            state: CapabilityState::Unavailable,
            reason: UnavailableReason::AdapterNotImplemented,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnavailableDiagnostic {
    pub schema_version: &'static str,
    pub provider: Provider,
    pub operation: EngineOperation,
    pub outcome: UnavailableOutcome,
    pub reason: UnavailableReason,
    pub execution_authorized: bool,
    pub mutation_authority: bool,
    pub side_effects: Vec<&'static str>,
    pub retry_advice: RetryAdvice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Lnsat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterState {
    Placeholder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    NotAttempted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    NotChecked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAuthority {
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableOutcome {
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    AdapterNotImplemented,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryAdvice {
    DoNotRetryAutomatically,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_a_fixed_non_observation_with_all_unavailable_capabilities() {
        let status = LnsatPlaceholder.status();
        let encoded = serde_json::to_value(&status).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/contracts/engine-status.json"
        ))
        .unwrap();
        assert_eq!(encoded, fixture);
        assert_eq!(encoded["schemaVersion"], STATUS_SCHEMA_VERSION);
        assert_eq!(encoded["provider"], "lnsat");
        assert_eq!(encoded["adapterState"], "placeholder");
        assert_eq!(encoded["connectionState"], "not_attempted");
        assert_eq!(encoded["runtimeState"], "not_checked");
        assert!(encoded["installedVersion"].is_null());
        assert!(encoded["observedContract"].is_null());
        assert_eq!(encoded["executionAuthority"], "none");
        assert_eq!(encoded["networkAttempted"], false);
        assert_eq!(encoded["reference"]["releaseQualified"], false);
        assert_eq!(encoded["capabilities"].as_array().unwrap().len(), 5);
        for capability in encoded["capabilities"].as_array().unwrap() {
            assert_eq!(capability["state"], "unavailable");
            assert_eq!(capability["reason"], "adapter_not_implemented");
        }
        assert!(encoded.get("executionAuthorized").is_none());
        assert!(encoded.get("mutationAuthority").is_none());
    }

    #[test]
    fn every_closed_operation_returns_an_inert_unavailable_diagnostic() {
        let port = LnsatPlaceholder;
        for operation in EngineOperation::ALL {
            let encoded = serde_json::to_value(port.invoke(operation)).unwrap();
            assert_eq!(encoded["schemaVersion"], UNAVAILABLE_SCHEMA_VERSION);
            assert_eq!(encoded["provider"], "lnsat");
            assert_eq!(
                encoded["operation"],
                serde_json::to_value(operation).unwrap()
            );
            assert_eq!(encoded["outcome"], "unavailable");
            assert_eq!(encoded["reason"], "adapter_not_implemented");
            assert_eq!(encoded["executionAuthorized"], false);
            assert_eq!(encoded["mutationAuthority"], false);
            assert_eq!(encoded["sideEffects"], serde_json::json!([]));
            assert_eq!(encoded["retryAdvice"], "do_not_retry_automatically");
        }
    }

    #[test]
    fn operation_enum_rejects_unrecognized_or_request_bearing_input() {
        assert!(serde_json::from_str::<EngineOperation>("\"activate\"").is_err());
        assert!(
            serde_json::from_str::<EngineOperation>(
                "{\"operation\":\"submit_operation\",\"request\":{}}"
            )
            .is_err()
        );
    }
}
