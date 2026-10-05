# Claims, current evidence, and the work required

Supporting audit, October 4, 2026. [intent.md](intent.md) governs this packet. This is a product/architecture readiness review with targeted source and test inspection, not a full security audit, runtime qualification, or line-by-line verification. No LNSAT tests were rerun in this task.

## Assessment

Rangoon has a detailed vision, coherent visual references, a public marketing site, and an application repository that began this packet with one README. LNSAT has a substantial pre-release source foundation in authority contracts, canonicalization, approval and capability binding, SQLite durability, bounded Git operations, and readback. These are useful foundations, but they do not establish a released product, general sandbox, native desktop support, or working integration catalog.

The defensible ambition is strong: portable capability management with explicit consequence authorization and verifiable evidence. Making that real requires a Rangoon-owned capability model/import/compiler/workspace product and a qualified integration with an independently released authority engine. The UI should reference and verify engine evidence rather than authorizing itself or minting a competing receipt format with the same name.

## Snapshot and coverage

Primary stable source: local LNSAT `6741676496241a3cddf146d7dc28aa6c1a09860b`, branch `codex/canonical-docs-checkpoint-20261001`, tracked tree clean. Supplemental source is pinned to `eebfa485a9b4c45199da4a3243c36f2cded9d20d` (abbreviated **S** below) from `/private/tmp/lnsat-ipv6-target-validation-20261004`. Every citation prefixed **S** refers to that immutable commit, not the checkout HEAD. The checkout later advanced to `c82f18fc0c165e3f1e97fc87bb9e80920e26720f`; that later source was not re-audited. Neither the older `8e50dac8843465306a40d385bc25d4957e0b9041` documentation receipt nor this audit establishes a release or runtime qualification. Reproduce a citation with `git show eebfa485a9b4c45199da4a3243c36f2cded9d20d:<path>`.

Coverage at the pinned snapshots: required control docs, status/roadmap/build-sequence, Rust/TypeScript architecture, open-core ADR, distribution proposals, daemon/readback contracts, source/test pairs for capability claim/replay/reconciliation, fake runtime dispatch, native fdinfo parsing, configuration composition, and console behavior. Inventory recorded during the supplemental checkout review (not a coverage denominator): four Cargo packages, 89 Rust source files, 424 TS/TSX files, and 37 Rust test-path files. Counts are inventory, not coverage or test passes. Graphify's exact `persist_phase11_docker_runtime_result_v1` query returned no node; focused source search supplied the evidence.

Canonical documentation pointers are `docs/PROJECT_STATUS.md`, `docs/ROADMAP.md`, `docs/PRODUCT_BUILD_SEQUENCE.md`, and `docs/architecture/README.md` in LNSAT. Exact source citations below distinguish the canonical snapshot from the pinned supplement. A source file containing a test does not establish that the test passed on this host.

## Claim register

| Claim / aspiration | Current evidence | Safe wording now | Required proof / packet |
| --- | --- | --- | --- |
| Rangoon is an available application | Initial app main `4dfcb746` contained README only; this packet adds a prototype | “Application in development; interactive design preview available.” | R1–R6 actual app, installer, docs, release artifacts |
| Free open-source Rangoon | Explicit current user direction; app license not adopted | “Initial product is planned as free and open source.” | R1 license/notice/edition decision, R6 source + artifact release |
| LNSAT is an open authority core | Canonical Cargo license metadata/NOTICE and ADR-0003; four Rust crates plus TS surfaces | “Apache-2.0 pre-release execution-authorization and evidence source foundation.” | Published support artifacts and release gates; license is not operational warranty |
| Exact authorization and anti-replay | `crates/lnsat-contracts/src/execution.rs:58,245`; `crates/lnsat-store/src/phase7_git_adapter.rs:740` at canonical snapshot | “Source contracts bind exact request/configuration identities and one-time capability use.” | Real selected runtime/conformance tests; R7 integrated negative suite |
| Durable receipt/reconciliation | **S** `crates/lnsat-store/src/tests/phase8_runtime_composition.rs:826,900` covers replay, restart-to-unknown, reconcile without redispatch | “Durable source/test seams exist; runtime qualification remains separate.” | Process crash/effect ambiguity/restart fixtures against actual executor |
| Governed Docker execution | **S** `crates/lnsatd/src/lib.rs:1480,3850` separates test-only fake runtime; **S** `docs/PROJECT_STATUS.md:2084–2085` retains missing real driver/proof and UNSET_BLOCKING identities | “Docker-local workflow is not qualified for released use.” | LNSAT selected driver, pins, actual artifact and cleanup proof, then R7 |
| OS/resource enforcement | **S** `docs/PROJECT_STATUS.md:1513–1540` leaves native reader/custody/return feasibility open | “Native enforcement remains an unresolved engineering gate.” | Accepted bounded-return/custody model, feasible non-root positive, source and runtime proof |
| New fdinfo parser proves native readiness | **S** `crates/lnsatd/src/headless_native_fdinfo.rs:1` and `crates/lnsatd/src/headless_native_fdinfo_tests.rs:41` parse supplied synthetic bytes only | “Private bounded byte decoder; no procfs, descriptor, syscall, or product caller.” | Separate native reader and integration evidence; do not inflate this progress |
| Complete daemon/API | `crates/lnsatd/README.md:17,99`, canonical status `:329,476` has authenticated loopback `/v1` source interfaces | “Source-level loopback/session/approval interfaces exist; support remains gated.” | Stable published API, supported auth/bootstrapping and actual host integration |
| Rangoon can reuse LNSAT import/compiler | **S** `crates/lnsat-contracts/src/headless_config/parser.rs:50`, `crates/lnsat-contracts/src/headless_config/composition.rs:10` are declaration-only; no general capability compiler found | “Rangoon import, IR and compilation remain to be built.” | R2–R5 golden corpus, lineage, transforms and compatibility |
| Management UI is already complete | `apps/console/README.md:3`; **S** `apps/console/src/lib/control-center-live-readback.ts:150` and `apps/console/src/app/operation-readback-client.test.tsx` show real read-only evidence UI | “LNSAT has readback-oriented console code, not the Rangoon lifecycle workbench.” | R0 preview then real product flows, mutations and persistence |
| MCP/A2A/identity/policy integrations | `packages/gateway/src/workload-identity.ts:10`, `registry-supply-chain.ts:205` use injected interfaces; source/test contracts | “Versioned interoperability targets and source contracts, not universal production integration.” | Real provider adapters, version pinning, negative auth tests and evidence matrix |
| Every displayed connector works | Supplied screenshot catalog only | “Illustrative integration catalog.” | One real bounded connector at a time with credentials, scope, failure/reconcile tests |
| Cross-harness portability preserves behavior | Historical brief and technical specification; no released compiler corpus | “Research and product objective; representation and semantic preservation must be measured.” | R4 at least two qualified target subsets, third research target, adversarial constraint corpus |
| Desktop support on all OSs | User-selected target, not artifacts | “First release targets macOS, Windows, Linux.” | R1/R6 per OS/CPU install/update/restore/accessibility; execution rows separate |
| Cloud/enterprise/air-gap/mobile | Architecture proposals and documentation-only downstream repos | “Expansion roadmap.” | R8–R12 identity/tenancy/offline/update/recovery and support evidence |
| Signed immutable audit / tamper-proof | No complete release/retention/anchoring proof established by this review | “Integrity-verifiable evidence within a defined threat model, where implemented and checked.” | Key custody, verification, external anchoring/retention and tamper tests; never absolute tamper-proof |
| 98.6% compliant / 38% fewer tokens / 70% complete | Mockup values or historical proposal language | Omit; show “Not measured” or explicit synthetic fixture counts | Defined denominator/baseline, measured data, reproducible method and unknowns |
| Government-ready / certified / endorsed | No award, certification, ATO or procurement evidence found | “Designed for evidence-based evaluation; suitability requires agency-specific review.” | Selected program/mission, independent assurance and applicable process |

## Review findings and next actions

**High-priority product blocker: native resource enforcement is not a reusable completed layer.** Source/status does not establish the proposed synchronous five-second return bound, worker custody, atomic host trust, or a qualified positive. This is a feasibility/qualification gap, not a newly discovered exploitable vulnerability. Do not make desktop v0 depend on claiming it finished. Retain composition/export as useful independent functionality.

**High-priority product blocker: real Docker runtime proof is missing.** Fake-runtime routes are useful tests but cannot satisfy a real runtime or package claim. Exact identities, real adapter behavior, cleanup, unknown-outcome and recovery evidence remain separate engine work.

**P2 documentation defect: stale Rust/TypeScript architecture summary.** `docs/architecture/RUST_CORE_AND_TYPESCRIPT_CONTROL_CENTER_ARCHITECTURE.md:27` still describes readiness-only daemon behavior and denies interfaces now documented/implemented in daemon source and newer status. Correct the current-state section in a separate LNSAT docs packet. Preserve the source-only and unsupported-release limits. This review did not modify LNSAT or send work into its active chat.

**P2 evidence defect in the broader narrative: scope conflation.** A configuration parser, readback console, or protocol contract does not deliver general import, capability composition, workflow execution or supported integrations. R2–R7 explicitly supply those missing product layers. The real enforcement boundary must mediate the actual tool path; an unmanaged harness can bypass Rangoon. Show advisory-only status rather than implying universal control.

**Packaging/brand inconsistency:** the live application repository's description still says public website, despite the README correctly reserving it for app source. Prepare a metadata correction with the first reviewed application submission. No repository metadata was changed here. The website itself already discloses concept/release limitations; do not strip those disclosures because this prototype looks more finished.

## Recommended replacement positioning

> Rangoon is an open-source-oriented capability workbench under development for macOS, Windows, and Linux. It is designed to help teams inspect, refactor, test, and compile agent configuration with traceable provenance and explicit compatibility limits. LNSAT is the independent reference execution-authorization and evidence foundation. Governed execution will be enabled only for qualified integrations with published runtime evidence.

After R6, replace “under development” only for the management functions and exact supported platform rows actually released. After R7, name the specific qualified operation and environment. Keep cloud, mobile and enterprise capabilities labeled by their own maturity. This makes claims true incrementally instead of trying to make every aspiration true at once.

## Research hypotheses that can earn stronger claims

1. A provenance-preserving capability IR can make safety-relevant loss explicit across three materially different harnesses. Compare against manual copy/paste and naive text translation on a versioned corpus. Measure silent constraint loss, semantic test outcomes, human correction time, and unsupported detection.
2. Exact authorization, durable claims, and reconciliation can prevent duplicate or substituted effects under defined crash/replay/adversarial conditions. Compare with prompt-only permissions and ordinary retry logic in an isolated testbed. Report residual threats and cases the boundary cannot mediate.
3. Operators can use source-to-effect evidence to identify unsafe changes faster and with fewer mistakes. Test against current repository/configuration workflows with real participants and predetermined tasks; do not substitute a visually attractive dashboard for a usability result.

These are hypotheses, not proven novelty or expected grant awards. Prior-art analysis, rigorous test design, and real user demand determine whether they support a funding case. See [government-funding.md](government-funding.md).
