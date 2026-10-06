# Continue Rangoon application work

Use this guide to restart work in the isolated application repository:

```sh
cd /Users/jeff/hypler/code/rangoon/app-review
pwd
git status --short --branch
git rev-parse HEAD
git remote -v
```

The continuation branch `codex/native-bundle-export` starts from PR18 merge `309060254fa4e01526e94dbe5a34cf30fc4a821e`, reviewed head `213d10e9d6a1cd35a75c3b135c93fdab0a06caf6`, with exact base `42e79dd03c6d001fa8cbde40193e29c801896e73`. The R3 editor, pure compiler, native Compile inspection and portable bundle core/host foundation are published on `main`; use the startup commands for current branch and working-tree truth, and the [development ledger](development.md) for receipts. The parent directory is a separate marketing site; do not mix its history, files, or deployment workflow with this application repository.

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
10. [Plan](plan.md) — future product shape and staged acceptance.

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

Native Export/Inspect commands and the Compile workbench file-action states are implemented in this source under [instruction-bundle-ui.md](instruction-bundle-ui.md). Local validation passed 97 Node tests, 29 native tests, formatting, warnings-denied native Clippy, the macOS development build, visual checks and 48 synthetic rendered states. Use the canonical ledger for independent review and publication receipts. Source/helper tests and synthetic browser checks do not establish native picker/IPC GUI behavior.

Native Compile/Export/Inspect GUI, target-loader qualification and full R4/R5 remain open. Bundle verification reports internal consistency only, authority none, and no authentication or complete source-graph proof. An earlier isolated macOS Compile QA app opened, but automation stopped when the user navigated into its sample preview. Leave that app and the earlier Composition QA app untouched; no actual Compile or bundle-export IPC/GUI result is claimed.

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

For core Rust source changes:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
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

Finish publication gates for the current native bundle source packet if the canonical ledger has no exact merged-head receipt. Then qualify the real macOS Compile/Export/Inspect picker and IPC flow in a new disposable development bundle with an isolated application identifier. Keep the user's workspace and earlier QA applications untouched. Record native behavior independently from synthetic fixtures; preserve Windows/Linux GUI and target-loader gates.

1. Read [compilation.md](compilation.md), [instruction-bundle.md](instruction-bundle.md), [compilation-ui.md](compilation-ui.md) and [instruction-bundle-ui.md](instruction-bundle-ui.md), then the R4/R5 rows in [plan.md](plan.md).
2. Exercise explicit synthetic-file selection, save, derive, local review, Compile, export to a fresh neutral filename and independent file inspection through real native commands. Include cancellation, collisions and failure recovery where feasible; no provider or engine activation.
3. Preserve deterministic exact-byte output, plaintext disclosure, exclusive creation, the 295,016-byte bundle limit and uncertain partial-write recovery. Do not imply installation, authenticated review, complete provenance or target compatibility.
4. Qualify target-loader behavior, destination drift/collision behavior and all-three-OS failures as separate R4/R5 gates.
5. Preserve remaining editor gates: append/mixed destinations, complete failure/keyboard coverage, Windows/Linux GUI, installers and release support.
6. Record local evidence in `docs/development.md`; publication receipts require exact head/base, independent review, hosted CI and fetched ancestry.

Continue the broader V1 plan through adapters, compatibility/export, Workflows, Agents/templates, Connectors/Harnesses, local security design and later team/cloud work. Preserve separate decisions for engine activation, providers, encryption, licensing, deployment and grants. Do not call this source packet complete V1 or full R3/R4/R5.

## Hard boundaries

- Keep synthetic browser fixtures separate from real local source and native QA data.
- Use explicit user-selected files for native analysis; do not add automatic scanning.
- Do not activate LNSAT, providers, network transfer, credentials, or execution authority.
- Do not claim encryption, authentication, tamperproof recovery, installer support, or released-OS support.
- Do not edit the parent marketing site or deploy it from this repository.
- Reviewed source commit/push, PR creation, and main merge are authorized; verify the exact diff, review and CI gates before publication.
- Installer/release publication, license adoption, engine activation, deployment, paid services, and government submission remain separate decisions.
- Update the canonical ledger rather than duplicating receipts here.
