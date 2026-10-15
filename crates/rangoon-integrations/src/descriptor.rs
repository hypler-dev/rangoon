//! Owned metadata copies. No provider registration, discovery or compatibility claims.
use crate::OPERATIONS;
use rangoon_domain::byte_digest;
use serde_json::{Value, json};

pub fn fixture_profile() -> Value {
    json!({
        "instanceId": "offline-fixture",
        "providerFamily": "synthetic",
        "productTarget": "rangoon-offline-fixture",
        "productVersion": "1",
        "protocol": "rangoon.fixture.evidence.v1",
        "adapterId": "rangoon.offline-fixture",
        "adapterVersion": "1",
        "adapterDigest": byte_digest(b"rangoon.offline-fixture.adapter.v1"),
    })
}

pub fn candidate_inventory() -> Value {
    Value::Array(
        [
            "agentgateway standalone",
            "Cerbos PDP",
            "Permit authorization",
            "Permit FastMCP middleware",
            "Permit MCP Gateway",
            "Invariant local",
            "Invariant remote",
            "LNSAT",
        ]
        .into_iter()
        .map(|target| {
            json!({
                "productTarget": target,
                "state": "unavailable",
                "reason": "adapter_not_implemented",
                "observedVersion": null,
                "capabilities": OPERATIONS.map(|operation| json!({
                    "operation": operation, "state": "unavailable",
                })),
            })
        })
        .collect(),
    )
}
