# Rangoon

Rangoon is a capability workbench for making AI instructions, skills, workflows, and connector plans understandable, portable, and reviewable. It is an application in development. This repository currently contains a synthetic interactive preview, a small deterministic Rust source-analysis foundation, and an R1b Tauri desktop spike for analyzing one explicitly selected file; it is not a released desktop app or execution runtime.

![Rangoon Command Center preview](docs/screenshots/command-dark.jpg)

The product direction is a free, open-source workbench for developers and small platform teams managing `AGENTS.md`, `CLAUDE.md`, `SKILL.md`, rules, tools, hooks, and project context across repositories. The first desktop release is planned for macOS, Windows, and Linux together. A specific software license has not been adopted yet, so this repository makes no license grant.

## What exists now

### Interactive preview

The local preview has nine synthetic workbench views:

- Command Center
- Import & Analyze
- Decompose
- Merge & Split
- Skills
- Workflows
- Connectors
- Evidence
- Three-desktop release plan

It includes dark and light themes, narrow layouts, keyboard navigation, compact and focus modes, fixture-driven state changes, merge conflict review, inert draft compilation, and an explicit unknown-outcome workflow state. It uses sample data only. It does not read project folders, call providers, persist a workspace, install anything, execute instructions, authorize actions, or connect to LNSAT. Its analysis route explains that browser preview analysis is disabled; the real selected-file flow belongs to the native desktop spike below.

Run it with Node.js 20 or later:

```sh
git clone https://github.com/hypler-dev/rangoon.git
cd rangoon
npm start
```

Open `http://127.0.0.1:4377`. The preview server is local and dependency-free. Theme preference may remain in the browser; reloading resets the sample workspace. See the [preview gallery](docs/preview-gallery.md) for rendered dark, light, and narrow captures.

### Rust source-analysis foundation

The portable Rust slice accepts caller-supplied Markdown bytes on stdin and returns a deterministic JSON inventory. It preserves the exact source text, SHA-256 digest, display label, line count, section spans, and unreviewed fragment proposals. It recognizes a deliberately small Markdown subset: ATX headings, preambles, and fenced blocks. It is an inert source-analysis library, not a CommonMark parser, semantic extractor, harness compiler, policy engine, or executor.

Build and test it with Rust 1.85 or later:

```sh
cargo run --locked -p rangoon-cli -- analyze --name AGENTS.md < fixtures/contracts/AGENTS.md
cargo test --workspace --locked
```

The command opens no file. The shell supplies bytes through stdin; `--name` is a display label, never a path. `AGENTS.md`, `CLAUDE.md`, and `SKILL.md` receive format labels; other valid `.md` names are generic.

On macOS and Linux, ordinary redirection preserves the file bytes:

```sh
cargo run --locked -p rangoon-cli -- analyze --name AGENTS.md < fixtures/contracts/AGENTS.md
```

On Windows, use binary-preserving redirection from Command Prompt:

```bat
cargo run --locked -p rangoon-cli -- analyze --name AGENTS.md < fixtures\contracts\AGENTS.md
```

Text-mode PowerShell pipelines can change encoding or newlines before stdin reaches the CLI; the reported digest describes bytes actually received. The [contract fixture](fixtures/contracts/AGENTS.md) is safe sample text; its instructions remain data.

Successful output goes to stdout as JSON. Rejected input produces a fixed JSON error on stderr and a nonzero exit. The analyzer rejects invalid display names, non-`.md` names, NUL bytes, invalid UTF-8, oversized input, overlong lines, too many lines, and too many fragments. Limits are hard bounds: 256 KiB total source bytes, 10,000 logical lines, 16,384 content bytes per line, and 256 fragments. Input is rejected rather than silently truncated. An unclosed fence remains inert and is reported as a diagnostic. Empty input returns an empty report with an explicit diagnostic.

See [source-analysis.md](docs/source-analysis.md) for the complete experimental contract and [development.md](docs/development.md) for recorded validation evidence.

### Native selected-file analysis spike (R1b)

The desktop shell is a separate Tauri workspace under `apps/desktop`, using Rust 1.98 and the existing Rust analysis library. Run it from the application repository with:

```sh
cargo run --manifest-path apps/desktop/Cargo.toml --locked
```

Install the [Tauri native build prerequisites](docs/desktop-spike.md#run-from-source) for the host OS first. The native window opens a file picker for one Markdown file. The renderer supplies no path, source bytes, or options: the native side reads the selected file, returns its original source, fragments, digest, diagnostics, and fixed errors, and keeps the report in memory only. Cancellation and failed replacement preserve the current report. No selected path is sent from the renderer, no provider request or upload occurs, and no source is written to storage, logs, telemetry, or a database.

The three-OS source/build qualification matrix covers macOS, Windows, and Linux as work in progress. Current manual runtime evidence proves the macOS path only: picker launch and cancellation, a golden 144-byte file producing 7 lines and 3 sections, and section selection highlighting. Windows and Linux native UI behavior remain unverified. There is no installer and no released OS support claim. See the [desktop spike record](docs/desktop-spike.md) and [product-experience behavior contract](docs/product-experience.md#image-to-behavior-contract).

## Product boundary

Rangoon owns the user-facing lifecycle: discovery, provenance, editing, decomposition, composition, compatibility, static tests, compilation planning, review, and evidence views. LNSAT remains an independent reference authority and evidence engine. Rangoon does not copy LNSAT implementation or recreate its authority semantics in UI code. A future qualified adapter may reference exact LNSAT contracts; the present preview and analyzer do not connect to it.

Management must remain useful when execution is unavailable. Imported files are data, not commands. Future import, provider, compiler, connector, and authority paths require explicit contracts, bounded inputs, visible review, and evidence. A UI field, model response, credential, test pass, or subscription entitlement cannot by itself authorize a consequential operation.

## Current versus planned

| Area | Current evidence | Planned work |
| --- | --- | --- |
| UI | Local synthetic browser preview with nine views and dark/light states; native R1b analysis view | Qualify the native shell and bridge on macOS, Windows, and Linux |
| Import | Bounded stdin analysis plus one explicitly selected native Markdown file; no directory scan | Safe discovery, durable snapshots, and a persistent local workspace |
| Editing | Fixture interactions for review, merge, split, and draft states | Capability revisions, lineage, provenance, and reversible editing |
| Compatibility | No production adapters | Versioned harness adapters with golden fixtures and visible unsupported fields |
| Testing | Static preview tests and Rust source-analysis tests | Deterministic bundles, fixture simulation, static test lab, and export evidence |
| Execution | No executor, provider call, connector call, or deployment | Only qualified governed operations through an explicit authority and enforcement path |
| Distribution | No installer or released OS support; native UI evidence is currently macOS-only | Fresh-host qualification, signing, update, rollback, and docs for all three OSs |

These distinctions are deliberate. A buildable artifact, a browser capture, or a passing source test does not establish desktop support, native enforcement, security certification, or production readiness.

## Architecture direction

The proposed shape is a modular monolith: React/TypeScript for interaction, Rust for application services and native boundaries, and a Tauri shell after a three-OS qualification spike. Local management and a later owner-hosted service should share versioned domain contracts. Local SQLite and content-addressed artifacts are proposed for the first durable workspace; a service boundary and authenticated workers come later. These paths are planning targets, not implemented modules:

```text
apps/studio/                 shared React + TypeScript UI
apps/desktop/                Tauri shell and scoped native bridge
apps/server/                 Rust API, service composition, job supervisor
crates/rangoon-domain/       capability revisions, provenance, compatibility
crates/rangoon-import/       bounded inert discovery and parser isolation
crates/rangoon-jobs/         durable jobs, leases, cancellation, recovery
packages/contracts/          versioned schemas and generated types
packages/adapter-kit/        importer/compiler/runner conformance fixtures
adapters/                    independently versioned harness and connector adapters
fixtures/                    secret-free source corpus and hostile inputs
docs/                        decisions, support matrix, evidence, runbooks
```

The current Rust crates are the first small foundation under this direction. The full tree is not scaffolded by this preview.

## Roadmap

The roadmap is proposed in [plan.md](docs/plan.md). Its sequence is:

1. Select a license, freeze contracts, and qualify the desktop shell and native bridge on all three operating systems.
2. Add durable local workspaces and safe, explicit import with hostile-input coverage.
3. Add capability IR, editing, lineage, merge/split, and provenance.
4. Add pinned harness adapters and visible compatibility results.
5. Add static tests, fixture simulation, deterministic export, and evidence bundles.
6. Qualify a free desktop alpha on macOS, Windows, and Linux.
7. Separately qualify one disposable governed operation through an accepted LNSAT path.
8. Extend to owner-hosted teams, an extension SDK, and later enterprise service boundaries only after their own evidence gates.

No roadmap item changes LNSAT, grants a license, claims released OS support, or authorizes production or government operations.

## Security and privacy boundary

The portable analyzer performs no network access, persistence, filesystem scanning, model/provider call, script execution, package installation, hook execution, authorization, or external write. The R1b desktop spike reads only one explicitly selected file and keeps its report in memory; it does not scan directories or persist source. Its output includes original source content and is therefore a local analysis artifact, not redacted telemetry. Keep sensitive files out of shared logs and exports.

Future import and remote-analysis work must keep prompt-injection content as data, validate outputs against schemas, show payload and retention choices, and distinguish unreadable or excluded files from an empty project. Native bridges must expose narrow selected-folder and approved-output operations rather than a generic shell. Credentials belong in OS secret stores or an explicitly reviewed fallback. A subprocess is not automatically a sandbox.

Rangoon and LNSAT stay separate stores and trust boundaries. Nothing here claims security certification, compliance approval, domestic processing, or government endorsement. See [claims and evidence](docs/claims-and-evidence.md) for the current LNSAT audit boundary.

## Development and contribution

Read [intent.md](docs/intent.md) first, then [plan.md](docs/plan.md), [development.md](docs/development.md), and [validation.md](docs/validation.md). Keep changes small, scoped, and evidence-backed. Include the exact commands and results for behavior you change. Preserve synthetic fixture labels and separate implemented behavior from proposals. Do not add secrets, customer data, provider payloads, deployment state, or claims of certification.

Useful checks from the repository root:

```sh
npm run check
npm test
node scripts/validate-review.mjs
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

The Node checks cover the local preview and server. The Rust checks cover the source-analysis foundation. They do not qualify installers, native interactions, released platform support, LNSAT runtime behavior, or production execution.

## Documentation index

- [Accepted intent](docs/intent.md) — scope, constraints, boundaries, and acceptance.
- [Architecture and roadmap](docs/plan.md) — proposed product shape, support matrix, and build packets.
- [Source register](docs/sources-and-features.md) — recovered requirements and evidence sources.
- [Claims and LNSAT evidence](docs/claims-and-evidence.md) — maturity, proof gaps, and safe wording.
- [Source-analysis contract](docs/source-analysis.md) — input limits, parser subset, records, and errors.
- [Development status](docs/development.md) — current implementation and validation receipts.
- [Validation and review](docs/validation.md) — command results, browser QA, review scope, and limitations.
- [Desktop spike](docs/desktop-spike.md) — native selected-file analysis boundary, prerequisites, and qualification limits.
- [Product experience](docs/product-experience.md) — image-to-behavior contract and first useful file lifecycle.
- [Preview gallery](docs/preview-gallery.md) — rendered synthetic screens.
- [Visual provenance](docs/artwork.md) — source and derived asset records.
- [Funding research](docs/government-funding.md) — research plan, not eligibility or submission.
- [Review submission](docs/review-submission.md) — packet review proposal.

The marketing website is maintained in a separate checkout. This repository is the application repository under development.
