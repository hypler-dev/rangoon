# Target loader discovery and qualification boundary

Status: discovery complete; target loader qualification remains open.

Latest identity recheck: 2026-10-16. The older installed-artifact table below remains its original discovery snapshot, not the currently pinned executable. The later recheck included a version-only process invocation with a denied local-write attempt, described separately below.

Authority: [Product-value priority continuation](intent.md#product-value-priority-continuation--october-7-2026) and [export handoff](export-handoff.md). This document records discovery evidence for a later qualification packet. It does not establish runtime support, instruction adherence, offline behavior, or released-OS support.

## Scope

The original `0.159.0-alpha.12.1` discovery pass examined the installed Codex and Claude command-line artifacts and the public documentation or source needed to identify a possible inspection path. In **that historical pass**, no installed target process was launched, including `help` or `version`. No target model call, target authentication, provider operation, project installation, target-runtime network attempt, private configuration/transcript read, or application source change was performed. Public documentation and upstream source were retrieved through read-only tools; this was not a network-free research session. The October 16 identity recheck below is a separate event and does not inherit this no-process claim.

The frozen descriptors `agents_md_v1` and `claude_md_v1` were not changed. Their contents remain outside this report's evidence boundary.

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
