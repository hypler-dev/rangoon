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

Producer: primary Sol controller for architecture, host/native boundary, integration, CI and evidence; OpenAI GPT-5.6-Terra high for the five-file UI/controller/test slice; OpenAI GPT-5.6-Luna medium for the README. Independent OpenAI GPT-5.6-Terra xhigh UI review closed a theme-button accessibility finding. Independent native-boundary review required Windows pre-open and opened-handle reparse tests; both corrections received independent PASS. The first hosted Windows core job then passed the actual reparse tests. GLM was skipped after the earlier recorded transport failures; no private payload was transferred.

### R1b evidence, October 5

- Local macOS: core 23 tests, desktop 2 tests, formatting, Clippy with warnings denied and native executable build passed. Windows adds one opened-handle test and has its own file/directory link fixture; counts differ by target.
- Node analysis-controller tests: 6 passed, including native command shape, cancellation/rejection preservation, clear/stale results and escaping. The full Node suite passed all 10 tests; the first hosted receipt and correction are recorded below.
- Actual macOS runtime: launched an unsigned local development bundle built from this source (temporary packaging for UI automation, not a distributed artifact). Native picker cancellation restored focus to Choose. Selecting the tracked golden fixture displayed 144 bytes, 7 lines, 3 sections and SHA-256 `453acea7707fd5d102d47c0539950f5d93cd997561e704db516548761ab4b42d`. Section selection updated byte/line metadata and visibly highlighted lines 6–7 while retaining section focus.
- Further macOS runtime checks passed: a NUL-containing replacement returned `binary_input` and preserved the successful source/selection; Clear removed the report and restored Choose focus; an empty file showed zero bytes/lines/sections and `empty_source`; light styling remained readable. Relaunch with the reviewed theme-button correction showed a plain action button and no retained source.
- Windows/Linux picker behavior, fresh-host installs, signing, updates and release qualification remain unverified. Three-OS desktop compilation/non-GUI tests are a separate source gate, not proof of those runtime behaviors.

### First R1b hosted run and corrections

[PR run 37290891929](https://github.com/hypler-dev/rangoon/actions/runs/37290891929) for source `34ec7dc62fb7c8297ca88112f1d01f7c13a51090` passed all three Rust 1.85 jobs, the Node/review job and native macOS/Linux build/test/Clippy jobs. The Windows native build failed because `icons/icon.ico` was missing. That failure remains part of the record. A derived Windows icon from the existing brand symbol and its reproducible Tauri generation command are now included. Correction commits, exact-head checks and final publication receipts are tracked in [PR #2](https://github.com/hypler-dev/rangoon/pull/2); consult those checks for the current source-build result.

A separate usability correction bounds source and section-list scrolling, reveals a selected section's first line within the source pane, preserves pane scroll during state/theme changes and resets it for a different source. Browser-only unavailable states were checked at 320 and 768 pixels, with no horizontal page overflow. A populated narrow native view remains a manual qualification item.

Actual macOS long-file QA passed with a 214,396-byte synthetic file containing 9,000 lines and 45 sections. Keyboard Tab navigation reached section 45; Return retained focus and list position, displayed its source beginning at line 8,801 inside the bounded pane, and showed bytes 209,607–214,396. [The captured native result](screenshots/native-analysis-long.png) records this behavior. This is a usability smoke check, not a throughput benchmark. The scrolling correction and Windows icon resource received fresh independent UI/native review PASS.

R1b reached main through [PR #2](https://github.com/hypler-dev/rangoon/pull/2): reviewed source `94ba0cfc77bfff3010df7b400099861aaa298bec`, merge `79c27d5a9d87558a4fb9a7b366c360f1ac9ff6ee`. All fourteen push/PR checks passed for the reviewed head.

## Durable local source snapshots: R2a

The current continuation adds one bounded durable loop: select a Markdown file, inspect it, explicitly Save locally, quit, reopen, list the saved record, and open it again. Native commands use an opaque `sourceId`; the renderer cannot choose a filesystem path or source body. The desktop stores source bytes and metadata in `app_local_data_dir()/source-workspace/workspace.sqlite3` only after Save. Storage is unencrypted on this computer, with owner-only Unix permissions and inherited per-user Windows ACLs; it is not an encrypted vault or tamper-proof evidence store.

Identical basename and bytes return the existing record. Changed bytes or basename create a separate immutable snapshot. Reopen reanalyzes persisted bytes with the deterministic analyzer and preserves `authority: none` plus `unreviewed` fragments. Save, list, open and clear preserve the specified current-report and failure semantics; malformed identity/schema/database records fail closed rather than being silently omitted. Limits are 128 snapshots, 256 KiB per source and 64 MiB database. No automatic save, browser source storage, upload, provider call, delete or retention policy exists.

### R2a local evidence, October 5

- Rust 1.98 local core suite: 33 tests passed, plus one ignored subprocess helper that the interrupted-writer test explicitly launches and kills. Storage contributes 10 tests covering byte-preserving save/reopen, deduplication, changed content, invalid identity, corrupted bytes, schema/trigger rejection, 128-record cap, rollback, SQLite page exhaustion, interrupted writer recovery and Unix permission/symlink handling. The existing Windows host suite has an additional opened-reparse test; store permission tests also differ by platform.
- Native unit tests: 4 passed, including stale source identity and Clear generation handling. Core/native formatting and Clippy with warnings denied passed. Native executable build passed. Controller suite plus preview tests: 14 passed before the later splash change.
- Actual macOS native IPC: selected the public 144-byte, 7-line, 3-section fixture, saved it, saved it again (`Already saved`, one record), quit the process, relaunched, observed one saved source with no active report, and reopened it. SHA-256 remained `453acea7707fd5d102d47c0539950f5d93cd997561e704db516548761ab4b42d`; source ID remained `source:2c0a616d8d080a745118c5ccbae7217fdfd724cfc8a0b8bb57053eeec69301b8`. Fragments remained unreviewed and authority remained none. Clear removed the active report while preserving the saved record. No injected browser bridge was used.
- Fresh OpenAI GPT-5.6-Terra xhigh storage/native review: PASS after the actual lifecycle receipt closed its P3 evidence gap. Fresh Terra xhigh UI review: PASS after correcting memory-only copy, unavailable-list states and ambiguous Open button labels. Primary controller owns schema/native code and final judgment; Terra high owns bounded UI implementation, Luna medium owns documentation synchronization.
- Other-provider review: the hardened GLM registry resolved requested model `glm-5.3` and the four-file dry run passed. Automatic approval review rejected the external source/test transfer because it lacked specific payload/destination approval. No request reached GLM, so no returned model or contact error exists. Native review was used; no bypass or Spark fallback occurred.

Exact final-source three-OS CI and publication receipts belong to the review PR for `codex/durable-source-workspace`; verify its head, checks and merge state before extending this packet. [Branch checks](https://github.com/hypler-dev/rangoon/actions?query=branch%3Acodex%2Fdurable-source-workspace) retain both failed and passing attempts. The macOS GUI receipt is one-OS manual evidence; Windows/Linux interactive persistence, installer readiness, signing, upgrades and released support remain unverified.

### Explicit icon and launch-screen continuation

Jeff selected the exact transparent Rangoon symbol and requested a beautiful splash screen. The selected PNG matches the existing brand asset byte for byte. Native PNG/ICO/ICNS resources now derive from that exact asset; [icon provenance](../apps/desktop/icons/README.md) records hashes and the reproducible tool command. The splash implements actual startup state and has a separate visual preview. The OS application name is **Rangoon** with a logo-only icon; Jeff explicitly retained **Rangoon.ai** as permitted splash branding. The source review and final runtime checks below passed; publication is tracked through the branch review PR.

- Fresh OpenAI GPT-5.6-Terra xhigh startup/branding review: PASS after correcting initial Escape handling, the inactive skip link, duplicated spoken logo text and a premature documentation claim. No outstanding P1/P2/P3 findings. Producer: OpenAI Terra high, with controller integration and validation; design: Terra high.
- Browser preview at 1280 × 720 fits without page overflow. At 320 × 480 it has no horizontal overflow and allows vertical access to lower controls. The standalone [launch screenshot](screenshots/splash-dark.png) is a visual preview, not a captured native loading delay. There is no artificial minimum splash duration.
- A temporary, explicitly synthetic localhost harness imported the unchanged `launch.mjs` and exercised initial Escape recovery, failed-workspace completion, the eight-second timeout, Retry followed by another timeout, and Open workbench with keyboard focus restored. This validates browser startup states, not native storage.
- Rebuilt native macOS development executable with the final icon/name/startup source: launch completed to the usable workbench, showed the existing saved record, and reopened the exact fixture with unreviewed state. [Native reopen screenshot](screenshots/native-workspace-reopen.png) records the actual native result. The temporary QA bundle is not a signed or distributed installer.
- Final local checks: 14 Node tests pass, JS syntax checks pass, core/native formatting and Clippy pass, native tests/build pass, reference hashes and Markdown checks pass, and intent/specification/plan validators pass. A sandboxed Node attempt failed its socket-listener permission; the authorized unrestricted rerun passed. No test or security constraint was weakened.

## External action boundaries

Commit and push of reviewed app work, review PR creation, repository metadata updates and main merge are authorized by the October 5 request. Deployment, production data changes, LNSAT mutations, license adoption, installer/release publication, paid infrastructure and government application submission remain closed.
