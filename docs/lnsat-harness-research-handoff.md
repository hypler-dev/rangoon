# LNSAT handoff: harness research — October 18, 2026

Prepared in Rangoon for Jeff to pass to the independent LNSAT session. This document is a proposal for that session to audit, not an accepted LNSAT packet, an instruction to change LNSAT from Rangoon, or a statement that these capabilities are missing there. Re-read LNSAT's current branch, HEAD, status and canonical controls before assessing its implementation. Its active work may be ahead of Rangoon's pinned offline decoder.

## Video and relevance

Turing Post TV's [28:56 video](https://www.youtube.com/watch?v=pEHfZUrahGc), October 17, was retrieved as English auto-captions. Its central distinction is that better model behavior can reduce orchestration scaffolding while execution, authority, recovery and evidence remain necessary. The full Rangoon [claim audit and sources](harness-research-20261018.md) distinguishes vendor reports, benchmark findings and unverified talk claims. Captions misrecognize names; use primary sources.

For LNSAT, concentrate on the consequence boundary. Model capability, a successful evaluation, a human-facing approval click and valid credentials are each insufficient proof that a particular external effect was authorized and occurred.

## Audit questions and acceptance targets for LNSAT's owner

1. **Bind the actual effect.** Bind actor/delegation, resource, tool/runtime artifact identity, arguments, policy version, approval scope, expiry and attempt identity to what the executor consumes. Mutating any consequential input must reject or require a fresh decision. Include mutable external references and TOCTOU at the actual executor, not just manifest comparison.
2. **Keep resumed work bounded.** Waiting days does not keep a grant valid. Resume must check current policy, approval/identity validity, revocation and resource state. Keep task checkpoint identity distinct from authorization and consumption. Duplicate/out-of-order wakeups must not duplicate effects.
3. **Qualify uncertain outcomes.** Crash after effect but before receipt, lost response, disconnected executor and late cancellation must preserve unknown outcomes. Reconcile through independently identified evidence; do not auto-retry a non-idempotent action or claim exactly-once distributed effects.
4. **Govern generated capabilities.** Treat new tools, plugin versions, runtime images and environment changes as new identities with bounded scope. A model may propose a tool; it cannot expand its own grants. Changing the artifact invalidates a grant tied to the previous artifact. Test removal during active work and revocation while disconnected.
5. **Enforce cumulative constraints.** Assess repeated small actions, resource sensitivity, destination rules and aggregate budgets, not only per-call syntax. A scoped credential proxy reduces exposure but does not by itself prove that an API operation or redirected request is allowed.
6. **Measure control correctness separately from agent quality.** Build an adversarial corpus for wrong actor/resource/arguments, stale approval, replay, bypass path, tampered evidence, crash/restart and concurrent consumption. Report false allows, false denies, unknown-outcome handling, durable recovery and p50/p95/p99 authorization overhead under a stated load. An LLM judge may assist triage but cannot satisfy deterministic authorization assertions.
7. **Expose inspectable evidence without becoming Rangoon.** Version read-only native records, preserve original meanings and provenance, bound/redact exports, and distinguish decision, approval reference, consumed authorization, attempt, receipt and reconciliation. Rangoon's inspection is not a grant. OpenShell remains an independent optional runtime/security product; neither engine depends on the other.

These are checks to map against current LNSAT evidence, not permission to implement all seven or extend public wire contracts. Choose the smallest unresolved consequence/recovery issue after the audit.

## Comparables and methods to watch

| Source | Relevant lesson | Boundary |
| --- | --- | --- |
| [Cursor efficiency](https://cursor.com/blog/improved-token-efficiency), [HarnessTax](https://harnesstax.github.io/), [Apple/EPFL MLE study](https://machinelearning.apple.com/research/harness-autonomous-ml-engineering) | Compare the same workload and version; measure overhead; remove obsolete behavioral instructions when evidence supports it | Model quality and cost studies do not justify weakening deterministic controls |
| [Norm Ai / Blackstone](https://ai.engineer/nyc/2026/talks/blackstone-norm) | Keep expert-labeled regression assets across architectural changes | Practitioner report, not a verified security benchmark |
| [OpenShell](https://docs.nvidia.com/openshell/home), [Google credentials](https://ai.google.dev/gemini-api/docs/agent-credentials) | Inspect sandbox/egress/credential boundaries and bypass coverage | Runtime isolation and secret shielding do not equal LNSAT action authorization |
| [Modal Sandboxes](https://modal.com/docs/guide/sandboxes), [LangGraph persistence](https://docs.langchain.com/oss/python/langgraph/persistence), [Arca](https://ai.engineer/nyc/2026/talks/arca) | Environment replacement, checkpointed work, external waits and resumption | Resumed tasks need current authority; snapshots are not receipts |
| [DeepSeek Harness / Cordis](https://www.deepseek.com/harness/en/) | Runtime plugin identities and mount/unmount lifecycle deserve first-class coverage | Developer preview; composability does not establish isolation or safe self-modification |
| [Phoenix](https://arize.com/docs/phoenix/), [LangSmith](https://www.langchain.com/langsmith/evaluation), [Arize Signal](https://arize.com/blog/debug-production-ai-agents-with-signal-tutorial/) | Failures become regression cases and reviewable evidence; compare changes on fixed datasets | Trace collection, model scores and trace IDs are not authenticated effect proof |

## Suggested response from the LNSAT session

Return a current-code matrix: implemented and tested; implemented but unqualified; planned; unsupported. Include exact commit/source/test evidence, trust boundary and remaining failure case for each applicable item. Recommend one bounded packet with owner, files, negative tests and validation. Preserve unrelated work and independent product boundaries. No model installation, external credentials, live operation, public contract change, merge, deployment or release is requested by this handoff.
