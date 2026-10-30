# Target loader discovery and qualification boundary

Status: discovery complete; target loader qualification remains open.

Latest identity recheck: 2026-10-25, by metadata and byte hashing only. Installed Codex changed to `0.164.0-alpha.11.1`; historical identities below retain their original dates. TLQ-1's selected macOS synthetic canaries pass, but no installed target process was launched and target-loader qualification remains open. The separate October 16 version-only invocation attempted a denied local write, as described below.

Authority: [Product-value priority continuation](intent.md#product-value-priority-continuation--october-7-2026) and [export handoff](export-handoff.md). This document records discovery evidence for a later qualification packet. It does not establish runtime support, instruction adherence, offline behavior, or released-OS support.

## TLQ-1 isolation prerequisite — October 25, 2026

Authority: Jeff requests starting the target-handoff/readback phase. The [accepted bounded continuation](intent.md#target-readback-prerequisite-continuation--october-25-2026) opens qualification tooling and synthetic isolation tests only. This does not accept an installed-target runtime launch. The [development receipt](development.md#tlq-1-target-isolation-prerequisite--october-25-2026) owns results, review and exact ownership.

### Refreshed candidate identity and source

Read-only inspection now identifies installed Codex metadata `0.164.0-alpha.11.1`, package SHA-256 `2d850da28de4e799ff848e93a769087440b515c85083c40b984560b79a84d140`; the same installed executable path is 260,394,448 bytes, SHA-256 `a04752e4d46a4d89e53e6f61c59c5349a638c8ce172ffbf391fd482b4d902a4e`. Older pins below remain historical evidence. No version/help or target process was invoked.

Official tag `rust-v0.164.0-alpha.11.1` resolves to upstream commit `6905ecede7816e91373e9eabda3f6b0368fc747b`. Public source acquisition inspected CLI entry/dispatch, arg0 startup, core prompt debugging and app-server source. At this source pin, CLI `main` calls build initialization and `arg0_dispatch_or_else` before command dispatch. The schema-generation match arm invokes the protocol generator, but arg0 runs dotenv loading and helper-path preparation before dispatch. Its selected path can read the Codex-home .env and create helper directories there. Those operations must be independently blocked or isolated; the startup path is not fully qualified. Prompt debugging still loads configuration and enters `AuthManager`, environment/thread management and `start_thread`. These facts do not establish installed-binary/source correspondence or credential-free startup. Exact fetched files and hashes are local evidence, not a vendored runtime.

Current [official developer commands](https://learn.chatgpt.com/docs/developer-commands#codex-app-server) describe stdio transport and optional experimental schema fields. They also document a separate outbound Code Mode host connection. This is current documentation, not the complete schema or startup behavior of the installed artifact. Preserve the earlier requirement for artifact-bound schema before implementing any real read adapter.

### Source contract and checks

`python3 scripts/qualify-target-isolation.py --self-test` is the only executable mode. It accepts no arbitrary target, executable, path, shell or command argument. On the selected macOS host it uses the exact existing Xcode clang, SDK and `/usr/bin/sandbox-exec`; missing tools or another OS return unavailable without fallback. No dependency install or global toolchain change is needed. Root owns this development tool, tests and synthetic C probe. Application/native/storage/compiler profiles, provider ports and UI are unchanged.

The harness creates a private temporary tree containing a generated readable canary, a protected sibling canary, scratch directory, compiled probe and a Unix-domain test listener. The candidate policy denies by default; it permits only that probe executable, `/System/Library` and `/usr/lib` reads/executable mappings, the exact generated input and scratch operations. Executable mapping is separately restricted to the exact probe and those system-library paths; scratch is not an executable-mapping allowance. It does not allow the broad `/System` tree, user data, network, Mach lookup or child executable paths. After two pre-main probe failures, read-only inspection identified Apple's `dyld-support.sb` as the OS's explicit process-bootstrap rules. The candidate imports only that exact file, pinned to SHA-256 `06215a5d32689aefe395c29710e182eb54ba22162f50df8b4842290f8a19bf1c`; unknown bytes return unavailable before compilation. This reviewed OS dependency adds cryptex read/map paths and their ancestors, root-directory read, named dyld syscalls and file-signature/path fcntls. It adds no network or Mach lookup grant. Its hash must remain unchanged through the run. These are private OS rules and not a stable cross-version contract; no broad `system.sb`/`bsd.sb` import is permitted. No HOME/CODEX_HOME variable is set or replaced; child environments are a fixed minimal allowlist. No ambient secrets, configuration, stdin or extra descriptors are forwarded. Compilation is a separately bounded trusted-tool operation, not proof of target isolation.

Seven synthetic checks require permitted read and write to succeed, and sibling read/write, loopback TCP connect, Unix-socket connect and `/usr/bin/true` execution to fail with EPERM/EACCES from the probe. Each result must include the exact fixed startup marker, expected exit, closed streams and completed owned-process drain; a sandbox parser/launcher failure cannot count as an expected denial. The parent verifies that the denied file existed and stayed byte-identical and that permitted output contains the synthetic canary. Receipts retain only fixed outcomes, lengths and tool/source/policy/artifact digests, never file contents, child diagnostics or environment values. Only named generated files are removed; unexpected entries or changed directory identity refuse cleanup.

The supervisor uses explicit argv, no shell, a fresh process group, a monotonic deadline and a combined stdout/stderr byte ceiling. Timeout, output overflow, failed group termination, inherited open pipes and incomplete drain never become completed qualification. Interruption triggers exact owned-process cleanup. This is unmounted test infrastructure, not a new agent executor or application command API. Process-group cleanup is not a general proof against deliberate process/session escape.

### Selected prerequisite result

The final source passes 19 focused supervisor/policy tests. The reviewed hash-pinned dyld-bootstrap candidate then passes all seven actual macOS synthetic checks: both permitted operations succeed, five forbidden operations return the expected denial, every probe emits the fixed marker and drains, blocked fixture remains unchanged and permitted output matches. Exact receipt: `<local-evidence>/rangoon-target-qualification-20261025/selftest-dyld.json`. Earlier two pre-main crashes remain failed runs and are not included in those seven passes. This is bounded prerequisite evidence, not actual target loading or full containment qualification.

### Limits and next gate

The local macOS manual explicitly marks `sandbox-exec` deprecated. Treat it only as a selected development containment experiment, not shipping desktop isolation, a cross-platform backend or an assurance standard. Finite canaries do not prove all filesystem aliases, denied-attempt logging, credential/Keychain access, resource exhaustion, subprocess escape or target startup effects are covered. This slice establishes no target schema, loaded instructions, effective configuration, model adherence or authorization.

After this prerequisite, prepare the exact-version startup/schema capture packet: inspect arg0/build initialization and schema output behavior, freeze a target-specific allowlist and explicit synthetic configuration, qualify complete bounded capture and startup effects, then separately accept target execution. Prompt-input and configuration observations need separate attribution and cannot be merged into an atomic effective-state claim. Real sessions, credentials, network/provider calls and normal instruction directories remain closed.


## Instruction change archive and restore — October 25, 2026

Status: accepted product requirement; AR-1/AR-2 store implementation source-tested and independently reviewed. Native/renderer mounting remains open. Jeff explicitly requires backup/undo for instruction-file edits, secured archive/restore and hashing. This refines the target-handoff sequence: no target-file replacement or automatic instruction edit may ship before its recoverable archive path is qualified. Current bundle export exclusively creates a new selected file and refuses overwrite; it is not an instruction installer and does not already provide external-file undo. Workspace recovery protects saved Rangoon records, not arbitrary originals or external target files.

### User outcome and ownership

The eventual interaction is **Preview change → Save restore point → Apply → Verify**, followed by **History → Preview restore → Restore → Verify**. The common view shows filename, target, time, change summary and verified/pending/conflict state. Exact hashes, archive record IDs and validation evidence belong in details. No encryption key enters the renderer. Archive integrity is separate from content review, engine approval and execution permission.

Rust owns native-selected target references, original-byte capture, authoritative archive availability, logical validation, journal state and commit/recovery decisions. The renderer requests a preview, shows it, and confirms its native single-use handle; it does not supply replacement originals, raw keys, arbitrary database paths or an authoritative expected digest. Reuse the existing Gate/Owner/generation/callback-drain contracts. Do not repurpose an engine port, target session or VUSL profile as the archive store.

### Protected content and change identities

Each immutable restore point binds the exact original and proposed bytes, explicit file state (absent versus present, including an empty file), byte lengths and SHA-256 digests; selected workspace/target identity and relative filename; native parent/file identity; source revision/compiler profile/candidate where applicable; parent change ID, journal phase and observed outcome. Preserve BOM, Unicode, CRLF/LF and final-newline state without normalization. Initial bounds and supported regular-file types must be explicit before capture; an unsupported or oversize original blocks Apply rather than creating an incomplete backup. Restoring creation of a previously absent file means removing that exact created version, not restoring an empty file; that action needs an explicit preview and retained recovery history.

SHA-256 alone is not authentication: someone able to replace an archive and its adjacent checksum can recompute both. Store original bytes and all security-relevant metadata inside the existing authenticated encrypted SQLCipher workspace, using native-owned key custody and complete logical validation. Do not introduce a homegrown cipher, standalone plaintext .bak file, unauthenticated sidecar checksum or discarded beta recovery codec. SQLCipher page authentication detects covered ciphertext modification; it is not author authentication, permission to restore, or proof of current history. A fully valid old database/archive may still be replayed. Independent freshness/rollback detection requires a separately designed trusted anchor or witness; until then report freshness as unverified. An unlocked or compromised host can still read or modify authorized plaintext.

### Durable ordering and safe restore

1. Select and inspect one exact supported target with native custody. Refuse symlinks/reparse points, hard links, moved roots and unsupported filesystems unless that platform path has its own qualified handling. Do not search normal directories or follow imported instruction paths.
2. Capture original bytes and proposed output, validate the complete change, check archive capacity and commit the encrypted restore point before changing the external file. Refuse Apply if secure archival fails or workspace is locked; no plaintext fallback or automatic key regeneration.
3. Retain a crash journal before the external effect. File modification and a SQLite transaction are not one atomic transaction. Record pending/verified/failed/unknown independently, recover interruption by fresh disk inspection, and never replay an uncertain write automatically.
4. Recheck target identity, original state/digest and native preview generation immediately before any replacement. An atomic rename is not an atomic compare-and-swap against other editors. Strong replacement/restore guarantees require qualified exclusive target custody or an appropriate platform primitive; advisory locking and hash rechecks alone do not eliminate an uncooperative writer race. Without that prerequisite, refuse replacement or offer explicitly selected new-file export.
5. Verify final bytes and file identity after the effect, then record a verified receipt bound to the exact change. Failed final readback or lost acknowledgment remains uncertain, never success or an automatic retry.
6. Restore authenticates/decrypts, validates the complete archive, previews exact differences and confirms once. Expected current target must match the recorded applied version under the same custody rules. If someone changed it afterward, refuse blind overwrite and offer inspection/new preview while preserving both versions.

Archives share a visible encrypted-storage budget. Reaching capacity blocks new changes before Apply; it must not silently evict restore points. Explicit archive deletion must show which recovery paths will disappear and preserve referenced history. Portable encrypted workspace backup, strict restore, crash recovery and distinct-process reopening must include every archive/journal record before any new archive tables are enabled. Existing V3 logical backups do not already contain this proposed domain.

### Staged implementation and acceptance

| Stage | Ownership and delivery | Named evidence required |
| --- | --- | --- |
| AR-1 closed domain contract | Root defines immutable archive/change states and pure restore eligibility using existing content identity conventions; no filesystem or authority behavior | Literal exact-byte and absent/empty fixtures; changed original/current/parent identity, unsupported metadata/version, malformed/oversize and unknown-outcome refusals |
| AR-2 encrypted persistence/recovery | Root owns versioned store schema and archive/journal repository, logical backup/restore/deletion and native-key lifecycle integration | Wrong key/ciphertext tamper causes zero mutation; old supported backups remain correctly scoped; full-history backup/restore and distinct-process reopen retain exact archive identities; capacity refusal occurs before external writes |
| AR-3 one target and native custody | Root owns native-selected file references, retained one-shot previews, qualified replacement primitive, journal ordering and readback | Existing/new/empty file, drift, path swap, links, collision, lock/cancel/drain, archive failure, interrupted write, lost result and conservative recovery with positive controls |
| AR-4 restrained history UI | One disjoint UI writer after the authoritative service exists; preserve current library/editor/inspector | Human-readable history/diff, restore preview, conflict recovery and technical details; keyboard/320/768/1440 light/dark; no fabricated Apply/Restore button before native support |

AR-1/AR-2 now has an explicitly authorized store-only continuation. Schema 5 adds immutable instruction changes and an append-only outcome journal, with exact-state inspection and dependency-aware logical deletion. The pure constructors preserve absent/present/empty state, arbitrary bounded original bytes, hashes, selected-target identities, lineage and parent-change links. An attempted event precedes one terminal verified/failed/unknown outcome; unknown never becomes verified through inference or automatic replay. A verified event records exact host-supplied readback identity/digest/length. This is a host assertion validated against the archived proposal, not proof of native custody or authorization.

History save requires an existing keyed SQLCipher workspace and explicitly opts it into schema 5. No normal startup migration or history creation is mounted. Complete schema validation, stale-state identities, authoring, dependency checks, additive/empty-target recovery and encrypted copy comparison include all changes/events. History-aware workspace inventories use `rangoon.workspace-data.v3`; retained restore plans use `rangoon.restore-plan.v3` when any participating workspace/archive schema is 5, including empty history. V3 restore plans also expose `targetWorkspaceSchema`; a schema-only recovery from an empty schema-5 archive still requires the retained write transaction and preserves schema 5 on an absent or empty destination. V1–V4 keep their exact v2 formats. Optional history count fields are omitted only when their derived value is zero; this is a wire convention, not an unassessed count. Existing renderers are not mounted for the new contract. The original V1–V3 logical formats stay unchanged. Protected history uses a private V4 in-memory recovery codec and an opaque Rust snapshot; public plaintext backup export refuses schema 5 and public plaintext backup import refuses V4. Authenticated encrypted-copy inspection is the entry point for protected portable recovery. Internal plaintext buffers are restricted to trusted Rust and zeroized where owned; no secure-memory-erasure or compromised-host guarantee follows.

Current bounds are 256 KiB per original/proposed side, 128 changes, 512 journal events and 16 MiB of serialized history, inside the existing 64 MiB database budget. Capacity refusal precedes a history transaction and cannot silently evict originals. Explicit history deletion reports lost event count and refuses retained child changes. Exact source/revision/composition references prevent deletion of required lineage. Bare checksums remain consistency evidence, not authentication or rollback prevention. Freshness remains unverified without an independent anchor.

Parsing reuses the existing deterministic analyzer; unsupported/binary bytes can be archived without being parsed. Parsed fragments retain exact UTF-8 byte ranges. Existing composition contracts perform source decomposition, saved-revision split and multi-revision merge with copy/authored/replacement pieces, exclusions, duplicates and conflicts. There is no new parser, wire profile or automatic semantic merge in this packet. Tests exercise BOM/CRLF-preserving reassembly and explicit split/merge recipes. Future categorization, similarity detection and optional model proposals remain advisory and must expose uncertainty, conflicts and transformations; they never change originals or substitute fuzzy matches for exact identities.

AR-3/AR-4 remain unimplemented: no external-file Apply/Restore, exclusive custody, retained native file preview, interrupted external-write reconciliation or History UI is mounted. The read-only restore inspection only checks recorded outcome and current host-supplied state; it does not authorize replacement or solve concurrent-writer races. Native mounting must connect the new opaque recovery path before enabling normal target-file changes, qualify complete callback drain/locked behavior and rehearse fresh encrypted recovery. AR-3 must add an explicit durable reconciliation observation after fresh qualified file readback, preserving the prior unknown event without repeating an external write. AR-2 does not yet admit that transition; unknown remains restore-ineligible. The [development receipt](development.md#ar-1ar-2-encrypted-instruction-history--october-25-2026) owns final named checks and review limits.


### Sibling product compatibility review

Read-only local source comparison on October 25 covered LNSAT checkout `6741676496241a3cddf146d7dc28aa6c1a09860b`, the newer locally available LNSAT commit object `605742b39e09bde9c37c6d767e21917e705b3757`, and VUSL checkout `78b5ae41c8c720badd1f2301e1eabd0d07a83d5e`. The newer LNSAT worktree is prunable and lacks source/Git metadata, so that evidence was read through the canonical repository's object store. These are dated local pins, not claims about current public main, a deployed engine or qualified live integration. Neither sibling repository was changed.

| Boundary | Required Rangoon behavior |
| --- | --- |
| LNSAT authorization, consumption, attempts and execution receipts | Keep them distinct from local archive/readback events. An `InstructionPhase::Verified` event means that supplied local readback matched the proposal; it is not LNSAT authorization, an execution receipt, or a statement about downstream enforcement. |
| LNSAT ambiguous execution and replay | Preserve explicit unknown outcomes. Inspection or recovery may not retry an external consequence. The current history store has no executor or automatic retry; native interrupted-file reconciliation remains AR-3 work. |
| VUSL native references and profile-separated canonical IDs | Preserve native system/contract/version/revision/scope and original identifiers in any future adapter. Rangoon's raw-byte SHA-256 and local `instruction-change:`, `instruction-event:` and `workspace:` identities remain separate. Never substitute prefixes, recompute a foreign identity using local JSON, or infer compatibility from equal digest text. |
| Recipe/input links versus generated-artifact proof | Exact linked records and hashes establish which saved context was referenced. They do not by themselves prove that proposed file bytes are the materialized recipe output, a compiler candidate, target-loaded content, or model behavior. Existing explicit composition/compilation records own those stronger derivation facts. |
| Additive restore with an existing owner's newer/different history | Preserve current owner/head semantics. If the incoming archive's pinned revision or recipe cannot resolve in the resulting graph, refuse the restore before mutation. Offer separate inspection or fresh-workspace recovery; do not invent orphan recipes, relax existing composition invariants, or silently omit archive lineage. |

LNSAT source evidence: `crates/lnsat-store/src/phase7_persistence.rs` authorization-attempt records/selectors, `phase7_consumption.rs` authenticated durable reread, and `phase7_git_adapter.rs` receipt/reconciliation/unknown-outcome paths at the listed local commits. VUSL source evidence: `crates/vusl-core/src/canonical.rs` profile-domain-separated hashing and native-reference identity. No shared database, new dependency, engine operation, VUSL profile or protocol alias is introduced here. Live adapter qualification, shared freshness anchoring and cross-product recovery remain separately unassessed.


## Scope

The original `0.159.0-alpha.12.1` discovery pass examined the installed Codex and Claude command-line artifacts and the public documentation or source needed to identify a possible inspection path. In **that historical pass**, no installed target process was launched, including `help` or `version`. No target model call, target authentication, provider operation, project installation, target-runtime network attempt, private configuration/transcript read, or application source change was performed. Public documentation and upstream source were retrieved through read-only tools; this was not a network-free research session. The October 16 identity recheck below is a separate event and does not inherit this no-process claim.

The frozen descriptors `agents_md_v1` and `claude_md_v1` were not changed. Their contents remain outside this report's evidence boundary.

## Target-load milestone — October 20, 2026

Jeff identified actual target loading and surviving configuration as the next distinctive product milestone. Prioritize one attributable source-to-target study over additional disconnected decoder or catalog breadth, while completing the already accepted encrypted-workspace qualification. This reuses the accepted product-value direction; the runtime qualification protocol remains proposed. VUSL adoption and connected LNSAT/OpenShell operations remain independent work, not prerequisites for inspecting existing `agents_md_v1` output.

### Current evidence

GitHub readback confirms [PR #51](https://github.com/hypler-dev/rangoon/pull/51) merged at `9bd9be17d17fd8f778ec88f8e9b9f41df5c01dc4`. Its [Source checks run 39381341599](https://github.com/hypler-dev/rangoon/actions/runs/39381341599) passed on that exact commit: three Rust jobs, three desktop source/test/build jobs and one preview/review-artifact job. Native executables were built without installers. These jobs do not qualify a real target loader, GUI behavior, or this different mixed local worktree. PR #51 remains a documentation change, not VUSL runtime integration.

Metadata and byte hashes still identify local Codex `0.163.0-alpha.10.2`: package SHA-256 `558c7aabb05ccc87bdb8c8514f87659fec72929a39919c7156f8b8889f7589b3`, executable size 249,954,368 bytes and SHA-256 `1afb28c01d7d49e892bd75f11076b10f1451e34e0ee308a95e5ab7f69f9c84fe`. No target process was launched. The official version-matched tag `rust-v0.163.0-alpha.10.2` resolves through unsigned tag object `d9151c94a70013f176ff370fa93da02734662e41` to source commit `4ebc485ca321243727539187169425fcf972f2c4`. A matching version does not establish that installed binary's correspondence to source.

Official documentation describes experimental [`codex debug prompt-input`](https://learn.chatgpt.com/docs/developer-commands#codex-debug-prompt-input). At the immutable source pin, [CLI construction](https://github.com/openai/codex/blob/4ebc485ca321243727539187169425fcf972f2c4/codex-rs/cli/src/main.rs#L1995) builds configuration/authentication and installs Git-attribution and skills extensions. [Core prompt debugging](https://github.com/openai/codex/blob/4ebc485ca321243727539187169425fcf972f2c4/codex-rs/core/src/prompt_debug.rs#L26) initializes authentication, installation identity and environment/thread infrastructure, then starts a thread. It constructs a prompt without entering `run_turn` and returns `prompt.input`, not an effective-configuration dump. Ephemeral mode and disabled API-key environment loading do not prove that credentials, hooks, writes, subprocesses or network activity are absent. These facts justify isolation review, not immediate invocation.

### Separate proof levels

| Level | Required evidence | Permitted claim |
| --- | --- | --- |
| Exported | Verified exact artifact, revision and compiler profile | File generated; loading unknown |
| Placed | Exact bytes in explicitly selected disposable target, with collision/drift checks | Artifact placed; loading unknown |
| Target input observed | Pinned target's own input report, bound to case/configuration/working directory | These instructions appeared in this observed input |
| Effective configuration assessed | Native evidence for relevant layering, overrides, omissions and limits, with explicit gaps | Only the assessed scope and preserved/lost content |
| Enforcement assessed | Separately qualified runtime/authority checks and outcomes | Only tested enforcement coverage |

A disk digest, successful exit or `prompt.input` does not establish full effective configuration. Included instruction text does not establish model adherence. “Constraint survived” must distinguish preserved instruction content from enforced controls. LNSAT and other providers retain their authorization/enforcement responsibilities; equal IDs or digests across engines do not establish an authority edge.

### Complementary native readbacks and the next gate — October 20, 2026

Current official [app-server documentation](https://learn.chatgpt.com/docs/app-server) describes `config/read` as resolved on-disk configuration and `configRequirements/read` as managed requirements, with `null` permitted when none are configured. Its [app-settings example](https://learn.chatgpt.com/docs/app-server#config-rpc-examples-for-app-settings) uses `includeLayers: false`; it does not establish the installed version's complete parameter schema or layer ordering. These are candidates for configuration inspection alongside prompt-input inspection, not interchangeable evidence or current Rangoon integrations.

| Candidate observation | Contribution to the study | Remaining gap |
| --- | --- | --- |
| `debug prompt-input` | Target-native instruction content presented in the captured input | Does not report every resolved setting, prove adherence, or establish enforced controls |
| App-server `config/read` | Target-native resolved configuration within the returned schema and invocation scope | Does not by itself prove AGENTS.md inclusion, a thread's effective state, or enforcement |
| App-server `configRequirements/read` | Target-native report of managed configuration requirements | `null` is a reported absence, not permission, an engine allow decision, or proof of unrestricted execution |

Read-only installed-bundle inspection found the pinned manifest and executable, but no standalone app-server TypeScript or JSON Schema asset among the bundle's JSON/TypeScript files. Compiled `config/read`, `configRequirements/read` and configuration-type strings were present. Strings establish neither callable methods nor schema, semantics, precedence or installed-binary correspondence to upstream source. Three absent guessed instruction-read method names do not prove all instruction readback is unavailable. This bounded search does not cover external packages or private user configuration, neither of which was read.

The next qualification design must resolve these gates in order:

1. **Exact protocol shape:** obtain and review an artifact-bound schema for the pinned target before implementing a reader. Current documentation describes version-specific `app-server generate-json-schema` output, but generation executes the target. It is a candidate for a separately accepted disposable process packet, not an approved command here. First inspect the exact-version dispatch/startup path and proposed isolation. Do not invent request fields, defaults, missing methods or layer order from current docs or binary strings.
2. **Independent startup containment:** select and qualify OS-enforced network and ambient credential/configuration/hook isolation, bounded lifetime/output and filesystem-effect capture before schema generation, app-server startup or prompt debugging. No isolation mechanism has been selected or qualified. Do not repurpose `HOME` or `CODEX_HOME`, borrow the active session, or treat target sandbox flags and a read-method allowlist as startup containment. Current docs describe an app-server local Code Mode host; a read-only request alone cannot establish a side-effect-free process.
3. **Minimal observation exchange:** after exact schema and process scope are accepted, permit only required initialization and the explicitly reviewed configuration reads, plus the separately qualified prompt-input observation. No configuration writes/imports, authentication flow, thread/turn request, model/tool/MCP request, process command or provider operation is included in this proposed configuration exchange. The separate prompt-input path itself starts thread infrastructure, as recorded above; it needs its own startup review rather than inheriting the exchange's request limits. Unsupported methods, malformed/oversized or incomplete output, timeout and cancellation remain failed or unassessed, without a broader-command fallback.
4. **Attributable combined result:** retain each native observation and its exact configuration, working directory, artifact and capture scope. Configuration and prompt observations from separate processes are not one atomic snapshot. Freeze the selected synthetic inputs, hash them before and after each observation, and retain any unobserved or mutable state as a gap. Equal file hashes, IDs or timestamps cannot silently promote partial observations to a complete effective-state or cross-runtime authority claim.

This changes the proposed first runtime step: do not invoke prompt debugging or start a server on the assumption that it is an offline loader. Resolve the exact schema and startup-containment design first. The existing ten study cases still apply; add schema/version mismatch, absent versus `null` requirements, differing configuration between observation processes, startup side effects and incomplete capture to their concrete fixtures. No target process, schema generation, native RPC, normal configuration/authentication/session read or runtime implementation occurred in this research. The [feasibility receipt](development.md#target-readback-feasibility--october-20-2026) owns the evidence and review status.

### First study and acceptance cases

Use one synthetic source, one saved/reviewed Rangoon revision, frozen `agents_md_v1`, its exact export and one pinned Codex artifact. Link those records to target-native observation; do not substitute a Rangoon-written loader imitation. Claude and other targets require their own inspection/isolation review.

The source/runtime packet remains proposed. Root owns isolation, artifact identity, privacy and claims. Before launch, establish independent OS-enforced network and ambient credential/configuration/hook isolation, bounded process lifetime/output and captured filesystem effects. Do not repurpose `HOME` or `CODEX_HOME`, reuse the active session, or assume application-level sandbox flags contain the target's startup. Missing isolation or observation stops qualification at `unassessed`; no model call substitutes for inspection.

The first corpus must cover these cases; none has run:

1. Exact exported root file appears in target-reported input.
2. Root/nested instructions and override exclusion follow the target's documented precedence for the selected working directory.
3. Empty, comment-only, UTF-8 and line-ending variants retain target-native behavior.
4. Configured size boundaries expose truncation, omission or refusal without false preservation.
5. On-disk changes after export refuse an exact-export identity claim; changes after observation require fresh target readback.
6. Same basename in another root cannot satisfy selected-target linkage.
7. Missing, malformed, unsupported, cancelled or failed inspection remains failed/unassessed, never inferred success.
8. Matching artifact text with unobserved layered configuration remains partial observation.
9. Forbidden startup access fails isolation even if useful JSON appears; absent instrumentation remains unknown.
10. Foreign-engine matching IDs/digests do not create a cross-runtime authority or execution relation.

Each case receipt binds source revision/profile, exported/on-disk bytes, target version/executable digest/OS, invocation and working-directory scope, explicit target configuration, native observation and capture coverage, expected/observed order/content, omissions/diagnostics, exit/cancellation and independent isolation/filesystem observations. Retain only synthetic evidence inside the accepted disposable root; do not collect normal sessions, credentials or private instructions. Record unknowns rather than completing them from UI state.

Next safe action: exact-version startup/isolation design review, then separately accepted harness and disposable run. No runtime, portability, adherence, three-OS target support or release qualification follows from this research. The [development receipt](development.md#target-load-milestone-and-public-evidence-recheck--october-20-2026) owns exact evidence and review state.

## Installed artifact evidence

| Target | Observed evidence | What it proves | What it does not prove |
| --- | --- | --- | --- |
| Codex | Installed bundle metadata identifies `codex-package.json` version `0.159.0-alpha.12.1`, target `aarch64-apple-darwin`. Metadata SHA-256: `663d734fec63c905ce4f4ed360639a45d9c0511b25181a038c0be7ce6be3b798`. | A specific installed Codex package identity was recorded. | Loader presence, startup isolation, instruction loading, model adherence, or runtime compatibility. |
| Codex executable | `/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex`; SHA-256: `1180e2d56ea06ec583092acd933345685da3441cb1769a436d76dbf320613e75`. | A specific executable path and digest were recorded. | That executing it is safe, offline, read-only, or equivalent to current upstream source. |
| Claude | `~/.local/bin/claude` is a symlink to `~/.local/share/claude/versions/2.1.92`; target is Mach-O arm64, `198736976` bytes; SHA-256: `6d1b9657727dce81332b3cda11bfe0a8c83e2392e3c062a31022e10b0e71cdd1`; signature identifier `com.anthropic.claude-code`. | A specific installed Claude artifact identity was recorded. | Runtime version semantics beyond PATH naming, standalone loader behavior, offline safety, or instruction adherence. |

Claude `2.1.92` is an identity observed from the versioned PATH target name. It is not a runtime version result. No adjacent package JSON or JavaScript loader was identified in this discovery evidence.

## October 16 candidate recheck; still no loader qualification

The first proposed target-load study should use the existing `agents_md_v1` compiler profile and a **disposable Codex project**, because Rangoon already produces exact `AGENTS.md` bytes and a Codex executable is present locally. This selects a study candidate, not a supported target, an installation action or a new format. The accepted local source-to-export task can proceed without this study or any provider.

On October 16, metadata inspection found `/Applications/ChatGPT.app/Contents/Resources/codex-cli/codex-package.json` reporting version `0.163.0-alpha.10.2`, SHA-256 `558c7aabb05ccc87bdb8c8514f87659fec72929a39919c7156f8b8889f7589b3`. The executable at `/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex` was 249,954,368 bytes, SHA-256 `1afb28c01d7d49e892bd75f11076b10f1451e34e0ee308a95e5ab7f69f9c84fe`. `codex --version` printed `codex-cli 0.163.0-alpha.10.2` and warned `WARNING: proceeding, even though we could not create PATH aliases: Operation not permitted (os error 1)`. This command attempted a PATH-alias side effect despite being version-only; the sandbox denied that attempt. No loader command, model turn, project installation or instruction readback was requested. Credential access and network behavior were not instrumented, so their absence is unproved. This new binary is different from the earlier discovery pin and needs its own isolated startup qualification; do not infer its behavior from upstream `main` or the older artifact.

The candidate proof must separate three questions: whether exact exported bytes can be placed into a user-selected disposable target; whether that pinned target reports which artifact and layered instructions it **effectively loaded**; and whether any later runtime event can be source-qualified to that loaded state. File placement or matching on-disk digest answers only the first. A matching target-reported artifact digest still leaves effective layered configuration and instruction coverage unknown unless the target exposes verifiable readback. If no isolated readback path exists, record `unassessed` and keep the cross-system link unclaimed; do not substitute a paid agent/model call or an inference from observed behavior. Review the existing qualification protocol below before any target process is run again.

## Codex inspection-path finding

Official documentation describes AGENTS.md discovery from repository root toward the current working directory, override priority, and a default aggregate limit of 32 KiB: <https://learn.chatgpt.com/docs/agent-configuration/agents-md>. This is documentation evidence only; it is not proof that the installed binary implements or exposes a safe standalone loader.

Official developer-command documentation lists experimental `codex debug prompt-input`: <https://learn.chatgpt.com/docs/developer-commands>. It is a candidate inspection path, not a qualified path.

Public mutable upstream source reviewed for context shows `main.rs` building config, using `AuthManager::shared_from_config`, and installing git-attribution and skills extensions (lines 1907–1998): <https://raw.githubusercontent.com/openai/codex/main/codex-rs/cli/src/main.rs>. Upstream `prompt_debug.rs` resolves an installation ID, builds an environment manager and thread manager, starts a thread, and builds a prompt without `run_turn` (lines 23–111): <https://raw.githubusercontent.com/openai/codex/main/codex-rs/core/src/prompt_debug.rs>.

Therefore, upstream `codex debug prompt-input` is not established as a pure, isolated, read-only loader for the installed `0.159.0-alpha.12.1` binary. It is also unsafe to infer installed behavior from mutable upstream source. This report does not claim that the command is absent, that it invokes a model, or that startup isolation is impossible; those points remain unaudited.

## Claude inspection-path finding

Claude's official memory documentation describes memory visibility within a session: <https://code.claude.com/docs/en/memory>. The CLI reference documents `-p`, `--bare`, and `--no-session-persistence`: <https://code.claude.com/docs/en/cli-reference>. The documented flags do not establish a comparable standalone, offline loader inspection path. `--bare` skips `CLAUDE.md`, so it cannot qualify target-file loading. `-p` and `--no-session-persistence` still involve session-path behavior. Current documentation also cannot be applied as proof of behavior for the installed `2.1.92` artifact.

## Qualification result

Loader presence and model adherence are separate claims. Current result:

- Codex loader presence: unknown; qualification untested.
- Codex instruction loading and adherence: unknown; qualification untested.
- Claude loader presence: unknown; qualification untested.
- Claude instruction loading and adherence: unknown; qualification untested.
- Offline, credential-free, config-isolated, hook-isolated, filesystem-read-only startup: unknown for both targets.

No target compatibility claim may be promoted from this discovery report. Rangoon's own filename parser, portable bundle verifier, or source-level analysis cannot substitute for native target-loader qualification.

## Proposed future qualification protocol

This protocol is proposed, not accepted. A later packet must pin the exact executable, version, digest, OS, and invocation. It must use an independent OS-enforced environment that blocks network and credentials, isolates configuration and hooks without repurposing `HOME` or `CODEX_HOME`, and captures all writes. The protocol must use synthetic project roots and subdirectories containing root instructions, nested instructions, override files, imports, comments, encoding variants, boundary sizes, and malformed inputs.

The observation record must distinguish discovered, loaded, omitted, overridden, imported, truncated, and rejected bytes; diagnostics; exit status; filesystem effects; network attempts; and credential/config reads. It must include historical replay against frozen descriptors and test macOS, Windows, and Linux separately. A loader can be present while model adherence remains unqualified. Failure to find a safe standalone loader is an explicit gap; invoking an agent or model is not a substitute under the export-handoff packet.

The existing [model-proposal inspection and application sequence](model-proposal-inspection.md) and useful local workbench may proceed independently. Resume at M1d-2 native retained inspection, preserving M1d-1's typed validated response and bounded immutable inspection instead of reconstructing provenance from renderer JSON. Durable application must still follow the existing M1d-3/M1d-4 storage, recovery, conflict and explicit-save gates. Target compatibility claims remain gated on separately accepted exact-version qualification and fresh evidence; this proposed protocol is not a newly imposed local-editing release prerequisite.
