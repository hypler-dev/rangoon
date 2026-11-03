# Review Rangoon now

Rangoon is ready for **guided source and design review**. This is a development
preview, not a desktop beta or a live governance service. The fastest useful
feedback concerns whether a developer can understand the local capability
workflow and distinguish saved content, reported evidence and authority.

Current review focus: the [October 29 audit](system-audit.md#october-29-whole-application-viability-and-drift-audit) prioritizes one repeatable local instruction workflow and accurate storage/compatibility claims. Review saved versus unsaved content, exact historical revisions, the new Skills-to-Compile handoff, recovery and refusal behavior. The old parser is inert historical source; VUSL adoption and live OpenShell/LNSAT operations remain separate unqualified work. Synthetic adapter tests do not establish connected support.

## Choose a review lane

| Lane | What to do | What the result can establish |
| --- | --- | --- |
| Design and usability, about 20 minutes | Run `npm start`; open `http://127.0.0.1:4377`. Compare the sample screens with the real `analyze.html` workbench. Try navigation, dark/light, narrow width, workflow editing and the Operations empty state. | Clarity, hierarchy, terminology, recoverability and visual fidelity. Sample counts and playground behavior are synthetic. Browser mode cannot save native records or inspect a selected local operations file. |
| Source and architecture | Read the [current audit](system-audit.md#october-29-whole-application-viability-and-drift-audit), [architecture](application-architecture.md), [feature matrix](../README.md#product-and-feature-status), [OpenShell boundary](provider-interoperability.md#openshell-runtime-boundary-and-adapter-sequence) and [development evidence](development.md). Run `npm run build:check` and the relevant Rust package tests if desired. | Contract and implementation critique. Passing source tests do not establish installed desktop support, enforcement, certification or model accuracy. |
| Guided native task on disposable data | Use an explicitly prepared isolated qualification artifact and synthetic fixture, with its own manifest, app-data location and test-key custody. Select, save, derive, revise, review, open the saved revision in Compile, export and reopen after restart; retain the exact stopping point. Do not substitute an ordinary personal-profile launch. | A task observation for that exact artifact and machine. Default source uses Managed encryption and recovery has dated macOS evidence, and the folder-picker harness now has source-level permission/fixture preparation; a fresh artifact and actual GUI observations are still required. No private projects, normal keys, provider credentials or production data. |

No LNSAT, OpenShell, model account or provider credential is needed for core
content work. Do not install an authority engine or configure a live gateway
for this review. The Operations route reads one explicitly selected local
report and displays **unverified observations**; it is not a live event feed,
approval inbox or execution controller. Provider comparisons describe
candidate abilities and unavailable states, not connected support.

## Disposable native workflow checklist

The October 30 NQ-1 changes prepare the existing macOS qualification runner; they do not record a native task pass. Use exclusive, reliable UI control and a new run from the exact reviewed source. Windows testing is deferred until the Proxmox test host is available; Windows encrypted custody still requires implementation.

1. Build with pinned Rust 1.99.1 and offline dependencies using the existing `scripts/qualify-native-encrypted.py init` entry. Keep the resulting source/executable/manifest hashes and OS/architecture. Its `folderFixture` output names the only folder for this rehearsal. Manifest v3 rejects old runs; never edit an older manifest to reuse it.
2. Start the `authoring` phase with `run NONCE authoring`. Complete isolated Managed setup using the disposable app's own custody. Record each confirmation and expected cancellation; normal user workspaces and keys remain outside this test.
3. Open Agent files and cancel Choose folder first. Verify the current selection/results remain intact. Choose the exact printed `folder-fixture`: expect only `AGENTS.md` and `skills/review/SKILL.md`. Hidden `.git/ignored.md` and `notes.txt` must not appear. Check relative names, sizes, observed timestamps and complete-list status. Metadata selection must not expose content or create saved source records.
4. Select/filter a row, then Lock. Verify folder rows, selected metadata, timestamps and private filter/inspector state are cleared. Unlock must not revive previous metadata. Also test Lock while a picker/result is pending; late completion must not restore rows. Capture actual UI/accessible-tree evidence alongside native lifecycle observations.
5. Use the separate explicit file picker to inspect a synthetic Markdown file, then Import & Analyze's explicit selection to save a source. Derive a skill, revise it, inspect historical/current revisions and review the chosen revision. Open that exact saved revision in Compile; unsaved changes must request save/discard without losing another draft.
6. Complete a composition from saved references, inspect its source coverage and resulting revision, then compile/export. Verify selected format and exported bytes through the existing explicit inspection flow. Export produces readable files; do not use private instructions. An export receipt proves output creation, not that an external runtime loaded or obeyed it.
7. Lock, end `authoring` cleanly, then restart the app using the existing `relaunch` phase. Reopen the saved records and confirm their content, revision pins and lineage. Exercise encrypted backup/restore on a separate fresh disposable destination through the existing recovery procedure and retain its artifact-bound evidence. Workspace recovery is separate from undoing changes to external project files.
8. End each phase cleanly, retain its parent receipt and exact UI stopping point, then perform the existing isolated slot cleanup. `phaseExitVerified` and `folderFixtureVerified` describe process/input checks only. Keep the picker/Lock/full-journey verdict unassessed until each observation above is recorded. Never turn a missing observation, cancelled test or interrupted controller into a pass.

The runner accepts fixed controls only; it does not automate these UI actions. Timeout remains 20 minutes per phase, with existing event/output bounds. The fixed folder is outside all workspace case roots and is never an instruction execution target. CUA currently fails before launch because of the configured symlinked writable root; resolve controller access before starting a vault-backed native rehearsal.

## Feedback that changes the build

For each observation, include route, platform/browser, task, expected result,
actual result and whether data was sample, unsaved, saved or reported. These
prompts are most valuable:

For ordinary product, usability and reproducible source feedback, open a
[Rangoon GitHub issue](https://github.com/hypler-dev/rangoon/issues/new) with
those details and disposable example data. Issues are public, so omit private
projects, credentials and provider payloads.

1. Could you find the real import-to-export path without mistaking the sample
   Command Center or playground for saved work?
2. Where did a label, blocked action, revision or source origin need more
   explanation? Did an action risk losing an unsaved draft?
3. Can you tell when a graph edge edits a workflow definition versus when a
   runtime action has actually been authorized and attempted?
4. Which step in select → save → derive/revise → compile/export → reopen took
   longest or failed? What exact file/target was involved, using only disposable
   test content?
5. Does the Operations view make missing, conflicting, unknown and unverified
   evidence visible without implying that an allow or approval caused an
   effect?

Review against the eight original screen references through the
[product-experience mapping](product-experience.md#eight-reference-comparison).
Those images establish visual direction, not working integrations or real
metrics. Report a suspected security or privacy issue privately to the project
owner; do not post private content or exploit details in a public issue. A
dedicated security-reporting policy is still needed before broad public review.

Broader hands-on desktop testing still requires a repeatable local task and
recovery rehearsal against the actual current artifact. October 25 disposable
macOS recovery evidence is dated and scoped; it is not a full current-build
qualification. CW-1 source checks pass, but rendered browser interaction remains
pending because the UI automation runtime failed before opening a page. Windows
startup/recovery is unsupported in current source; Linux GUI/custody and three-OS
package release remain separate gates.
