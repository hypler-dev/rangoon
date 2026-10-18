<!-- intent-driven-delivery:spec:v1 -->
# Specification: Rangoon Universal AI Systems Language (RUSL) draft v0.1

Status: proposed
Intent: [accepted application intent](intent.md) and [independent connector direction](provider-interoperability.md#independent-engine-connectors--october-16-2026)
Owner: Jeff
Last updated: 2026-10-16

RUSL is a **versioned semantic interchange model inside Rangoon**, not a new wire protocol, executable workflow, policy language, identity provider, or authority engine. Adapters translate only the subset they can prove between provider-native contracts and RUSL. Native messages, versions, IDs, order, and provenance remain accessible. Rangoon's local authoring, compilation, export, and inspection work without any adapter. This proposal does not replace frozen I1/A1 contracts, W1 definitions, O1 observations, or provider protocols. A later accepted packet must decide whether and how to implement it.

No parser, adapter, runtime or public compatibility claim exists for this draft.

## Behavior

### Invariants

1. A common term describes meaning, never grants a capability. `provider_decision=allow`, model advice, protocol compatibility, local content review, UI approval, human authentication, engine authorization, execution attempt, and observed effect are separate typed facts. No conversion between these facts is implicit.
2. Every normalized fact points to its native source and exact provider/adapter/schema identity. External references are source-qualified tuples, never bare provider IDs. A digest proves equality of bytes under a stated algorithm; it does not authenticate their producer or prove a real-world effect.
3. A translator declares each field and operation as exact, lossless, lossy, opaque, or unavailable. A mandatory field that cannot be carried or round-tripped blocks a consequential export or managed action. A lossy view may be displayed only with visible loss and no authority inference.
4. Rangoon never upgrades a provider's advertised feature, connected endpoint, syntax-valid response, or green UI state into qualified mediation. Qualification binds operation, artifact, OS/deployment, identity, direct-path coverage, and dated tests.
5. LNSAT and OpenShell have independent descriptors, adapters, policies, authorization/evidence, and failure states. Both may appear in one operator graph through separately sourced edges; neither is prerequisite for the other or for local Rangoon work.

### Semantic families

| Family | Minimum distinctions and native references | Example source standard, not a guarantee |
| --- | --- | --- |
| Agents and tasks | Agent identity/card, declared skill, task ID, context ID, request, status, artifact, delegation and cancel/result status; distinguish authored workflow from runtime task | [A2A 1.0.1](https://a2a-protocol.org/v1.0.1/specification/) |
| Skill packages and publication | Authoring revision versus portable skill directory, `SKILL.md` frontmatter, supporting files, content digests, server-scoped published identity, activation consent and declared tool/environment requirements; published instructions remain untrusted content | [Agent Skills format](https://agentskills.io/specification), [MCP Skills extension](https://skills.extensions.modelcontextprotocol.io/specification/stable/skills) |
| Target artifact and effective load | Reviewed source revision, compiled target-specific bundle, exported bytes, installed target artifact and effective loaded configuration are distinct states with separate identities, digests, timestamps and readback sources; include declared constraints, translation loss, rollback reference and drift | Target-native harness/package format and loader, only where separately qualified; a successful export proves neither installation nor load |
| Inference and content | Model/provider/version, input/output role, text/structured/image/audio/video/binary reference, token/usage units, streaming sequence and terminal state; safety labels are advice | Provider-native API; [OpenTelemetry GenAI conventions](https://github.com/open-telemetry/semantic-conventions-genai) for observations |
| Tools and MCP | Tool/resource/prompt identity, input schema and exact call/result IDs, transport/version, MCP task extension if negotiated; tool catalog is not permission | [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28) |
| Agent exchange and UI | A2A message/part/artifact and task lifecycle separate from AG-UI run, message, tool, state, user-intent and interrupt events; coding-agent editor sessions and embedded MCP views keep their own contracts | [A2A](https://a2a-protocol.org/v1.0.1/specification/), [AG-UI 1.0](https://github.com/ag-ui-protocol/ag-ui/blob/main/docs/spec/1.0/basic/index.mdx), [ACP v1](https://agentclientprotocol.com/protocol/v1/overview), [MCP Apps](https://apps.extensions.modelcontextprotocol.io/) |
| APIs, webhooks and events | HTTP operation/security scheme/callback or webhook versus message-driven channel/operation versus event metadata; delivery/ack/replay separate from effect | [OpenAPI 3.2.1](https://spec.openapis.org/oas/v3.2.1.html), [AsyncAPI 3.1.0](https://www.asyncapi.com/docs/reference/specification/v3.1.0), [CloudEvents 1.0.2](https://github.com/cloudevents/spec/blob/ce@v1.0.2/cloudevents/spec.md) |
| Data and resources | File path/handle, object, database/catalog/table/query, vector collection/index/search, version/snapshot, sensitivity and read/write/delete effect; no generic `resource` grant | Native filesystem/DB/vector contract; OpenAPI/AsyncAPI only when actually used |
| OS and sandbox | Process/command/arguments/env, filesystem/network/device scope, sandbox/workload identity, policy revision, credential injection, actual boundary and bypass paths | [OpenShell architecture](https://docs.nvidia.com/openshell/latest/about/how-it-works) where qualified |
| Identity and credentials | Human subject, workload principal, delegated actor, issuer/audience/scope, credential reference, custody, expiry/revocation, authentication evidence; no secret value in RUSL | [OAuth BCP RFC 9700](https://datatracker.ietf.org/doc/html/rfc9700), [OIDC Core](https://openid.net/specs/openid-connect-core-1_0-errata2.html), [SPIFFE Workload API](https://spiffe.io/docs/latest/spiffe-specs/spiffe_workload_api/) |
| Policy and approval | Policy evaluation, guardrail result, human/agent approval plan and decision, engine authorization, consumption, revocation and expiry as distinct records; agency/approver duties stay provider-native | LNSAT's pinned [A1 research](engine-integration.md); provider-native semantics |
| Operations evidence | Request, evaluation, approval, authorization, attempt, reported outcome, independently checked effect, reconciliation, coverage, trace/span and measured latency/cost with units and source; security-event and build-attestation references remain separate evidence types | [Rangoon O1](operations-control-plane.md), [OpenTelemetry](https://opentelemetry.io/docs/specs/semconv/), [W3C Trace Context](https://www.w3.org/TR/trace-context/), [OCSF](https://ocsf.io/), [SLSA v1.2](https://slsa.dev/spec/v1.2/) |

### Minimal conceptual entities

`System` identifies provider product and installed instance independently of adapter. `Actor` identifies a human, agent, service, or workload with provenance and verification state. `Capability` describes what one target advertises and what the adapter actually supports. `Artifact` is an immutable authored Rangoon source/revision, compiled/exported bundle, or provider-native object reference; those artifact stages are never interchangeable. `Operation` describes one requested action and exact target. `Observation` records a source report. `Relationship` links facts with a named basis (`declared`, `reported`, `verified`) matching the existing operator-record vocabulary and never manufactures a missing fact. A `reported` relation preserves a source's correlation assertion; it is not a verified cross-system join. `TranslationReport` makes compatibility and loss visible.

These are **views over existing domain records**, not replacement database tables. A future implementation should reuse source/capability revisions, W1 workflow graphs, I1 profiles/observations, and O1 event vocabulary where their semantics already match. New fields require additive, versioned contracts. Generic names such as `approval`, `decision`, `tool`, and `run` must be qualified by origin and stage.

### Future adapter/SDK boundary

One adapter package targets one pinned product/protocol profile and declares exact operation subsets, input/output schemas, data classes, side effects, credential references, required consent, and native evidence shape. These functions describe a proposed **SDK contract**, not current callable code:

| Function | Contract |
| --- | --- |
| `describe_profile()` | Return static product/protocol/adapter identity, supported semantic families/extensions and qualified operation matrix; never probe or infer an installed engine. |
| `validate_configuration(input)` | Validate bounded local shape and show planned egress/secret scopes. No apply, install or activation. |
| `preview_translation(native_or_rusl)` | Return bounded semantic projection, field-level loss, required extensions and native provenance. No mutation. |
| `check_connection(consent)` | Optional explicit bounded endpoint/identity check with a single named credential reference; returns state, not authorization. |
| `read_evidence(exact_ref, consent)` | Optional read of one exact provider object; preserve native bytes/ID and authenticated transport evidence. No list-all or ambient discovery default. |
| `plan_consequence(exact_operation)` | Future separately accepted path only: expose prerequisites and coverage gaps; it never dispatches. Any actual submit/consume/execute port stays provider-specific and independently qualified. |

The first offline implementation would implement only the first three functions. A declarative extension is data, not code. Executable third-party adapters need a separate signed/pinned package, isolated host, least-privilege network/filesystem/secret scopes, explicit install/upgrade/revocation, and negative conformance; no provider package or SDK runs in the renderer. Do not add a generic URL/command escape hatch to make a missing adapter appear supported.

## Interfaces and contracts

### Envelope and provenance

Proposed serialized documents carry `languageVersion`, `kind`, `id`, `createdAt`, `origin`, `native`, `semantic`, `extensions`, and `translation`. These nine core keys are required in v0.1; each `kind` has a closed, separately defined required-field schema. Unknown core keys, missing required keys, duplicate JSON names and unknown critical semantic values reject the document. The only open data slot is the typed `extensions` array; an unrecognized `kind` may be preserved as opaque native data for inspection but cannot be interpreted or acted on. Before source implementation of **a specific profile**, freeze every kind included in that profile with its required/nullability rules. D0a freezes only its two inert observation kinds before D1; D0b and later gates separately freeze any consequential or other new kinds before their own source work.

`origin` contains `{systemId, product, productVersion, protocol, protocolVersion, adapterId, adapterVersion, adapterDigest, sourceRef, observedAt, receivedAt, authenticity, qualificationRef}`. A source-backed observation requires a non-null `systemId` and `sourceRef`; a research-only descriptor may leave them null, but then cannot be correlated with an operation or used as evidence. Every external `sourceRef`, actor/target reference, approval/authorization/attempt reference and relationship endpoint uses an opaque tuple `{systemId, environmentId?, engineId?, contractVersion, nativeId}`. A Rangoon-local asset uses its own namespace, schema version and immutable revision ID. Bare IDs, matching display names, timestamps or trace IDs cannot join records from different systems. A cross-system edge needs separately qualified endpoints plus an explicit relationship basis and evidence; otherwise it remains an unlinked visual suggestion. This reuses the scoped `ExternalRef`/flow-context idea in O1 rather than replacing it.

`native` contains bounded opaque bytes or an app-owned encrypted reference plus media type, length, digest algorithm/digest, redaction class, and retention policy. If native bytes are not retained, its digest may be `null`; lack of bytes is visible and forbids `lossless_translation`. `semantic` contains one typed family object. `translation` contains direction, status, field-level mapping/loss, and prerequisites. Fields without evidence are explicit `null`/`unassessed`, not guessed. No unsigned self-assertion is silently treated as verified.

Do not put secrets, raw prompts, tool arguments, file contents, database rows, vectors, or full multimodal payloads in telemetry by default. `native` retention requires explicit source-selection/consent and storage classification; the current workspace does not qualify a general encrypted native-reference store. A redacted source is represented by a digest/reference plus an explicit `unavailable_for_round_trip` flag; it cannot be called lossless. The envelope carries both event time from provider and Rangoon receipt time, plus sequence/fence/correlation identifiers when present. Time alone never orders a stream or verifies freshness.

### Operation stages and authority

An operation has `{operationId, actorRef, targetRef, actionKind, actionDigest, contextDigest, requestedEffect, policyRevisionRef?, deadline?, idempotencyKey?, traceRef?}`. `operationId` is Rangoon-local correlation identity; `actorRef`, `targetRef`, any external policy/trace ref and later provider-stage IDs use source-qualified tuples. Exact action and context commitments include the canonicalization algorithm and all authority-critical input bytes or native references; partial commitments cannot satisfy an execution gate. D0b below must freeze the binding field set and byte profile before these digests are used. Its lifecycle is a **set of separately sourced observations**, not one mutable `status`:

`intent -> provider_policy_evaluation -> local_review? -> human_or_delegated_approval? -> engine_authorization? -> consumption? -> attempt? -> provider_result? -> independent_effect_check? -> reconciliation?`

Branches can stop at any stage. A provider may lack a stage. Rangoon must label that stage `unsupported`, `unobserved`, or `not_required_by_this_provider` with reason; it must not insert a synthetic approval or receipt. Agent approver authority requires a provider-verifiable delegation chain and cannot self-expand. A UI button records local intent/review only unless a separately qualified engine route returns authenticated approval evidence. A classifier or guardrail can annotate risk or recommend denial; it cannot sign an approval, issue an engine grant, or prove execution. Consequential dispatch checks must occur at the selected native enforcement boundary, bound to exact action/context, current policy, identity, expiry/revocation and attempt. A Rangoon-proxied path is only `managed_proxy` when actual route and credential custody close direct bypass for its declared scope.

### Content, transport and event semantics

Content parts are tagged (`text`, `json`, `image`, `audio`, `video`, `binary_ref`, `file_ref`) with media type, byte size, sensitivity, encoding, ownership and digest/reference. Image/audio/video shape metadata may be retained, but transcoding, OCR, truncation, frame sampling, tokenization and redaction are declared transformations. A text summary of an image is not a lossless translation. Model and task streams retain native event ID, source sequence if available, arrival ordinal, chunk boundaries, partial/final markers, tool-call linkage, and a terminal event. Gap, duplicate, reorder, cancellation, lost terminal, or unknown replay window produce `partial`/`unknown`, never `completed`. AG-UI and A2A state are separately typed and only mapped where equivalence is demonstrated.

API descriptions remain descriptions; OpenAPI authentication schemes are not proof that a request was authorized. AsyncAPI sender/receiver direction is not inverted by inference. CloudEvents metadata is delivery context, not command authorization. Webhook signature verification, replay protection, ack and retries belong to a qualified transport profile. Files, databases, vector stores and OS actions retain native operation names, target identifiers, versions, scopes and effect classes; no all-purpose `write` or `tool.invoke` substitutes for delete, network egress, credential read, query, embedding upload or process exec.

### Extensions and version negotiation

Core identifiers are explicit strings, e.g. `rangoon.rusl.v0.1`; extension identifiers are immutable URIs or namespaced tokens plus exact schema version and content digest. An extension declares `critical` or `optional`, data class, semantic owner, encoder/decoder, and whether it affects authorization, identity, effect, ordering or privacy. Critical extensions must be recognized, validated and preserved byte-for-byte or semantically round-tripped under a documented canonicalization profile; otherwise refuse the operation. Unknown optional extensions remain opaque, source-linked and non-authoritative, with a visible loss/retention report. No unregistered extension may become a top-level grant or override a core field.

Negotiation is explicit: compare advertised language versions, required semantic families/extensions, adapter and provider protocol/product pins, then record one selected exact version/profile and schema digest. An overlapping numeric range or a product `latest` tag alone is insufficient. Major/core incompatibility rejects. Minor/additive changes can be accepted only if all critical fields and extension semantics are recognized or safely opaque for observation; a patch does not silently change semantics. Provider protocol negotiation happens independently using its own rules, such as A2A Agent Interfaces or MCP protocol versions. Cached descriptors are invalidated on product, adapter, protocol, policy, identity, configuration, or profile-digest drift. There is no silent downgrade to an older, broader, or less restrictive profile.

**Two separate wire gates:** [D0a observation profile](universal-systems-observation-profile.md) proposes an exact, inert `capability_descriptor`/`observation` subset. Its nested schemas, nullability, profile digest, canonical-byte vectors and existing-record mapping require owner acceptance before a pure D1 parser. D1 accepts no action commitment, native-host reference, authority, dispatch or effect verification. **D0b** must later freeze exact action/context field sets, bytes, algorithms, numeric/string rules, native binary commitments and cross-language vectors before any consequential profile, export or managed route. [RFC 8785 JCS](https://www.rfc-editor.org/info/rfc8785) plus domain-separated SHA-256 is a candidate, not selected or implemented. Until D0b is accepted, `actionDigest` and `contextDigest` are informational reported values only and cannot be used for authority, compatibility or consequential export. Each later `kind` needs its own accepted closed schema. Unknown core semantics always fail closed rather than being dropped.

### Translation and support labels

| Label | Exact meaning | Consequential use |
| --- | --- | --- |
| `native_support` | Provider/product documentation or tested native artifact exposes named operation; record evidence level separately | Never sufficient alone |
| `lossless_translation` | Proven bidirectional mapping for named subset, preserving all critical semantics and native round trip under pinned versions; requires immutable native bytes/reference, algorithm+digest and a round-trip test receipt | Still needs identity, consent, enforcement and runtime qualification; null digest or redacted source forbids this label |
| `managed_proxy` | Rangoon owns a **qualified** mediated path for named operation and bypass/credential limits, with external engine still owning its native decisions | Only as strong as proved path; no generic authority |
| `observation_only` | Rangoon can inspect or display a bounded report; no outbound consequence control | Never dispatch authority |
| `unsupported` | Known incompatibility or absent feature in pinned provider/adapter profile | Disable dependent action; show reason |
| `unassessed` | No adequate evidence, version pin, fixture, connection or test | Do not infer support |

`unavailable`, `disconnected`, `error`, `stale`, and `unverified` are **current operation/evidence states**, orthogonal to this translation label. A native provider may support an action while Rangoon's adapter remains unimplemented and therefore unavailable. A lossy translation can serve a clearly marked informational view but cannot be labeled lossless or used to satisfy a mandatory control.

### Inert examples

The examples below are older conceptual sketches for the **full future model**, not D0a-shaped documents, executable manifests, compatible adapter fixtures, or provider responses. The [D0a profile](universal-systems-observation-profile.md#inert-examples) has nine-key observational examples. IDs here are local example values, timestamps are illustrative, and no digest or authorization material is supplied.

```json
{
  "languageVersion": "rangoon.rusl.v0.1",
  "kind": "capability_descriptor",
  "id": "example:openshell-network-observation",
  "createdAt": "2026-10-16T00:00:00Z",
  "origin": {"systemId": null, "product": "OpenShell", "productVersion": null, "protocol": "native", "protocolVersion": null, "adapterId": null, "adapterVersion": null, "adapterDigest": null, "sourceRef": null, "observedAt": null, "receivedAt": null, "authenticity": "unverified", "qualificationRef": null},
  "native": {"retained": false, "mediaType": null, "length": null, "digestAlgorithm": null, "digest": null, "redaction": "omitted", "retention": "none", "availableForRoundTrip": false},
  "semantic": {"family": "os_sandbox", "operation": "network_egress", "scope": "selected sandbox only"},
  "translation": {"direction": "native_to_rusl", "status": "observation_only", "fieldMap": [], "loss": ["no authenticated gateway read", "direct paths unassessed"], "prerequisites": ["version pin", "native read adapter"]},
  "extensions": []
}
```

```json
{
  "languageVersion": "rangoon.rusl.v0.1",
  "kind": "observation",
  "id": "example:provider-policy-report",
  "createdAt": "2026-10-16T00:00:00Z",
  "origin": {"systemId": "fixture:system", "product": "synthetic-policy-engine", "productVersion": "fixture", "protocol": "fixture", "protocolVersion": "1", "adapterId": "fixture:adapter", "adapterVersion": "1", "adapterDigest": null, "sourceRef": {"systemId": "fixture:system", "contractVersion": "fixture.v1", "nativeId": "7"}, "observedAt": null, "receivedAt": null, "authenticity": "unverified", "qualificationRef": null},
  "semantic": {"family": "policy", "stage": "provider_policy_evaluation", "reportedDecision": "allow", "operationRef": {"systemId": "fixture:system", "contractVersion": "fixture.v1", "nativeId": "request-7"}},
  "native": {"retained": false, "mediaType": null, "length": null, "digestAlgorithm": null, "digest": null, "redaction": "omitted", "retention": "none", "availableForRoundTrip": false},
  "translation": {"direction": "native_to_rusl", "status": "observation_only", "fieldMap": ["reportedDecision"], "loss": ["no verified approval", "no authorization", "no execution outcome"], "prerequisites": ["provider identity", "signed decision evidence"]},
  "extensions": []
}
```

A hypothetical `urn:rangoon:extension:vector-search:1` with `critical=true` and a pinned schema digest would fail negotiation if the receiving adapter cannot preserve its index, metric, dimension, namespace and query filter semantics. Marking it optional could permit **display only** of the opaque native record; it could not silently turn a filtered vector query into an unrestricted search.

If an A2A task and AG-UI run share a correlation ID, Rangoon can draw a `reported_correlates_with` edge. It cannot collapse their identities or infer that either completed a provider tool action. If LNSAT and OpenShell both report an event, separate native source refs remain on separate nodes; an operator may compare them without requiring one to authorize the other.

## States and failure handling

| Case | Required result |
| --- | --- |
| No provider installed/selected | Local authoring/import/compose/compile/export/inspection usable; optional connector shows `unavailable`, no probe |
| Provider not reachable, timeout, invalid credentials | `disconnected` or `error` for exact operation; no silent fallback, retry or broad source upload |
| Unsupported version, critical extension, or lost mandatory field | Reject translation/consequential plan with exact field and version diagnostics; retain native evidence securely if consented |
| Unknown optional extension | Preserve opaque source and mark semantic view partial; never interpret as authority |
| Stale, replayed, conflicting or missing evidence | Show separate event rows and `unknown` effect/coverage; no convenient winner or automatic redispatch |
| Changed action/arguments/target/policy/identity after check | Invalidate exact binding; require new provider evaluation/authorization at execution boundary |
| Provider `allow`, model `safe`, or UI `approve` only | Display respective provenance; no engine grant or execution permission |
| Stream gap or lost terminal; webhook duplicate | Mark partial/unknown; dedupe only by native identity and qualified rules; never infer completion |
| Managed path's direct bypass unproven | Downgrade coverage to observation-only/unassessed for that path; block mandatory mediation claims |

## Data, privacy, and permissions

Only explicit user-selected assets enter local analysis. A connection check, provider evaluation or read of provider evidence is a separate consented operation with named endpoint, data classes, credential scope and egress. No ambient `.env`, keychain, token cache or network discovery. Desktop encryption/retention rules in [encrypted-workspace.md](encrypted-workspace.md) continue to govern native storage; this spec does not activate encryption, add a database table, or license plaintext export as safe. Hosted transport and tenancy require their own threat model. Untrusted descriptions, Markdown, schemas, cards, event attributes and native payloads are data, never instructions or code to execute. Bound size/depth/reference expansion; sanitize UI; no SSRF from imported OpenAPI references or provider endpoints. Credentials remain native secret references, never serialized into RUSL documents. Correlation identifiers should be scoped/pseudonymous, and high-cardinality or sensitive fields omitted from default telemetry. Audit export requires explicit selection and redaction review.

## Compatibility and migration

First implementation should be an offline library/profile and conformance fixtures, not a generic network gateway. Existing I1/A1/W1/O1 schemas remain valid and unchanged. Add adapters incrementally, each translating a named native subset into RUSL; only implement reverse translation where the provider supports it and tests prove no critical loss. A future old-document migration must preserve original bytes and provenance, report changed/lost fields, and permit rollback to old Rangoon revisions. Older clients may inspect unknown optional extensions opaquely but may not activate a consequential plan that depends on them. Do not auto-upgrade saved workflows or apply new provider policy. A qualification expires on pinned artifact/profile drift.

## Acceptance mapping

| Requirement | Proposed first proof (none run by this design packet) |
| --- | --- |
| Independent local core | Offline no-provider startup, author/compose/compile/export/inspect regression on all three desktop OSs |
| Native provenance and no authority promotion | Fixture round trips for LNSAT and OpenShell separately; provider `allow`/model advice/UI approval negative tests |
| Version/extension negotiation | Exact profile selection, unknown critical refusal, optional opaque retention, no downgrade/upgrade on drift |
| Loss accounting | Native-to-RUSL-to-native byte/semantic comparison, redaction/multimodal/stream-order negative vectors |
| Honest operations | Unavailable/disconnected/error/stale/unknown screen and CLI cases; evidence source, clock, digest and qualification labels |
| Security | Fuzz/property bounds, untrusted URI/Markdown handling, no hidden egress/credential use, direct-path coverage tests for any managed proxy |
| Review | Fresh independent architecture/security review of exact spec and fixtures before source implementation; provider-specific review before live adapter |

Proposed **staged** conformance vectors (IDs define expected behavior, not tests executed here). D1 implements only the accepted D0a observational subset. Consequential, protocol-specific and target-loader vectors require their own later profile and packet; their presence here does not expand D1:

| ID | Input mutation or condition | Expected result |
| --- | --- | --- |
| `RUSL-01` | No provider descriptor and valid local capability revision | Local review/compile/export succeeds; connector operation `unavailable` |
| `RUSL-02` | Synthetic provider policy says `allow`; no engine grant | Observation displays allow; dispatch eligibility remains false/unknown |
| `RUSL-03` | Model classifier says `safe` and UI user clicks Approve | Two advisory/local facts; no human identity proof or engine authorization |
| `RUSL-04` | Change one argument, target, actor, policy revision or canonicalization after evaluation | Exact binding invalid; no reuse of authorization/attempt handle |
| `RUSL-05` | Unknown critical extension or lost mandatory field | Reject translation and consequential export with named field |
| `RUSL-06` | Unknown optional extension | Retain opaque/redacted source ref; semantic view partial and non-authoritative |
| `RUSL-07` | A2A task reports complete but AG-UI stream lacks terminal event | Separate facts; UI stream remains partial, external effect unknown |
| `RUSL-08` | Duplicate/reordered webhook or missing stream sequence | Retain native IDs/order evidence, mark gap/conflict and no effect claim |
| `RUSL-09` | Multimodal image summarized to text or native bytes redacted | Mark lossy/non-round-trippable; no `lossless_translation` label |
| `RUSL-10` | OpenShell observation present, LNSAT absent (and reverse) | Each independent connector remains inspectable; no cross-engine prerequisite or inferred grant |
| `RUSL-11` | Provider descriptor/version/policy changes after qualified check | Invalidate affected qualification, show stale/unassessed until rechecked |
| `RUSL-12` | Proxy reports mediation while alternate direct credential/path exists | Refuse `managed_proxy` coverage for that operation and path |
| `RUSL-13` | Same native ID reported by two engines, or null `systemId` in a purported observation | Keep two separate facts or reject linkage; no cross-engine approval/authorization edge |
| `RUSL-14` | Extra unknown core field, duplicate JSON key or unrecognized critical enum | Reject core document; no silent downconversion into older profile |
| `RUSL-15` | Native bytes/ref omitted or digest null but translator advertises lossless | Reject `lossless_translation`; show opaque/lossy view only |
| `RUSL-16` | Two serializers disagree on canonical action/context bytes or digest | Fail D0b conformance and keep consequential profile unavailable; D0a/D1 observational parser remains independent |
| `RUSL-17` | Export succeeds but no target-native install/load readback exists | Keep source and exported artifact traceable; installed/loaded state remains `unassessed`, with no effective-configuration or runtime-coverage claim |
| `RUSL-18` | Target-native readback identifies a loaded revision/digest different from the exported bundle | Show explicit drift and both source identities; do not join runtime evidence to the exported revision without a separately proven relation |
| `RUSL-19` | Target reports the exported artifact digest but gives no evidence of effective layered configuration or instruction coverage | Report artifact identity as observed; keep effective configuration and runtime coverage `unassessed`. A matching digest alone cannot link later events to the reviewed instructions. |

## Non-goals and open questions

This document does not promise universal native support, invent a single authorization rule, implement any adapter or SDK, train a model, run an engine, or enable execution. Open questions for Jeff and a later implementation packet: public naming/versioning of RUSL; D0a acceptance of exact observational schemas and canonical bytes; D0b selection and freeze of the action/context JSON/binary commitment; privacy budget and retention for native payloads; whether any Rangoon-managed proxy is justified before one read-only adapter is qualified; supported MCP/A2A/AG-UI versions per target; and which exact OpenShell release/read surface can support the first goal-specific connector. The [audit sequence](universal-systems-audit.md#staged-implementation-plan-not-authorized-by-this-design-packet) prioritizes that read-only OpenShell path for the operator-record goal; the older [Cerbos candidate](provider-interoperability.md#candidate-matrix-and-provider-semantics) remains an independent lower-risk generic adapter option. Neither delays the useful local workbench release path.
