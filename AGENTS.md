# App review build rule

Every feature packet follows two linked tracks:

1. **Experience:** preserve the original eight-screen structural baseline and shared shell: grouped navigation, compact theme-aware banner, central work area, and right inspector. Route artwork may vary, but layout hierarchy, control meaning, provenance language, and unavailable states stay stable.
2. **Technical:** record the real implementation outcome, evidence, gaps, and tests for the same feature. A visual-only packet is allowed only when explicitly labeled UI-only; it never counts as technical progress. Subsequent technical work remains on the roadmap.

Each packet records both tracks in the canonical development ledger. Preserve native IDs, controls, security disclosures, and provenance. Automated checks enforce narrow contracts; they do not prove visual quality or runtime, security, release, OS, or V1 qualification.

Visual work stays stable per route: no random motion, fake readiness, simulated progress, or artwork that changes control hit targets. Validate rendered 320, 768, and 1440 pixel views in dark and light themes, with no overflow, keyboard focus, and reduced motion. Obtain fresh independent review for meaningful UI or behavior changes.

Scope authority: `docs/intent.md`, `docs/product-experience.md`, and `docs/visual-system.md`. Do not edit source, policy, release, CI, or unrelated documentation while applying this rule unless a packet explicitly names those files.

## README publication gate

Jeff requires a README update with every push or merge to `main`. Include a substantive `README.md` update in every publication packet before publishing it: describe the delivered change, current feature boundaries, and any changed roadmap or setup information. Documentation-only packets should summarize their relevant clarification. Do not add empty edits or promote planned, synthetic, source-tested, or partially qualified behavior into released capability. Keep exact validation and publication receipts in `docs/development.md`; link that evidence from the README. The controller must verify the README update is present in the reviewed publication diff and matches the final scope. A previous packet's README update does not satisfy a later publication.

## Security and standards build requirements

Authority: Jeff's October 8 requirement to consider ISO and OpenSSF during development. [The readiness track](docs/plan.md#security-and-standards-readiness) owns pinned references, candidate evidence and unresolved work; [encrypted-workspace.md](docs/encrypted-workspace.md) owns storage behavior. Do not duplicate a competing compliance status in build scripts or badges.

For every meaningful feature/build/publication packet:

1. Record applicable security and standards impact in the canonical development receipt: data handled, trust/authority boundary, storage/output confidentiality, dependencies/build privileges, applicable readiness target and unresolved gap/owner. A bounded “no new impact” explanation is allowed for unrelated changes; do not perform a full organizational audit for every source build.
2. Run the packet's named validators and fresh independent read-only review. Security identities, state, digests, consent and authority remain exact. Deterministic malformed-input/property tests supplement exact acceptance; do not label them coverage-guided fuzzing or use approximate matching for authorization.
3. Preserve encrypted-request fail-closed behavior: no plaintext fallback, key regeneration after loss, hidden user-vault/provider access, automatic migration, secret logging or renderer key exposure. Before mounting native encryption, qualify complete routing, callback drain, retained-data purge, explicit consent and durable recovery; source-only tests are not native adoption.
4. Review new/changed dependencies and build workflows proportionally: pinned identities, least privilege, untrusted-input isolation, vulnerability/license implications and secret exposure. Hosted enforcement, MFA, real human approvals and organizational processes need dated evidence; an agent review or repository file does not establish those controls.
5. Keep ISO/IEC 27001:2022 with 27002:2022 guidance, applicable ISO/IEC 42001:2023/27701:2025, OSPS Baseline v2026.08.28, Best Practices criteria, dated Scorecard results and SLSA v1.2 scope distinct. Record applicability and gaps; never assert certification, a badge/score, OSPS maturity or SLSA level without its own complete assessment. Update reference versions only through a reviewed readiness packet.
6. Before distributing executable packages, qualify the actual target backend/OS custody/GUI, fresh and legacy setup/recovery, dependency/distribution notices, an artifact-bound dependency inventory/SBOM, build provenance and the selected signing/integrity-verification path. Source publication is not package-release acceptance. Do not claim a supply-chain level from a checksum or ordinary CI success.

README updates remain required for every publication. Report current plaintext storage/export limits and unfinished capabilities honestly. Standards alignment does not widen execution authority or make LNSAT/providers mandatory. No natural-language hook, scanner score or agent approval becomes a security boundary. `npm run build:check` continues to enforce its documented syntax/vision/behavior scope; it does not validate ISO organizational controls or the full OSPS checklist. New automated checks, scanners, hosted policy, badge submissions and release attestations require separately reviewed implementation packets.

## Repository ownership and parallel work

These repositories are independent sources of truth:

| Repository | Owns |
| --- | --- |
| [VUSL](https://github.com/hypler-dev/vusl) | Normative language specifications, profiles, compatibility rules, conformance vectors and specification governance |
| [LNSAT](https://github.com/hypler-dev/LNSAT) | Native authorization and enforcement contracts, approval and authorization checks, receipts, reconciliation and engine implementation |
| [Rangoon](https://github.com/hypler-dev/rangoon) | Workbench, capability management, persistence, UI, operator records and Rangoon-owned adapters, including its current VUSL parser |

1. Before editing, record the Git root, worktree, branch, HEAD, status and fetch/push URLs. Match the repository to the table above. A folder name, `origin` label, private archive or website checkout is not proof of the intended repository. Never change an existing remote merely to make a push succeed.
2. Put source, tests, documentation and status updates in the repository that owns the change. Do not maintain another editable copy of a specification in a consumer. Consumer copies needed for offline tests must name an immutable upstream commit, path and digest; update them through a reviewed compatibility change.
3. Use one development chat per repository. Additional simultaneous writers in that repository need distinct branches/worktrees and explicit file ownership. Do not edit, reset, stash, clean or switch another chat's checkout. A coordination chat may inspect all three but does not silently become their implementation owner.
4. If work requires another repository to change, prepare a bounded handoff for its owner: outcome, owning repo, exact base commit, proposed files/contracts, compatibility impact, acceptance tests, blocked consumers and allowed external actions. Cross-repository edits require explicit task scope and ownership. The receiving chat rechecks current Git state before acting; chat history is not shared authoritative state.
5. Update the owning repository's existing status/handoff record at each meaningful checkpoint. For an isolated documentation change, the reviewed pull request may serve as that record. Record implemented versus proposed behavior, validation, independent review, unresolved issues, exact commit/PR and publication state. A local edit is not a GitHub update. A pushed branch is not a main merge or release.
6. When publication is authorized, commit and push only the reviewed owning-repository packet to its verified intended remote. Include a substantive README update when behavior, status, setup or ownership changes, plus every additional README gate required by that repository. Preserve unrelated work. Merge, deployment, provider operation and release keep their separate authorization gates; this rule is not blanket publication permission.
7. For a cross-repository contract change, agree the version and compatibility rules in the owning repository first. Consumers pin the reviewed revision and test both supported and rejected cases. Record the exact commit tuple used for integration tests. Do not claim combined support until the relevant end-to-end scenario is verified; do not require a coordinated release when unchanged compatible contracts permit independent shipping.
8. Keep authority boundaries intact. VUSL compatibility, classifier output and Rangoon UI review do not grant engine authorization. LNSAT remains independent; OpenShell is a separate optional engine. Rangoon's core local workflows must remain usable without either engine. A semantic adapter must not silently change native meanings, discard critical constraints or elevate authority.

Use a small coordination index of links, dependency pins and blockers rather than duplicating three product roadmaps. Repository records remain authoritative for their own implementation and release state. Instructions guide contributors; sandbox permissions, branch protection, CI and human review provide separate controls.
