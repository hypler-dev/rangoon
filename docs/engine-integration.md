<!-- intent-driven-delivery:spec:v1 -->
# Specification: LNSAT integration boundary (A1)

Status: accepted
Intent: [intent.md](intent.md), engine architecture continuation
Owner: Jeff; primary controller owns architecture, integration and release judgment
Last updated: 2026-10-11

## Reconciliation with optional interoperability

The October 7 [provider-independent continuation](intent.md) confirms that LNSAT is optional, not Rangoon's required backend. This frozen A1 schema and its five unavailable operations remain unchanged. Core startup, persistence, authoring, composition, compilation, export and inspection never wait for an engine. The separate [neutral I1 contract](provider-interoperability.md) is proposed metadata/evidence work, not a replacement `GovernancePort` or permission to enable transport. Provider decision, Rangoon local review, approval and execution outcome remain distinct.

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

## October 11 modular source reference

Jeff requested comparison with the latest LNSAT capabilities while refining Rangoon's modular management layer. Read-only GitHub inspection resolved public main to [`5cad4719e89395d4a0c90b7b06aacfe1f83555f5`](https://github.com/hypler-dev/LNSAT/tree/5cad4719e89395d4a0c90b7b06aacfe1f83555f5), committed October 11 at 04:43:09 UTC. The pinned [README](https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/README.md), [Project Status](https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/docs/PROJECT_STATUS.md) and [documentation index](https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/docs/DOCS_INDEX.md) supply this research update. LNSAT remains pre-release `0.1.0`, with separate `lnsat.contracts.v1_0`, product-surface v1/v2 and SQLite schema 17 identities. No LNSAT checkout was edited or run. The October 5 A1 diagnostic reference above stays unchanged in its source contract; this new research pin does not establish installed compatibility or enable it.

| Optional LNSAT responsibility | Pinned evidence and qualification | Rangoon management responsibility |
| --- | --- | --- |
| Identity and sessions | Authenticated local owner/operator source routes; explicit bounded session handling, not ambient infrastructure authority | Show exact reported identity and provenance only after a qualified adapter. No credential discovery or identity authority in the current catalog. |
| Exact proposal and policy | Versioned action/arguments/target/adapter/configuration/audience binding and deterministic policy evidence | Author capabilities and definitions locally. Future inspection must distinguish the authored revision from the provider's exact request and policy version. |
| Human approval | Authenticated distinct-human approval route and immutable terminal decision; approval does not itself consume authorization or prove execution | Present required material facts and reported decision evidence through a qualified path. A local review or UI click is not an engine approval. |
| Authorization consumption | Atomic consumption plus operation/attempt claim; private pre-supervisor source verification prerequisites | Show the exact provider-owned authorization and consumption separately. No generic Rangoon permission token or automatic dispatch. |
| Attempt and consequence | Experimental bounded disposable-Git served proof; exact attempt/receipt and `outcome_unknown` distinctions | Inspect actual admitted records when available. Do not turn animation, request acceptance or a missing response into success. |
| Recovery and reconciliation | Disconnected-response/restart/readback/reconciliation proof for the bounded Git path; no redispatch. Docker-source/fake-runtime checkpoints remain separate from unfinished real Docker proof. | Keep uncertain outcomes visible, link the exact operation/attempt, and present provider-specific recovery. No blind retries or invented rollback. |
| Headless configuration | Closed explicit configuration inspection source; private bootstrap/owner/native prerequisites have differing source/design maturity | Future configuration previews belong to an optional provider module. Authoring/export does not apply or activate configuration. |
| Evidence inspection | Exact-object authenticated approval/audit reads and exact-operation readback source; HCFG-4B snapshot and HCFG-4C watch remain proposed in the pinned index | Prefer an explicit read of a selected exact object before any future live feed. There is no qualified Rangoon evidence adapter, snapshot or watch connection today. |
| Coverage and limits | Per-packet finite limits are not cumulative quota proof; target/OS isolation and complete mediation need their own evidence | Expose mediated coverage, unsupported paths and gaps. Do not claim universal enforcement or infer complete coverage from a connector. |

These are provider responsibilities, not new Rangoon modules with executable authority. Preserve the existing I1 identity and five-operation contract; preserve the separate five unavailable A1 diagnostics. The authority catalog and comparison provide human-readable research only. All Rangoon provider operations remain unavailable, observed version/compatibility remain unknown, and Rangoon authority remains none. Exact evidence reads, approval, consumption, execution and recovery are distinct semantic slots, not interchangeable plug-ins. Local source provenance, decomposition/composition, revisions, compilation, export and inspection remain usable without LNSAT.

LNSAT's current source does not establish a supported end-to-end V1 artifact, installer, production deployment, real Docker qualification or government certification. This UI/reference update performs no LNSAT operation and opens no runtime, credential, provider-installation, release or deployment gate. The [layered experience contract](product-experience.md#layered-modular-management) defines how to keep these optional responsibilities from overwhelming ordinary authoring.

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
