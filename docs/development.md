# Development status

Authority: [intent.md](intent.md). Jeff authorized publishing the application work to `hypler-dev/rangoon` and continuing development on October 4, 2026. The repository is an application in development. Preview completeness does not imply product or runtime qualification.

October 5 continuation: Jeff authorized a detailed README, corrected repository metadata/topics, publication to main and further development. R0/R1a reached reviewed main publication in PR #1. A bounded desktop-shell and explicit file-analysis spike follows; it is not a desktop release.

## Publication

R0 was committed and pushed as `b37e1f9e3d201440e4c6480c2a33dc42367e2abd` on `codex/app-vision-preview`. GitHub returned the exact same branch SHA. [PR #1](https://github.com/hypler-dev/rangoon/pull/1) began as a draft and is now merged; see the October 5 receipt below. The repository description now identifies Rangoon as an application in development. Two initial HTTP pushes returned `HTTP 400`; the bounded retry using HTTP/1.1 and a 50 MiB request buffer succeeded. No force push was used. At that initial R0 checkpoint, no merge or direct update to main had been performed. The marketing-site repository stays separate.

R0 validation: [validation.md](validation.md). Final root-owned changes were independently reviewed by OpenAI GPT-5.6-Terra; review corrections passed. OpenAI GPT-5.6-Luna independently checked the exact outgoing packet for accidental private payloads; no blocking finding was reported.

## Completed source foundation: R1a

Implemented a platform-neutral Rust domain and source-analysis foundation with a stdin-only CLI. It accepts bounded UTF-8 instruction bytes, preserves original byte hashes and exact line spans, detects Markdown sections deterministically, and represents extracted sections as unreviewed proposals. The caller supplies every byte; a display name never opens a path. No recursive scan, archives, network, models, hooks, scripts, execution authority, persistence or installation. The experimental contract is recorded in [source-analysis.md](source-analysis.md).

This is a bounded early implementation of R1 contracts and the pure-analysis prerequisite for R2/R3, not completion of those packets. Three-OS source-test CI now passes. Full desktop qualification, shared UI evolution, license selection and real installers remain R1/R6 follow-ups.

Controller owns architecture, contracts, integration, validation and git state. Producer roles: primary Sol controller for domain/CLI/contracts/CI and integration; native OpenAI GPT-5.6-Terra high for the bounded analyzer and its tests. A fresh OpenAI GPT-5.6-Terra xhigh reviewer returned PASS for the 25-file R1a diff, with no actionable P1/P2/P3 findings, then independently passed the test-fixture correction. Hosted CI closed the minimum-toolchain and three-OS source-check gap. GLM was not retried after the recorded transport failures in the preview packet; an adequate native review lane is available.

## R1a validation ledger

Local validation on macOS, October 4, 2026: Rust 1.98.0 and Node 24.21.0. The CI configuration pins Rust 1.85.0 on Ubuntu 24.04, Windows Server 2022 and macOS 14.

R1a was committed and pushed as `ac6648ccb5ebbbc4c12b0c7f3d8ad4b36bed11e2`; the remote SHA matched exactly. The first [hosted run](https://github.com/hypler-dev/rangoon/actions/runs/37267155400) passed formatting and all 18 Rust tests on every OS, and passed the preview job. Its three Rust jobs failed Clippy 1.85's `format_collect` lint in two test-fixture builders. The follow-up appends headings directly to a string with `writeln!`, preserving the fixtures and warning-denial policy. The first run remains a failure in the historical record.

Corrected source commit: `36a5ce68b85912af76cf4e0e1a2e2ce2931caf3e`. [PR CI run 37267392112](https://github.com/hypler-dev/rangoon/actions/runs/37267392112) completed successfully for that exact head: Rust 1.85 formatting, all 18 tests and Clippy on Ubuntu 24.04, Windows Server 2022 and macOS 14, plus the Linux preview/review-artifact job. These are hosted source checks; they do not establish released desktop support, installer readiness or native execution enforcement. Later documentation-only commits retain this immutable source receipt; inspect PR checks for their own exact-head results.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 18 tests; 3 domain, 10 analyzer, 4 CLI and 1 cross-platform golden contract |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS |
| `npm run check` | PASS |
| `npm test` | PASS: 4 tests, including local static-server confinement |
| `node scripts/validate-review.mjs` | PASS: original asset/brief hashes, local Markdown targets and authored-text checks |
| Independent Python SHA-256/framing check | PASS: golden source identity, original-byte hash, reconstruction and expected line ranges independently recomputed |
| Intent and plan schema validators | PASS for both canonical artifacts |
| `git diff --cached --check` | PASS |
| Independent R1a review | PASS: OpenAI GPT-5.6-Terra xhigh; no actionable findings |
| Three-OS hosted CI | PASS for corrected source `36a5ce68b85912af76cf4e0e1a2e2ce2931caf3e`: three Rust jobs and the preview job |

Initial Cargo resolution required fetching the locked public dependencies; offline tests then passed. Parser boundary tests cover BOM-prefixed fences, CRLF line-length limits, invalid UTF-8/NUL, duplicate headings, unsupported names, line/fragment/input caps and inert hostile instructions. The golden fixture is synthetic and enforces identical report semantics across platforms.

The next bounded candidate at the R1a checkpoint was a three-OS desktop shell/native-bridge spike; R1b below implements its first selected-file flow. Durable snapshots, repository scanning, semantic decomposition, real adapter compilation and governed execution remain later work. The preview remains synthetic; it has not been connected to this CLI.

## Main publication receipt: October 5

README/publication-intent commit `033482a4160baac528551f0ef6d79ab8ec9ea3d1` received an independent OpenAI GPT-5.6-Terra xhigh PASS after correcting the CLI quickstart to use the tracked fixture. All eight push/PR checks passed for that exact head. PR #1 was marked ready and merged with an exact-head guard; GitHub reports merge commit `ff901594d731ee74ea8b9170c885629c3e4caf09` on October 5. Fetched public main contains the reviewed head. Branch protection was absent and repository rulesets were empty when checked; source CI and independent review were still required. No force push, installer release or deployment occurred.

GitHub description: “Local-first capability workbench for AI agents: inspect instructions, trace provenance, and plan governed workflows. In development for macOS, Windows, and Linux.” Homepage: `https://rangoon.ai`. Discovery topics: `agent-skills`, `agent-workflows`, `ai-agents`, `ai-governance`, `developer-tools`, `lnsat`, `local-first`, `provenance`, `rust`. These are repository topics, not version/release tags; no product release was created.

## Desktop selected-file continuation: R1b

The latest user direction is captured in [product-experience.md](product-experience.md): make the image workflows useful through complete, traceable actions. All eight originals were visually reviewed again. R1b implements one such action in a separate Tauri 2 desktop workspace: native one-file picker, bounded host read, existing deterministic parser, and an actual source/section/inspector UI with dark/light themes. [desktop-spike.md](desktop-spike.md) records commands and boundaries. The synthetic preview remains separate; its Import page links to the real analysis page, which explains when a native bridge is unavailable.

No renderer path or bytes are accepted by the bridge. The host checks file type before opening and on the opened handle, limits reads, and returns fixed non-echoing errors. The UI retains prior analysis on cancellation/rejection, ignores stale results after Clear, restores focus, and renders source as escaped text. Reports remain in memory. Persistent workspaces, semantic skills, real merge/split, harness compilation, connector actions and execution remain future work.

Producer: primary Sol controller for architecture, host/native boundary, integration, CI and evidence; OpenAI GPT-5.6-Terra high for the five-file UI/controller/test slice; OpenAI GPT-5.6-Luna medium for the README. Independent OpenAI GPT-5.6-Terra xhigh UI review closed a theme-button accessibility finding. Independent native-boundary review required Windows pre-open and opened-handle reparse tests; both corrections received independent PASS, with actual Windows execution pending hosted CI. GLM was skipped after the earlier recorded transport failures; no private payload was transferred.

### R1b evidence, October 5

- Local macOS: core 23 tests, desktop 2 tests, formatting, Clippy with warnings denied and native executable build passed. Windows adds one opened-handle test and has its own file/directory link fixture; counts differ by target.
- Node analysis-controller tests: 6 passed, including native command shape, cancellation/rejection preservation, clear/stale results and escaping. The full Node suite passed all 10 tests; the new three-OS desktop CI jobs have not run yet, so hosted qualification and an exact-head receipt remain pending.
- Actual macOS runtime: launched an unsigned local development bundle built from this source (temporary packaging for UI automation, not a distributed artifact). Native picker cancellation restored focus to Choose. Selecting the tracked golden fixture displayed 144 bytes, 7 lines, 3 sections and SHA-256 `453acea7707fd5d102d47c0539950f5d93cd997561e704db516548761ab4b42d`. Section selection updated byte/line metadata and visibly highlighted lines 6–7 while retaining section focus.
- Further macOS runtime checks passed: a NUL-containing replacement returned `binary_input` and preserved the successful source/selection; Clear removed the report and restored Choose focus; an empty file showed zero bytes/lines/sections and `empty_source`; light styling remained readable. Relaunch with the reviewed theme-button correction showed a plain action button and no retained source.
- Windows/Linux picker behavior, fresh-host installs, signing, updates and release qualification remain unverified. Three-OS desktop compilation/non-GUI tests are a separate source gate, not proof of those runtime behaviors.

## External action boundaries

Commit and push of reviewed app work, review PR creation, repository metadata updates and main merge are authorized by the October 5 request. Deployment, production data changes, LNSAT mutations, license adoption, installer/release publication, paid infrastructure and government application submission remain closed.
