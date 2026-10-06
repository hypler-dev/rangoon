# Continue Rangoon application work

Use this guide to restart work in the isolated application repository:

```sh
cd /Users/jeff/hypler/code/rangoon/app-review
pwd
git status --short --branch
git rev-parse HEAD
git remote -v
```

The continuation branch `codex/model-native-consent` starts from PR22 merge `31a082f4d97aad6d95ca9610544e3d90686dbbbf`, reviewed head `cbed9fbc7f40fc3c036eaa936269a3dcb5db8087`, with exact base `4da737ab67dfb54c60eada103e41f6869093c727`. Published source includes the R3 editor, compiler/bundle work, M1a pure context/proposal core and bounded M1b local transport library. The follow-on source on this branch adds a portable session-custody service and reviewed native lifecycle contract; native exact-payload consent, commands and UI remain unimplemented. Use startup commands for current branch and working-tree truth, and the [development ledger](development.md) for canonical validation/publication receipts. The parent directory is a separate marketing site; do not mix its history, files or deployment workflow with this application repository.

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
10. [Model assistance](model-assistance.md), [M1a core](model-context-core.md) [local transport](model-local-adapter.md) and [native session](model-native-session.md) — advisory, pure-byte, exact loopback and request-custody contracts.
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

For core Rust source changes:

```sh
rustup run 1.85.0 cargo fmt --all -- --check
rustup run 1.85.0 cargo test --workspace --locked
rustup run 1.85.0 cargo clippy --workspace --all-targets --locked -- -D warnings
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

PR22 publication gates passed; its exact merged-head receipt and earlier corrected CI failures are in the canonical ledger. The published M1a source implements a pure bounded packer, fixed task templates and strict advisory response validation, with independent canonical fixtures and negative tests. A source-only local transport slice now follows the [local adapter contract](model-local-adapter.md): immutable numeric-loopback profiles, exact final-payload binding, direct HTTP/1, process-wide one-flight, deadlines/cancellation and bounded strict responses. The `rangoon-model-session` foundation on this branch now owns session-only profile/request custody, saved-ID resolution and freshness/lifecycle helpers. No native model command, credential custody, consent UI, tokenizer or proposal-apply path exists yet. Use the ledger for its exact validation/publication state; this is not completed M1b or V1.

1. Read [model-assistance.md](model-assistance.md) and [model-context-core.md](model-context-core.md), then the latest development-ledger entry. Preserve exact selected text, required protected intervals, per-input aliases, omitted-range accounting, closed schemas and `authority: none`.
2. Finish exact-head publication gates for the independently reviewed session foundation, then freeze exact per-outcome IPC schemas and implement native command/dialog integration against [model-native-session.md](model-native-session.md). Reuse its ID-only resolution, retained single-use requests and generation/cancellation leases. Build an explicit native exact-payload decision before any source request. Profile changes, deleted inputs and stale expected state must invalidate sending/applying. Preserve direct numeric-loopback bounds and use disposable synthetic servers; do not treat a pack/profile hash as consent or authenticated endpoint identity.
3. Keep qualified cloud adapters, all-three-OS secret custody, payload/proposal UI and task-quality/token measurements as required M1c–M1e work. No private/paid calls, existing credentials, automatic transfer, silent fallback, model installation/launch or LNSAT activation.
4. Preserve bundle exact bytes, plaintext disclosure, exclusive creation, the 295,016-byte limit and uncertain partial-write recovery. Keep target-loader behavior, collision/write GUI, composition compile, complete keyboard coverage and Windows/Linux GUI as separate open gates.
5. Record local evidence in `docs/development.md`; publication receipts require exact head/base, independent review, hosted CI and fetched ancestry.

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
