<!-- intent-driven-delivery:spec:v1 -->
# Specification: LNSAT integration boundary (A1)

Status: accepted
Intent: [intent.md](intent.md), engine architecture continuation
Owner: Jeff; primary controller owns architecture, integration and release judgment
Last updated: 2026-10-05

## Behavior

Keep Rangoon useful while LNSAT approaches a supported release. Provide an explicit, replaceable boundary for future engine functions now. A1 implements an inert Rust port, native integration-status command, CLI diagnostics and an honest integration page. It does not implement a transport, LNSAT protocol client, discovery, installation, daemon start, credentials, approvals, execution or automatic retry.

The application port is **Rangoon's experimental v0 contract**, not a replica of LNSAT's wire API. Future typed requests and supported result families require a reviewed revision of this contract. No success variant or constructor for an executable grant exists in A1. An unavailable diagnostic is not an engine decision, receipt or audit event.

## Verified reference and version boundaries

Reference: public `hypler-dev/LNSAT` commit `e09a6b02634b04a46f861ed8b092acc2c2e50fe8`, inspected on October 5. GitHub returned no releases at inspection. The local canonical checkout points to a private archive and was not treated as public main. Source was read at this immutable public commit; neither LNSAT checkout was changed or run.

| Dimension | Reference value | Meaning |
| --- | --- | --- |
| LNSAT product/source version | `0.1.0` | Pre-release source; no supported artifact established |
| Gateway wire contract | `lnsat.contracts.v1_0` | Exact header value, not product 1.0 or a semantic-version range |
| Default product surface | `lnsat.product_surface.v1` | Feature projection, separate from wire version |
| Configuration diagnostic surface | `lnsat.product_surface.v2` | Explicit opt-in only; no fallback |
| LNSAT SQLite schema | `17` | Engine-owned storage, never opened by Rangoon |
| Rangoon port schema | `rangoon.engine-status.v0` / `rangoon.engine-unavailable.v0` | Application diagnostics only |

The pinned reference is build-time research evidence, never an observed installed engine. `releaseQualified` remains false. Compatibility, authentication, permission, approval, consumption and consequence evidence are distinct checks; none follows from a matching version.

## Interfaces and contracts

`crates/rangoon-engine` contains `GovernancePort`, `LnsatPlaceholder`, `EngineOperation`, `EngineStatus` and `UnavailableDiagnostic`. It depends only on serialization; it has no filesystem, process, clock, HTTP, database, environment or credential API. Operations are a closed enum:

| Operation value | Future responsibility | A1 result |
| --- | --- | --- |
| `negotiate_contract` | Inspect exact contract and feature surface through a qualified transport | Unavailable |
| `inspect_configuration` | Read explicitly selected, diagnostic-only effective configuration | Unavailable |
| `read_evidence` | Retrieve an exact authorized approval/audit evidence record | Unavailable |
| `submit_operation` | Submit an exact governed operation through an accepted engine path | Unavailable |
| `reconcile_operation` | Resolve a prior unknown effect through provider evidence | Unavailable |

`GovernancePort::status()` returns `EngineStatus`; `GovernancePort::invoke(EngineOperation)` returns `UnavailableDiagnostic`. There are deliberately no raw paths, URLs, commands, source bodies, secrets, claim-bearing booleans or arbitrary request bodies to this placeholder. A future adapter must add operation-specific validated request types and distinct result families; it cannot simply change `unavailable` to `success`.

Status fields use camelCase JSON: `schemaVersion`, `provider`, `adapterState`, `connectionState`, `runtimeState`, `installedVersion`, `observedContract`, `executionAuthority`, `networkAttempted`, `reference`, `capabilities`. Fixed values are `lnsat`, `placeholder`, `not_attempted`, `not_checked`, null installed/observed versions, `none`, and false. `reference` contains repository, sourceCommit, productVersion, wireContract, defaultProductSurface, configurationProductSurface and `releaseQualified:false`. Each capability has operation, state `unavailable`, reason `adapter_not_implemented`.

Unavailable output contains schemaVersion, provider, operation, outcome `unavailable`, reason `adapter_not_implemented`, `executionAuthorized:false`, `mutationAuthority:false`, `sideEffects:[]`, and retryAdvice `do_not_retry_automatically`. These fixed fields describe Rangoon's no-op result, not a remote system's state.

## States and failure handling

- `get_engine_status` is a read-only, no-argument Tauri command allowed only to the existing local main window. It calls the same port as the CLI. It cannot discover or connect to an engine. Existing CSP, navigation, storage and five source commands retain their boundaries.
- `rangoon engine status` prints the status JSON to stdout and exits zero because a local diagnostic was produced. `rangoon engine check --operation OPERATION` prints an unavailable diagnostic to stdout and exits **4**, indicating the requested capability is unavailable. Invalid/extra arguments exit 2 with a fixed, non-echoing CLI usage error. Engine commands never read stdin. No generic command runner is introduced.
- The integration page shows the native result separately from its pinned source reference. In a normal browser it says the desktop bridge is unavailable, with installation/runtime not checked; it does not fabricate a host status. Module load failure leaves a readable static explanation and return link.
- Loading, bridge failure and malformed response leave engine functions unavailable. Refresh repeats only the local status command. Unknown schema, unexpected fields that affect availability, unrecognized states, missing fields or a claimed success are not accepted as a usable engine. A late response cannot replace a newer refresh result. The page never enables execution controls.
- Source analysis and saved sources remain usable regardless of engine status. Dark/light, visible focus, keyboard links, narrow layout and accessible status text follow the existing workbench.

## Data, privacy, and permissions

No engine attempt occurs, so no effect can be awaiting reconciliation in A1. A future unknown result must remain unresolved until verified readback; do not retry a write merely because its response was lost. Session tokens/proofs, original instruction text, local paths and engine-database access do not belong in this status contract. Do not turn application telemetry into authority evidence.

## Acceptance mapping

1. Rust tests cover all five unavailable operations and exact serialized schema/absence of authorization, guessed runtime or fabricated engine response.
2. CLI process tests cover status, unavailable exit 4, invalid/extra arguments and fixed errors without echoing input; existing byte-preserving analysis continues to pass.
3. UI tests cover native status, absent bridge, failure, malformed/future/claimed-ready status and stale refreshes. Browser QA covers dark/light, narrow layout and keyboard recovery. Native QA verifies the real status command separately from browser fixtures.
4. Formatting, core/native Clippy, core/native tests and build, Node syntax/tests, artifact/link checks and exact-head three-OS CI pass. Fresh independent review examines the diff and evidence.
5. The architecture and adoption matrix cite immutable LNSAT source and distinguish current implementation from future work. No LNSAT write/run, secret, engine launch, network transport, installer, release or deployment is part of A1.

## Compatibility and migration

Revert the A1 commit to remove this inert port/page/command without a data migration; R2a saved sources are untouched. Start real adapter work only with the evidence gates in [application-architecture.md](application-architecture.md). When LNSAT 1.0 arrives, assess exact supported artifacts and interfaces rather than switching a readiness constant. A product-version change alone must never enable an operation.

## Non-goals and open questions

No actual transport, daemon discovery/start, credentials, authority request body, policy activation, engine mutation or execution. The supported LNSAT 1.0 artifact, transport and operation-specific request/result contracts remain open qualification work. This packet does not choose a license or publish an installer. Validation and review results are recorded in [development.md](development.md).
