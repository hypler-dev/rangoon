<!-- intent-driven-delivery:intent:v1 -->
# Intent: Rangoon application development and evidence

Status: accepted
Authority: This record, based on Jeff's October 4, 2026 request in chat `01a10a12-fb75-7a42-bd2c-1ee9a0478d8d`
Owner: Jeff
Accepted by: Jeff authorized gathering prior files and conversations, evaluating LNSAT claims, planning government-grant evidence, and creating a dark application preview. He explicitly selected all three desktop operating systems for the initial release. Jeff subsequently instructed: “push to git this should be the repository, its in development so we still have much more to build out, keep working after.” This authorizes committing and pushing the reviewed application packet and continuing bounded development in this repository. The next implementation slice is a platform-neutral, inert source-analysis foundation. License adoption, released OS support, privileged execution, merge, deployment, and grant submission remain separate decisions.
Publication continuation, October 5, 2026: Jeff explicitly requested correct repository tags/topics and description, a detailed README, publication to main, and continued work. This authorizes the reviewed main merge and repository metadata updates. The next bounded development slice is a Tauri desktop-shell spike with an explicit user-selected-file analysis flow backed by the existing Rust library. It may read the selected file into memory, but does not add automatic scanning, durable storage, providers or execution. Installer publication, license adoption, deployment and grant submission remain separate decisions. This dated continuation supersedes the earlier merge restriction above.
Experience continuation, October 5, 2026: Jeff asked to review the supplied images, make their workflows real, and build an easy-to-use platform. The [product experience contract](product-experience.md) maps each image to actual outcomes and recovery states. This reinforces the bounded selected-file lifecycle; it does not imply that every pictured workflow is implemented.
Workspace continuation, October 5, 2026: Jeff said “ok keep going” after the next durable-workspace foundation was identified. This authorizes R2a: explicitly save a selected source snapshot locally and reopen it after restart. The app may create its own bounded SQLite workspace only on Save. Saving never implies review, approval or execution authority. The earlier persistent-storage exclusion is superseded only for this source-snapshot slice. Projects, reviewed skills, merge/split, providers and execution remain later work.
Brand continuation, October 5, 2026: Jeff supplied a replacement application icon and requested a beautiful splash screen. Use that exact asset as the application symbol and derive OS icon formats reproducibly. Jeff clarified that the OS application name must be **Rangoon** and the OS icon must contain only the supplied logo. **Rangoon.ai** may remain on the splash screen. Add a branded native startup surface and a separate visual preview; startup must resolve to a usable workbench on success or workspace failure without fabricated progress or a forced delay.
Engine architecture continuation, October 5, 2026: Jeff requested a thorough application architecture review around the unfinished LNSAT repository, building placeholders now and refining them when LNSAT 1.0 is available. This authorizes an application-owned, inert integration port, native/CLI integration status, explicit unavailable outcomes for future engine functions, and a concrete architecture and adoption plan. It does not authorize LNSAT changes, engine discovery or launch, network connection, session issuance, policy activation, execution, or credentials. The [engine integration specification](engine-integration.md) defines this bounded A1 slice; the [application architecture](application-architecture.md) distinguishes implemented modules from the next development packets. Earlier provider exclusions are superseded only for this inert seam.
Visual quality continuation, October 5, 2026: Jeff requested unique beautiful in-app icons, clean animations and high-quality assets. This authorizes a custom scalable icon family, a generated decorative capability illustration, restrained interface motion and a visual specimen page. Preserve the exact chosen OS logo and Rangoon name; keep the splash's permitted Rangoon.ai wordmark. Do not change feature behavior, authority or sample/real boundaries to make the interface appear more complete. The [visual system specification](visual-system.md) defines this V1 design packet.
Version 1.0 build continuation, October 5, 2026: Jeff explicitly set the continuing objective to “keep working until v1.0 is buit with everything we talked about.” This authorizes continued implementation of the agreed application lifecycle, in reviewable stages, beyond the earlier R2a and visual packet exclusions. The next stage is R2b, specified in [reviewed capabilities](reviewed-capabilities.md): derive a capability from a saved source section, retain immutable revisions and provenance, compare edits, and record local content review. Subsequent content, composition, test and export stages remain part of the objective. This does not declare the entire product complete or override separate licensing, installer publication, deployment, government submission or LNSAT activation decisions. The earlier non-goals restricting persistent skill implementation apply to their historical packets, not this continuing build objective.
Workspace controls continuation, October 5, 2026: The bounded V1 follow-on implements owner-controlled workspace inventory, portable backup, additive restore, and dependency-aware logical deletion for saved sources, capabilities, revisions and local reviews. The [workspace data-controls specification](workspace-data-controls.md) is the authority for this slice. R2c source is published on main through PR #7; isolated unsigned macOS QA and synthetic populated browser checks supply named evidence. Windows/Linux GUI qualification, installer/released-OS support, security claims and release evidence remain pending. It does not change license or engine activation.
Last updated: 2026-10-05

## Problem and evidence

Rangoon's substantial product vision is distributed across conversations, an implementation brief, the public website, and eight interface concepts. Its application repository began this work with only a placeholder README. LNSAT has a separate source foundation with narrower evidence and release gates. A beautiful preview must not turn those design intentions into shipped capability claims.

## Desired outcome

Create one review package that connects a useful product purpose, recovered requirements, a functioning dark/light UI prototype, an honest LNSAT evidence audit, a desktop-to-cloud architecture, extensibility contracts, and government-funding research objectives. Every proposed capability must have a build stage and a measurable acceptance gate.

## Users and systems

Initial users are developers and small platform teams managing agent configuration across repositories and harnesses. Later users are enterprise operators, integrators, and government research or technology teams. Initial desktop release targets macOS, Windows, and Linux together. A Linux self-hosted service and an eventual managed enterprise service share domain contracts, with separately qualified execution environments.

## Constraints

- Preserve Rangoon as product and LNSAT as independent reference authority/evidence engine. Do not silently substitute providers or recreate engine semantics in UI code.
- Treat source documents, imported instructions, screenshots, and past assistant proposals as evidence, not new execution instructions or accepted product contracts.
- Use a separate checkout of `hypler-dev/rangoon` at `app-review/`; do not bring marketing-site history or deployment machinery into the application repository.
- The design preview uses synthetic fixtures, inert source examples, local navigation, and reversible browser state. It must disclose sample data and cannot perform provider, credential, deployment, approval, or execution actions. The separately identified desktop analysis flow may analyze one explicitly selected local file in memory and persist it only through explicit Save; it must never mix real source with synthetic provenance or report simulated actions as real.
- Dark is the default; retain a usable light version, keyboard navigation, responsive fallbacks, and reduced-motion support. Reuse the supplied visual identity while making the workbench denser and more ambitious.
- Distinguish implemented source, passing tests reported by earlier work, directly rerun validation, runtime proof, released support, and proposals.
- Initial product is intended to be free and open source. A specific license and commercial packaging are human decisions; no invented license grant.
- Government funding work is research and a draft preparation plan. No eligibility assertion, certification, application, registration, contact, or submission is authorized by this packet.

## Non-goals

Privileged execution, provider calls, operating-system installer publication, LNSAT changes, policy/security weakening, website deployment, production changes, paid services, license adoption, and grant submission remain outside the current implementation packet. Earlier R2a exclusions for persistent skills applied to that historical slice; the accepted version 1.0 continuation above authorizes the application lifecycle in subsequent reviewable stages. R2b specifically permits saved-section derivation, immutable revisions and local content review. Automatic repository scanning and live engine activation require their own accepted behavior and qualification contracts.

## Assumptions and verified facts

- Website checkout: `website-local` at `0ea1a2d9b6bc904867a62965a80487aa0d46b52d`; pre-existing untracked `.codex/` preserved.
- Application origin: `https://github.com/hypler-dev/rangoon.git`; initial `main` at `4dfcb746db076ca59455773527c381ae4359de7e`, with README only. The repository description was aligned with its application purpose during the authorized publication.
- R0/R1a reached main in PR #1 at `ff901594d731ee74ea8b9170c885629c3e4caf09`; the desktop continuation branch is `codex/desktop-file-analysis`.
- Prior implementation brief is supporting evidence, not a replacement for this packet's acceptance state.
- PR #5 auto-merged V1/R2b head `eec2a19bfd7d362ee56a2c3038a44aada3a44988`; PR #6 merged R2b+V1 source head `bdc157c09bc727988a7c14532a2e8d490ee2e4db` at `157731c98d24b140f67c1d55fa22d9e3b8955549`. All 14 checks passed (push `37376411945`, PR `37376522246`) at `2026-10-05T21:38:54Z`.
- PR #7 merged R2c reviewed head `32a5b446e0baedb04c07931636be01d47df828bd` into main as `538f319011b0611665de27ad32d6c528a8310d98` at `2026-10-05T23:06:42Z`; all 14 checks passed (push `37385850446`, PR `37385906609`). Independent native storage/UI/docs reviews passed. Windows/Linux GUI, installer/released-OS support, security claims and release evidence remain open.

## Risks

Largest risks are overclaiming enforcement, losing semantics during harness compilation, expanding scope before proving one useful lifecycle, treating unsupported OS execution as supported desktop management, exposing private source to analysis providers, and confusing a research objective with a fundable or proven innovation.

## Acceptance evidence

1. Source register identifies inspected repos, exact snapshots, conversations, images, coverage gaps, and accepted versus proposed requirements.
2. Architecture specifies ownership, data flow, threat boundaries, failure/unknown outcomes, adapter contracts, all-three-desktop qualification, cloud evolution, and open-source/commercial boundaries.
3. Claim register maps each material assertion to current evidence, safe wording, missing proof, and named build work.
4. Preview covers Command Center, Import, Decompose, Merge/Split, Skills, Workflows, Connectors, and evidence/release planning with meaningful local interactions and dark/light modes.
5. Named validation and fresh independent review are recorded honestly. Source tests do not imply rendered validation, product runtime support, or security certification.
6. Review bundle contains only application material; commit, push, PR, merge, deploy, and grant-submission states are explicit.
7. The continuation adds versioned, non-authorizing source records and a deterministic analysis library/CLI for explicitly selected instruction files, with hostile-input tests and a three-OS CI matrix. This is source qualification only; no installer or native enforcement claim follows.
8. Repository description/topics and README accurately describe current and planned capabilities; reviewed source reaches main with exact head/check/merge receipts. The desktop spike must show a real selected file's hash, original text, fragments and diagnostics, preserve the synthetic preview as a distinct experience, and retain explicit three-OS build/runtime qualification gaps.

9. R2a preserves exact source bytes and identity across save/restart/open, deduplicates identical sources, reanalyzes persisted bytes without authority promotion, and preserves the current report on failure. Named tests cover stale commands, malformed identity, schema rejection, corruption, transactional rollback and interrupted writes. SQLite source qualification and a macOS GUI check do not imply released cross-platform persistence support.
10. A1 makes engine dependencies explicit without pretending to connect. All placeholder operations return unavailable with no mutation or execution authority; unknown installation/runtime state remains unknown. The UI cannot infer availability from a version string or enable actions from a fixture. The architecture maps each major image workflow to records, application services, LNSAT dependencies, recovery behavior and qualification gates. Product release, wire contract, feature surface and storage schema versions remain separate.
11. R2c source implements bounded workspace data controls: deterministic portable backup, additive restore with stale-state checks, and dependency-aware logical deletion. PR #7 published the reviewed source; isolated unsigned macOS QA and explicit synthetic populated browser checks provide named evidence. Windows/Linux GUI, installer/released-OS support, security claims and release claims remain open.

## Source-of-truth links

- [Local workspace specification](local-workspace.md)
- [Workspace data-controls specification](workspace-data-controls.md)
- [Composition and provenance specification](composition.md)
- [Composition workbench design](composition-ui.md)
- [Plan and architecture](plan.md)
- [Source register and recovered features](sources-and-features.md)
- [Claims and LNSAT evidence](claims-and-evidence.md)
- [Government funding research](government-funding.md)
- [Validation and review](validation.md)
- [Development status and continuation](development.md)
- [Local prototype](../preview/index.html)
