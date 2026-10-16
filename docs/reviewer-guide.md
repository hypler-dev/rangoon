# Review Rangoon now

Rangoon is ready for **guided source and design review**. This is a development
preview, not a desktop beta or a live governance service. The fastest useful
feedback concerns whether a developer can understand the local capability
workflow and distinguish saved content, reported evidence and authority.

## Choose a review lane

| Lane | What to do | What the result can establish |
| --- | --- | --- |
| Design and usability, about 20 minutes | Run `npm start`; open `http://127.0.0.1:4377`. Compare the sample screens with the real `analyze.html` workbench. Try navigation, dark/light, narrow width, workflow editing and the Operations empty state. | Clarity, hierarchy, terminology, recoverability and visual fidelity. Sample counts and playground behavior are synthetic. Browser mode cannot save native records or inspect a selected local operations file. |
| Source and architecture | Read the [current audit](system-audit.md#october-16-architecture-and-usability-drift-checkpoint), [architecture](application-architecture.md), [feature matrix](../README.md#product-and-feature-status), [OpenShell boundary](provider-interoperability.md#openshell-runtime-boundary-and-adapter-sequence) and [development evidence](development.md). Run `npm run build:check` and the relevant Rust package tests if desired. | Contract and implementation critique. Passing source tests do not establish installed desktop support, enforcement, certification or model accuracy. |
| Guided native task on disposable data | On a prepared development machine, follow [native source instructions](../README.md#native-desktop-workbench) using the committed `fixtures/contracts/AGENTS.md`. Select, save, derive, revise, review, compile/export and reopen after restart; report the exact stopping point. | A task observation on that machine only. Current production workspace and portable backups are plaintext. Do not use private projects, credentials or production data. |

No LNSAT, OpenShell, model account or provider credential is needed for core
content work. Do not install an authority engine or configure a live gateway
for this review. The Operations route reads one explicitly selected local
report and displays **unverified observations**; it is not a live event feed,
approval inbox or execution controller. Provider comparisons describe
candidate abilities and unavailable states, not connected support.

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

Broader hands-on desktop testing should wait for one repeatable, documented
local task on disposable data, encrypted-workspace adoption or a deliberately
plaintext test profile, native recovery evidence and platform-specific GUI
checks. Release support for macOS, Windows and Linux has a separate gate.
