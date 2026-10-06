# Continue Rangoon application work

Use this guide to restart work in the isolated application repository:

```sh
cd /Users/jeff/hypler/code/rangoon/app-review
pwd
git status --short --branch
git rev-parse HEAD
git remote -v
```

The continuation branch `codex/bundle-native-qualification` starts from PR19 merge `356df0caa547ec5a6113c9edbf2e3e42913ce254`, reviewed head `8927211655a7a1047f73fa121e8c1da562746967`, with exact base `309060254fa4e01526e94dbe5a34cf30fc4a821e`. The R3 editor, pure compiler, native Compile workbench, portable bundle foundation and native Export/Inspect source are published on `main`; use startup commands for current branch and working-tree truth, and the [development ledger](development.md) for receipts. The parent directory is a separate marketing site; do not mix its history, files or deployment workflow with this application repository.

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

PR19 publication gates passed; its exact merged-head receipt is in the canonical ledger. Native macOS Compile/Export/Inspect qualification now has bounded evidence in the new isolated unsigned QA bundle. Continue with M1a from [model-assistance.md](model-assistance.md): freeze exact context-pack and advisory-proposal schemas, canonical serialization, digest framing, range rules, and golden/negative vectors before implementation. Keep local and cloud assistance as separate later V1 adapter deliverables; operator use stays optional. Preserve Windows/Linux GUI, target-loader, collision/write, composition-compile, and full keyboard gates.

1. Read [model-assistance.md](model-assistance.md), then freeze M1a wire fields, tagged identities, exact UTF-8 range/citation rules, canonical digest framing, ceilings, and independent golden/negative vectors.
2. Keep deterministic/manual workflows usable without a model. Do not add private/paid calls, existing-credential use, automatic transfer, provider fallback, model installation/launch, or live activation.
3. Preserve exact-byte output, plaintext disclosure, exclusive creation, the 295,016-byte bundle limit and uncertain partial-write recovery. Do not imply installation, authenticated review, complete provenance or target compatibility.
4. Carry forward target-loader behavior, destination drift/collision behavior, composition compile, complete keyboard coverage and all-three-OS GUI failures as separate gates.
5. Record local evidence in `docs/development.md`; publication receipts require exact head/base, independent review, hosted CI and fetched ancestry.

Continue the broader V1 plan through adapters, compatibility/export, Workflows, Agents/templates, Connectors/Harnesses, local security design and later team/cloud work. Preserve separate decisions for engine activation, private or paid provider use, encryption, licensing, deployment and grants. Do not call this source packet complete V1 or full R3/R4/R5.

## Hard boundaries

- Keep synthetic browser fixtures separate from real local source and native QA data.
- Use explicit user-selected files for native analysis; do not add automatic scanning.
- Do not activate LNSAT, providers, network transfer, credentials, or execution authority.
- Do not claim encryption, authentication, tamperproof recovery, installer support, or released-OS support.
- Do not edit the parent marketing site or deploy it from this repository.
- Reviewed source commit/push, PR creation, and main merge are authorized; verify the exact diff, review and CI gates before publication.
- Installer/release publication, license adoption, engine activation, deployment, paid services, and government submission remain separate decisions.
- Update the canonical ledger rather than duplicating receipts here.
