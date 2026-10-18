# RUSL: protocol design and implementation guide

**Public draft v0.1 · editorial revision October 18, 2026 · maintained in Rangoon.**

RUSL is Rangoon's provider-neutral semantic interchange design. It describes systems, capabilities and observations in a consistent vocabulary while retaining the meaning and provenance of each provider's native contract. Its first implemented subset is an experimental, offline JSON inspection library. Publishing this draft opens the design for review; it does not announce a stable network protocol, community standard or qualified live integration.

## Architecture

Rangoon owns capability authoring, source lineage, decomposition, composition, revisions, instruction compilation, export and inspection. These local workflows work without LNSAT, OpenShell or another provider. Each external system retains its native identity, authorization, enforcement and operational responsibilities.

```text
Rangoon workbench / future authenticated management service
                  |
       existing application/domain contracts
                  |
     RUSL semantic views and compatibility reports
                  |
       independently versioned provider adapters
           /                         \
  LNSAT native contract       OpenShell native contract
  authorization + evidence    sandbox/runtime observations

Separate execution path:
workload -> selected native enforcement boundary -> target resource
```

Rangoon adapts to an engine. An engine does not need to implement RUSL or depend on another engine. An optional future provider-side RUSL exporter could simplify readback, but native validation and authorization still belong to that provider. Sharing a diagram or a trace identifier does not establish an authorization relationship between engines.

RUSL is not a second content database, an executable workflow language or a replacement for existing I1/A1/W1/O1 contracts. The [architecture specification](universal-systems-language.md) defines the broader semantic design; the [D0a profile](universal-systems-observation-profile.md) defines the exact first document subset. Where conceptual examples differ from D0a, the closed D0a rules govern D1 inspection. A discrepancy between the specification and an implementation is a conformance defect to resolve, not permission to silently accept whichever interpretation is convenient.

## Documents and authority

| Read this | Purpose |
| --- | --- |
| [Language architecture](universal-systems-language.md) | Semantic families, native provenance, stage separation, future extensions and negotiation |
| [D0a observation profile](universal-systems-observation-profile.md) | Exact JSON shape, nullability, types, bounds and refusal behavior for the offline subset |
| [Conformance and evolution](rusl-conformance.md) | Reproducible checks, implementation limits, compatibility claims and change process |
| [Capability audit](universal-systems-audit.md) | Native support, translation, proxy, observation-only, unsupported and unassessed targets |
| [OpenShell connector](openshell-observation-connector.md) / [LNSAT connector](lnsat-evidence-connector.md) | Independent provider-specific semantics and qualification boundaries |
| [Publication acceptance](intent.md#rusl-documentation-publication--october-18-2026) / [validation receipt](development.md#rusl-public-draft-documentation--october-18-2026) | Scope accepted by Jeff and the evidence for this documentation revision |

The protocol design is public and reviewable. The accepted internal D0a snapshot and existing source packets retain their original scope. A publication commit is not acceptance of every planned profile or a declaration that another implementation interoperates.

## What is implemented

| Component | Current behavior | Relationship to RUSL |
| --- | --- | --- |
| D1 `rangoon-language` | Pure Rust inspection of supplied, bounded JSON; canonical bytes, document identity and fixed diagnostics; independent Python reference tests | Implements the closed experimental D0a subset offline; no transport or mounted UI |
| D0a-H source-reference probe | Inspects already validated O1 records for exact same-record source scope; I1 non-mapping fixture | Does not emit RUSL observations or fill missing provider identities |
| D2a OpenShell mapper | Maps pinned synthetic, already decoded typed windows into `rangoon.openshell.observation.v0.1` | Provider-specific output, not D0a serialization; no native gateway contact |
| D2b OpenShell client/host source | Unmounted read-only client and host custody exercised against synthetic TLS peers | Separate transport/host work; no qualified production Start or RUSL network binding |
| D3a LNSAT decoder | Inspects supplied native-shaped JSON for authorization, attempt, receipt and recorded reconciliation distinctions | Independent native evidence projection; those facts cannot be forced into D0a |
| Broader RUSL families, typed extensions and negotiation | Architecture and acceptance targets | No general serializer, public SDK or connected conformance claim |

## First profile: an observation, never permission

D0a recognizes two document kinds:

- `capability_descriptor`: research description of a potential operation, with support unassessed.
- `observation`: an unverified provider policy evaluation or runtime report.

Every D0a document has nine required top-level keys: `languageVersion`, `kind`, `id`, `createdAt`, `origin`, `native`, `semantic`, `extensions` and `translation`. The language token is `rangoon.rusl.v0.1`; the profile token is `rangoon.rusl.observe.v0.1`. These are profile identifiers, not the Rust package version or a product compatibility guarantee.

The [complete inert JSON examples](universal-systems-observation-profile.md#inert-examples) and [six executable canonical vectors](rusl-d0a-canonical-vectors.json) are the starting point for implementers. Each canonical vector contains input JSON, exact expected canonical JSON, its SHA-256 and a domain-separated document identity. Copy a whole vector when testing; fragments are not valid documents.

For example, the synthetic policy-report vector preserves `nativeState="allow"` and `projectedDecision="allow"` while retaining `origin.authenticity="unverified"` and `translation.authority="none"`. It does not claim that a human approved, an engine issued or consumed a grant, or a resource changed. D0a cannot carry a LNSAT authorization or receipt as an authoritative fact.

Policy evaluation, local content review, human or delegated-agent approval, engine authorization, consumption, execution attempt, reported outcome, independent effect check and reconciliation remain separate stages. Missing stages remain missing. A runtime's `completed` report is not independent verification of an external effect. Model advice and UI approval cannot impersonate engine authorization.

## Fidelity and compatibility

Every adapter must identify its provider product/version, native protocol/version, adapter revision, exact operation subset and applicable RUSL profile. Compatibility is assessed per operation and direction, not once for an entire vendor. A connector may inspect a policy report while being unable to apply policy or authorize an action.

The broader design distinguishes `native_support`, `lossless_translation`, `managed_proxy`, `observation_only`, `unsupported` and `unassessed`. These are separate from current connection/evidence states such as disconnected, stale or unverified. D0a itself permits only its narrower descriptor/observation status values; the wider labels are not extra D0a enum values.

Preserve provider-native identifiers, scope, versions and semantics. Record omitted or transformed fields. Do not silently downconvert an unknown critical field, drop a constraint or infer a broader permission. Native bytes may be retained only through a separately qualified host storage/privacy path; D0a requires them to be omitted and cannot claim round-trip preservation.

External references use exact system, environment, engine, native contract and native object identities. Matching display names, timestamps, trace strings or digests do not establish a cross-provider join. A future relation profile needs an explicit relationship basis and qualified endpoints. Original records remain intact when projections change.

## Versioning and extensions

Draft v0.1 is not a promise of backward compatibility. Pin a full repository commit and the files relevant to the selected profile, rather than consuming a mutable `main` URL as a wire contract. [Conformance guidance](rusl-conformance.md#version-and-artifact-identity) distinguishes document digests, specification snapshots and future negotiated profile identities.

D0a accepts only its exact language/profile identifiers and an empty `extensions` array. It performs no network negotiation and accepts no arbitrary optional fields. Future negotiated profiles must define typed extension identity, version, schema digest, semantic owner, privacy class and criticality. An unknown critical extension rejects the affected interpretation/action. An unknown optional extension can remain opaque only where that future profile explicitly permits it; it cannot confer authority. These future rules do not loosen D0a today.

Native protocol negotiation remains independent. Selecting a common RUSL profile does not establish compatible MCP, A2A, OpenShell or LNSAT versions, authenticated ingress or execution permission.

## Transport, volume and deployment

RUSL does not currently define an HTTP endpoint, WebSocket service, broker topic, media-type registration or network handshake. Existing adapters use their native transport contracts. Any future RUSL transport binding must specify authentication, tenant isolation, framing, maximum message/batch size, cancellation, deadlines, duplicate handling, ordering scope, cursor recovery and redaction before it is supported.

Keep the desktop out of mandatory high-volume execution paths. Workloads and enforcement engines run near their resources; Rangoon observes and manages through separately qualified interfaces. A future shared service can reuse application contracts without copying authorization semantics into a second engine.

The 128 KiB D0a limit bounds one minimized report, not a company's event volume. Scaling requires bounded queues, backpressure, explicit overflow/gap records, source-scoped partitions, cursor retention and measured capacity. No global event order or exactly-once effect may be inferred from delivery. Large media stays outside the observation envelope under separately qualified storage controls. Do not truncate an oversized report into a misleading success. Millions of requests, distributed collectors and load balancing remain design targets pending service implementation and benchmarks.

## Relationship to established standards

RUSL's role is to preserve relationships and distinctions across native systems, not replace their standards. The broad [semantic-family table](universal-systems-language.md#semantic-families) records target references; listing a standard does not declare an adapter implemented.

| Native standard family | Keep native responsibility |
| --- | --- |
| MCP, Agent Skills and MCP Apps | Tools/resources/prompts, skill packages and embedded application contracts |
| A2A, ACP and AG-UI | Agent tasks/messages, editor sessions and UI event/state lifecycles |
| OpenAPI, AsyncAPI and CloudEvents | API descriptions, asynchronous contracts and event delivery metadata |
| OpenTelemetry, Trace Context and OCSF | Telemetry conventions, correlation and security-event representations |
| OAuth/OIDC and SPIFFE | Delegated access, identity and workload credential semantics |
| W3C PROV and SLSA | Provenance concepts and independently scoped build evidence |

[MCP](https://modelcontextprotocol.io/specification/2026-07-28) and [A2A](https://a2a-protocol.org/v1.0.1/specification/) already define substantial interoperability behavior. [W3C PROV](https://www.w3.org/TR/prov-overview/) already defines provenance concepts. RUSL's proposed value is the operation-specific mapping, explicit translation loss and separation of review, authority and effects in Rangoon's workbench. That is a design differentiation target, not proof of technical novelty, exclusivity or superiority. Provider/version qualification and user outcomes must establish its value.

## Security and implementation path

The offline parser reads supplied bytes only. Future integrations require explicit destination, operation, data-class and credential-scope consent. Never resolve a source reference as a URL or filesystem path, execute imported descriptions, discover credentials implicitly or use RUSL documents as grants. Bound parsers and queues; escape displayed provider text; keep secrets out of serialized observations and fixed diagnostics.

Next steps are concrete: preserve and replay D0a conformance; qualify independent read-only ingress and encrypted custody; expose source-qualified operator views; define a dedicated authority/evidence profile without altering native enforcement; then consider specific bidirectional configuration/action adapters. Every later profile needs its own exact schema, failure behavior, vectors and independent review. Broad coverage is not a prerequisite for shipping the useful local workbench.

Contributions should identify a native contract, a user workflow and an explicit semantic gap. Follow the [change and conformance process](rusl-conformance.md#change-process). RUSL remains maintained within the Apache-2.0 Rangoon repository; publication does not claim external standards-body endorsement or independent multi-vendor adoption.
