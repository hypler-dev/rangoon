# RUSL conformance and evolution

Status: public draft guidance, October 18, 2026. Scope: documentation and existing offline implementations. [RUSL overview](rusl.md) explains architecture; the [closed D0a specification](universal-systems-observation-profile.md) owns document behavior. This guide does not add a network binding, SDK API, authorization profile or runtime permission.

## Version and artifact identity

Keep four identities separate:

1. **Language/profile identifier:** `rangoon.rusl.v0.1` and `rangoon.rusl.observe.v0.1` select the experimental D0a shape. They do not identify a deployed engine.
2. **Specification snapshot:** an exact Git commit and file hashes identify the draft bytes reviewed. The [internal acceptance record](intent.md#internal-d0a-profile-and-d2a-offline-adapter--october-17-2026) retains historical review pins; later editorial changes do not rewrite those pins.
3. **Document identity:** D1 computes SHA-256 over `UTF8("rangoon.rusl.observe.v0.1") || 0x00 || canonical_document_bytes`. It identifies document bytes under that rule. It is not a schema digest, signature, verified producer identity, engine action commitment or approval.
4. **Adapter qualification:** a future supported integration must separately pin provider artifact/protocol, adapter/profile, operation/direction, identity/transport, environment, limitations and test evidence. A parser result cannot supply these facts.

No stable public schema digest, registry service or version-negotiation API is established by this publication. A future stable profile must freeze its normative schema and vectors, assign an unambiguous immutable identity, and specify negotiation/migration before compatibility is advertised. Do not add an invented digest field to D0a: its object shapes remain closed.

## Existing offline implementation

The [Rust implementation](../crates/rangoon-language/src/lib.rs) exposes `inspect(&[u8]) -> Result<Inspection, Diagnostic>`. On success, `Inspection` provides `kind()`, `canonical_bytes()` and `identity_sha256()`. The caller supplies bytes and owns any file selection, transport or storage. The library performs none of those operations and has no authority to authorize or execute.

The [Python reference](../crates/rangoon-language/tests/reference_d0a.py) independently validates and serializes the same bounded subset. It is a test oracle, not a supported Python SDK. The two implementations replay shared fixtures; agreement on those fixtures is useful evidence, not exhaustive equivalence or certification.

| D0a constraint | Current bound or behavior |
| --- | --- |
| Input | At most 131,072 UTF-8 bytes for one document; no trailing content |
| Structure | Root depth 1, maximum depth 16; 256 total values; 32 fields/object; 64 entries/array |
| Scalars | Strings, booleans and null in their specified slots; no numeric fields |
| Identity tokens | 1–128 ASCII bytes from the profile grammar; exact comparison |
| Display strings | 1–4,096 decoded UTF-8 bytes; control characters rejected; no Unicode normalization |
| Time | Exact millisecond UTC spelling and valid Gregorian date; no inferred freshness/order |
| Kinds | `capability_descriptor` or `observation` with closed semantic variants |
| Extensions/native content | Empty extensions; native bytes/reference omitted; no round-trip claim |
| Trust | `authenticity=unverified`, `authority=none` |

For the closed no-number subset, D1 and the reference implement the pinned canonical-byte rules derived from [RFC 8785](https://www.rfc-editor.org/info/rfc8785/). They are not general-purpose JCS implementations. The broader action/context commitment design remains a separate unimplemented profile.

## Reproduce existing evidence

From the repository root, with the declared Rust toolchain and cached locked dependencies:

```sh
cargo test --locked --offline -p rangoon-language
python3 crates/rangoon-language/tests/reference_d0a.py
cargo test --locked --offline -p rangoon-integrations --test openshell_observation
cargo test --locked --offline -p rangoon-integrations --test lnsat_evidence
cargo test --locked --offline -p rangoon-ops source_ref_probe
cargo test --locked --offline -p rangoon-integrations d0a
node scripts/validate-review.mjs
git diff --check
```

The first two commands cover D1. D2a/D3a and the source-reference commands exercise separate provider-specific or host-source behavior; passing them does not imply that those components emit D0a. No command here contacts a provider or changes LNSAT. `--offline` requires dependencies to be available already; a missing dependency is a failed/unrun check, not evidence of conformance. The [publication receipt](development.md#rusl-public-draft-documentation--october-18-2026) records the toolchain and actual results for this revision.

| Evidence artifact | What it establishes | What it does not establish |
| --- | --- | --- |
| [Canonical vectors](rusl-d0a-canonical-vectors.json) | Six exact valid inputs, canonical output and digest expectations | General JCS equivalence, authenticated producer or compatibility with a live engine |
| [Negative vectors](rusl-d0a-negative-vectors.json) | 20 parser-addressable inputs with pinned accept/refuse results; one separate host-only input | Complete fuzz coverage or permission to treat the host-only case as parser conformance |
| [Rust tests](../crates/rangoon-language/tests/d0a.rs) / [Python tests](../crates/rangoon-language/tests/reference_d0a.py) | Fixture replay, bounds and deterministic malformed-input smoke | Coverage-guided fuzzing, formal proof or every future semantic family |
| [Host-source vectors](rusl-d0a-host-mapping-vectors.json) | Exact reference candidates/refusals from bounded existing records | An emitted D0a observation or authenticated cross-engine join |
| [OpenShell tests](../crates/rangoon-integrations/tests/openshell_observation.rs) | Synthetic typed-window projection and limitations | A captured native response, RUSL serialization or live gateway support |
| [LNSAT tests](../crates/rangoon-integrations/tests/lnsat_evidence.rs) | Pinned synthetic native-JSON evidence distinctions/refusals | Connected LNSAT operation, verified effect or RUSL authorization profile |

## Required refusal behavior

Reject an oversized, invalid UTF-8 or malformed document, duplicate names, unknown/missing fields, unsupported version/kind, invalid reference, unsupported extension or invalid semantic variant. Return a fixed `Diagnostic::code()` without echoing untrusted input. The [profile](universal-systems-observation-profile.md#bounds-and-canonical-bytes) lists the exact codes. Do not make arbitrary multi-error precedence part of a compatibility promise; the literal negative vectors pin the cases whose diagnostics are asserted.

An engine being absent leaves local Rangoon workflows usable. A failed future connection is an operation-specific disconnected/error state, not an allow/deny result. A read timeout does not prove an action failed to happen. A sequence gap or conflicting result keeps outcome partial/unknown. A source-critical field that cannot be preserved rejects the translation; it must not disappear from an apparently successful result.

## Compatibility claims

A useful claim names the exact subset and evidence, for example: “Experimental D0a offline inspection at commit X passes the six canonical vectors and 20 parser-addressable cases under toolchains Y/Z.” Never shorten this to “RUSL certified,” “universal support,” or “LNSAT-compatible enforcement.”

Before advertising a stable profile, require a reviewed closed schema, immutable vectors, reproducible independent implementations, negative/boundary tests, exact compatibility and migration behavior, and documented security/privacy limits. Before advertising a connected adapter, additionally qualify the real provider/version, ingress identity, consent, bounded transport, custody, cancellation, drift and per-operation semantics. A consequence-capable route also needs native execution-time checks and demonstrated bypass limits. Parser, transport, application and enforcement qualifications are separate.

An internal accepted implementation, a public draft, a stable specification and multi-party adoption are different milestones. GitHub publication establishes availability of the design and source; it does not establish the later milestones. No third-party conformance program is operated by this draft.

## Change process

Rangoon maintainers own the current draft. Submit proposed changes through repository issues and pull requests with:

1. The user workflow and exact native provider/standard version.
2. Existing profile fields that fail to express the requirement, plus one valid and one refusing example.
3. Source-to-projection mapping, native preservation, translation loss and privacy/authority impact.
4. Versioning, old-reader behavior, retention and migration implications.
5. Required validators, independent review and evidence for any compatibility claim.

Editorial corrections may clarify draft wording without changing bytes accepted by D1. Behavioral changes need a reviewed new profile revision and updated vectors; do not silently change an accepted token's meaning. Critical semantics cannot be hidden in optional extensions. New semantic families are added through small separately testable profiles, not an unrestricted payload dictionary or executable plugin escape hatch.

Executable adapters, network bindings and management services require their own implementation packets. Protocol documentation does not authorize installation, credential use or engine execution. A future independent standards effort would need published governance, interoperable independent implementations and external participation; it is not established by naming RUSL.
