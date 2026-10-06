# Continue Rangoon application work

Use this guide to restart work in the isolated application repository:

```sh
cd /Users/jeff/hypler/code/rangoon/app-review
pwd
git status --short --branch
git rev-parse HEAD
git remote -v
```

The continuation branch `codex/model-cloud-adapter` starts from verified PR25 merge `6dc5d664ec837bfbc25ed9c91a19f735ab1943f8`, reviewed head `6e5165bf7afc7e6424cbfeb861fe31185e8c4fad`, and exact base `c05e7d048cc281511345062219f266ddfea3a6b4`. Public source includes the local model workbench and native OS credential management; cloud model transport remains unavailable. All fourteen corrected-head hosted checks passed. Read the [development ledger](development.md) for exact reviews, synthetic macOS evidence, publication receipts and remaining GUI/security limits. This receipt and guide are intentional carry-forward documentation; the parent directory remains a separate marketing repository outside scope.

## Read first

Read the accepted objective and current evidence in this order:

1. [Intent](intent.md) — accepted V1 objective, constraints, non-goals, and authority.
2. [Local workspace](local-workspace.md) — local execution and data boundaries.
3. [Workspace data controls](workspace-data-controls.md) — backup, additive restore, and dependency-aware logical deletion.
4. [Composition contract](composition.md) — Decompose, Merge, Split, heads, conflicts, and commit semantics.
5. [Composition destinations](composition-destinations.md) — destination identity and ownership.
6. [Composition UI](composition-ui.md) — local editor behavior and UI evidence.
7. [Development ledger](development.md) — canonical publication and validation receipts.
8. [Application architecture](application-architecture.md) — source/runtime ownership.
9. [Engine integration](engine-integration.md) — inert diagnostics boundary.
10. [Model assistance](model-assistance.md), [M1a core](model-context-core.md) [local transport](model-local-adapter.md) and [native session](model-native-session.md) and [native workbench](model-native-workbench.md) — advisory, pure-byte, exact loopback and request-custody contracts.
11. [Plan](plan.md) — future product shape and staged acceptance.

Use the ledger for historical receipts. Do not copy old status paragraphs into this guide. Read additional parser, native, visual, or claim documents only when the active packet requires them.

## Current truth

The public `main` source includes the published composition store, native command layer, and native Decompose, Merge, and Split editor packet. Its exact publication checks and ancestry are recorded in the development ledger. The packet's evidence includes:

- 71 Node checks passed, including composition model and view checks;
- 16 native tests passed, with native formatting, warnings-denied Clippy, and a development build;
- 54 synthetic rendered checks passed across the three operations, three tabs, two themes, and 320/768/1440 widths;
- isolated macOS native QA passed for new Decompose, Merge, and Split outputs and restart reopening five skills;
- append, mixed-destination, complete failure/keyboard coverage, Windows/Linux GUI, installer, and release qualification remain open.

The editor is deterministic and manual over saved bytes. It does not activate the engine, call providers, transmit source, encrypt the workspace, install a released application, or submit a grant. PR15 publication does not declare R3 or V1 complete.

PR16 published the pure `rangoon-compile` implementation and ID-only read service for two frozen text profiles. PR17 published the native Compile inspection command, controller and workbench under [compilation-ui.md](compilation-ui.md), including the independently reviewed documentation correction. All fourteen hosted checks passed for its final head `98e0d9ea396d707df5ec5208995ddb49b069b772`. PR18 published the pure exact-byte bundle encoder/strict verifier and bounded selected-file host helpers after all fourteen checks passed for `213d10e9d6a1cd35a75c3b135c93fdab0a06caf6`. Exact byte, identity, source-test and synthetic browser evidence belongs in the canonical ledger.

Native Export/Inspect commands and the Compile workbench file-action states are implemented in this source under [instruction-bundle-ui.md](instruction-bundle-ui.md). Local validation passed 97 Node tests, 29 native tests, formatting, warnings-denied native Clippy, the macOS development build, visual checks and 48 synthetic rendered states. A new isolated unsigned `ai.rangoon.bundle.qa` macOS bundle exercised real native import/save/derive/review/Compile for `AGENTS.md` and `CLAUDE.md`, AGENTS export cancellation and successful exports for both profiles, external Inspect cancellation, golden success from no-workspace state, own-export success, invalid-file failure, restart, and reviewed skill persistence. The exact 61-byte BOM/CRLF/Unicode fixture was preserved; native export files were mode `0600`; export/Inspect/restart readback left the workspace hash unchanged. Duplicate failure notice behavior was fixed and independently reviewed; rebuilt native reports show one failure notice per report tab. Use the canonical ledger for independent review and publication receipts. Source/helper tests and synthetic browser checks do not establish native picker/IPC GUI behavior.

Native collision/write uncertainty, historical/composition compile, complete keyboard coverage, Windows/Linux GUI, installers, release qualification and target-loader compatibility remain open. Bundle verification reports internal consistency only, authority none, and no authentication or complete source-graph proof. Preserve the new QA bundle and earlier QA applications; no target compatibility or whole-V1 claim follows from this macOS evidence.

The initial desktop target remains macOS, Windows, and Linux together. The product is intended to be free and open source, but no license has been adopted. LNSAT remains an independent authority/evidence engine; no LNSAT operation is active in this repository.

## Safe validation rails

For the application preview and editor fixtures:

```sh
npm start
npm run check
npm run validate:visuals
npm test
node scripts/validate-review.mjs
git diff --check
```

For core Rust source changes, install Rust 1.85.0 with rustfmt and Clippy first, then use the validator that pins and prints Cargo, rustc and Clippy versions. A bare `rustup run` can still select Homebrew subcommands from PATH:

```sh
node scripts/check-source-rust.mjs
```

For desktop changes:

```sh
cargo fmt --manifest-path apps/desktop/Cargo.toml -- --check
cargo test --manifest-path apps/desktop/Cargo.toml --locked
cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets --locked -- -D warnings
cargo build --manifest-path apps/desktop/Cargo.toml --locked
```

Run only the checks relevant to the packet, record exact outcomes in `docs/development.md`, and keep known infrastructure failures distinct from source failures. Source CI or one macOS GUI run does not prove all-three-OS GUI or release support.

## Next packet

The native local Model Assistance workbench is implemented under [model-native-workbench.md](model-native-workbench.md). It uses saved IDs, full protected record text, session-only profile custody, exact retained requests, an isolated native review window and final OS consent. A synthetic macOS GUI run exercised the real commands, consent, approved transport, malformed response, cancellation and missing-input refusal. Use the latest ledger entry for exact tests, reviews, known QA limitations and publication state. M1b and V1 remain incomplete.

1. Preserve the reviewed model boundary: advisory output, no proposal application or authority; no startup traffic; numeric loopback only; single-use request handles; exact payload consent; bounded direct transport and cancellation. The inner context-pack body and outer wire body are different DTOs and must be validated separately. Do not rerender unchanged active polls.
2. Close remaining native qualification: Windows/Linux interactive confirmation, complete keyboard/screen-reader coverage, close/clear while an OS prompt is active, changed-head freshness and unavailable post-read GUI cases. Existing source tests do not substitute for those GUI receipts.
3. Native cloud credential management is published through PR25 under [model-cloud-custody.md](model-cloud-custody.md), with explicit inspect/save/remove, isolated entry and final native OS decisions. Read the latest ledger for exact review/publication and synthetic macOS evidence. The portable fixed-origin HTTPS adapter is implemented under [model-cloud-adapter.md](model-cloud-adapter.md), with its current validation/review state in the latest ledger entry. Integrate native cloud session custody and workbench next: saved-ID resolution, exact retained payload, credential-revision/OS-store lease, isolated request review, final native consent, source-free credential-transfer checking, cancellation and pre/post freshness. The desktop does not yet call the cloud adapter. All-three-OS custody, proposal-to-draft inspection/application and task-quality/token measurements remain required M1c–M1e work. Do not use existing credentials, private source or paid calls, install/launch models or activate LNSAT without their separate authorization.
4. Preserve bundle exact bytes, plaintext disclosure, exclusive creation, the 295,016-byte limit and uncertain partial-write recovery. Keep target-loader behavior, collision/write GUI, composition compile, complete keyboard coverage and Windows/Linux GUI as separate open gates.
5. Record evidence in the canonical ledger; publication requires exact head/base, fresh independent review, hosted CI and fetched ancestry.

Continue the broader V1 plan through adapters, compatibility/export, Workflows, Agents/templates, Connectors/Harnesses, local security design and later team/cloud work. Preserve separate decisions for engine activation, private or paid provider use, encryption, licensing, deployment and grants. Do not call this source packet complete V1 or full R3/R4/R5.

## Hard boundaries

- Keep synthetic browser fixtures separate from real local source and native QA data.
- Use explicit user-selected files for native analysis; do not add automatic scanning.
- Do not activate LNSAT, existing providers, private-source transfer, credentials, or execution authority. Disposable synthetic loopback tests are authorized; actual source transfer requires the accepted explicit native consent boundary.
- Do not claim encryption, authentication, tamperproof recovery, installer support, or released-OS support.
- Do not edit the parent marketing site or deploy it from this repository.
- Reviewed source commit/push, PR creation, and main merge are authorized; verify the exact diff, review and CI gates before publication.
- Installer/release publication, license adoption, engine activation, deployment, paid services, and government submission remain separate decisions.
- Update the canonical ledger rather than duplicating receipts here.
