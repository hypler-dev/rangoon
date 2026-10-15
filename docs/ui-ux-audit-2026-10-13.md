# Rangoon UI and UX audit — October 13, 2026

## Decision summary

Rangoon has a useful visual and interaction foundation. The flat workflow editor and the recently compacted module catalog already express the original library → work area → inspector concept well. Keep that direction. The highest-value work is to make the real workspace easier to find, protect edits consistently, and explain blocked actions at the point of interaction.

The product currently feels like several related applications: a sample dashboard, a native workbench, a synthetic workflow editor, and a flow simulator. This fragmentation creates more friction than the artwork does. A new visual framework, a larger demo catalog, or a default 3D editor would not resolve it.

This is an audit and recommendation, not an accepted redesign or implementation receipt. The user requested a full UI/UX audit and improvement suggestions. The existing [product experience record](product-experience.md), [architecture](application-architecture.md), [visual contract](vision-contract.json), and [implementation plan](plan.md) remain authoritative. Existing UA findings retain their IDs. New observations below are report-local; they do not silently change the canonical backlog.

## Scope and evidence

- Repository: `app-review`; branch `codex/approval-duty-inspection`; HEAD `8140689303737a6bd163f5190a7cf6910596915b`. Startup index was empty; the branch was eight commits behind the existing remote-tracking reference. No fetch was performed.
- The current mixed working tree was audited, not only committed HEAD or published main. All 548 pre-existing file identities were recorded before work; this report is the only intended repository addition.
- Reviewed all eight original screenshot references, nine sample routes, eleven workbench routes, the workflow playground, and the flow companion. Source inspection covered task entry, search, skill drafts, editor state, menus, view semantics, theme handling, and unavailable-state behavior.
- Three independent native OpenAI lanes contributed: `gpt_reviewer` / GPT-5.6-Terra xhigh for journeys and state; `gpt_ui_design` / GPT-5.6-Terra high for visual design; `gpt_ui_review` / GPT-5.6-Terra xhigh for accessibility. The root controller integrated the audit and performed browser checks. GLM was skipped; no external-provider payload was sent.
- The normal browser surface measured 837 × 523 CSS pixels despite a requested larger override. It was used for actual navigation, search, sample loading, pending edits, invalid connections, theme changes, and Outline fallback.
- A separate loopback fixture served 88 source-identical preview files. Same-origin iframe wrappers established actual child widths of 320, 768, and 1440 CSS pixels; the outer display was scaled only to fit the browser panel. Thirteen route/width cases were captured in each theme: seven routes at 1440 and analysis/modules/workflow at 768 and 320. Production preview CSP was not changed. Fixture-only framing permission was local; provider connections remained disabled.
- Native populated workflows, screen-reader operation, OS dialogs, vaults, user databases, provider connections, performance under large real datasets, and full WCAG conformance were not qualified. Browser refusal of native operations is expected and does not mean the native implementation is absent.

Local audit evidence: `/var/folders/v2/k6_v09bj1bg4q9szfz8573800000gn/T/rangoon-ux-audit-20261013-0c60iaak/`. This is temporary evidence, not a durable release artifact. Important receipts include `startup.json`, `normal-observations.json`, `fixture-matrix.json`, `pending-edit.json`, `theme-observation.json`, `menu-exit.json`, and the corresponding JPEG captures.

## Findings to fix

P2 denotes meaningful user friction or recoverability risk; P3 denotes a smaller interaction defect. Design priorities elsewhere in this report are not incident severities. No P1 was confirmed in the reviewed paths.

| Finding | Evidence and consequence | Recommendation / acceptance | Effort |
| --- | --- | --- | --- |
| **UA-01 / P2 — real workspace entry disappears** | Command Center's primary Import action enters the sample workflow. Before loading a sample, `Open desktop file analysis` exists; afterward it disappears. `preview/app.mjs:82,182`. Root reproduced both states. Users can mistake the sample route for the only import workflow. | Keep a persistent real-workbench entry in initial, populated-sample, and empty states. Identify samples once, explicitly. From every entry state, a new user must reach actual local import without visiting a playground. | Small |
| **UX26-01 / P2 — unsaved skill edits lack the shared exit guard** | `preview/skills-model.mjs:248` retains revision drafts in memory; `skills-view.mjs:51` identifies unsaved changes. `analyze.mjs:716` checks workflow and composition drafts before unload, but omits skills. Source-confirmed; populated native GUI reproduction remains pending. | Expose one dirty-state query covering all skill drafts, including drafts hidden after selection changes. Include it in exit protection; save/discard must clear the relevant state. Test editing, switching records, then reload/close. | Small–medium |
| **UA-03 / P2 — pending edits silently block node selection** | Select Check, change its title, then select Input: Check stays selected and status remains generic. Cancel restores Input selection. `workflow-model.mjs:201` correctly guards selection; the playground render branches in `workflow-view.mjs:83–84,254–266` omit the pending-edit explanation. Root reproduced this. | Show “Unsaved field changes — Apply or Cancel before selecting another node” beside the editor, with reachable controls. Retain the guard. Test rendered feedback, not just model rejection. | Small |
| **UX26-02 / P2 — search promises records but searches pages** | The search control advertises assets, workflows, and evidence (`preview/app.mjs:54`), but results filter only route names (`:141–147`). Searching `Security Review` produces “No matching page” despite the sample skill being present. Root reproduced this. | Immediately label the existing function “Go to page…”. Add actual scoped record search in a separate packet. Later results should identify type, workspace, revision, and location. | Small for label; medium for record search |
| **UX26-03 / P2 — composition view buttons have incomplete selection semantics** | Source/Recipe/Graph buttons use `aria-selected` without the associated tablist/tab/tabpanel contract; handlers are click-based. `preview/composition-view.mjs:214,409`. Source-confirmed. Tab activation is possible; the issue is the missing announced selection/view relationship, not total keyboard inaccessibility. | Use either ordinary pressed view buttons with `aria-pressed`, or complete tabs with correct relationships and keyboard behavior. Verify selected state and panel naming with a screen reader. | Small |
| **UA-05 / P3 — authority theme label stays stale** | Root observed theme light → dark while the button remained “Dark mode.” The persistent authority render in `preview/analyze.mjs:433–452` omits the theme-label update performed by other routes. | Refresh the shared label/state in both directions without losing focus. Add a mounted-authority-route regression check. | Small |
| **UX26-04 / P3 — action menu remains open after Tab leaves** | Workflow Canvas actions → Select → Tab → Close menu → Tab moves focus outside the menu, but the menu stays open. Root reproduced this. Related handlers: `preview/workflow-view.mjs:742–753`, `authority-planner-view.mjs:40,97–99`. Only the workflow instance was browser-reproduced. | Close menus when keyboard focus leaves, preserve normal Tab order, and restore trigger focus on Escape. Verify workflow and authority menus independently. | Small |

Do not count repaired findings as fresh defects. UA-04's shared-dock correction remains in place. UA-02's earlier catalog-below-planner overload has been materially addressed by [I1f-a](development.md#i1f-a-compact-module-workspace--october-13-2026): the compact catalog and inspector are the appropriate baseline now.

## What is working

- The original visual grammar survives: grouped navigation, orange accent, restrained route artwork, useful central workspace, and contextual inspector. The desktop workflow editor is the strongest authoring example.
- Module inspection now leads with a compact catalog. Comparison and planning can be opened deliberately. LNSAT's recommendation is separate from its unavailable adapter state; recommendation must never imply active enforcement.
- Outline exposes graph structure and connection editing without requiring spatial gestures. A self-connection attempt gives the specific refusal “A control edge cannot connect a node to itself.”
- Native-unavailable states are honest. Controls do not invent saved records or successful provider operations in a browser.
- Existing source guards distinguish stale previews, pending operations, uncertain results, and explicit user actions. Preserve these while simplifying presentation.
- The flow scene has an understandable task: inspect a proposed request, policy decision, consumption, and outcome. Its inspector gives the scene meaning. Keep it as optional inspection alongside the flat editor.

## Alignment with the original mockups

| Original intention | Current assessment | Adjustment |
| --- | --- | --- |
| One recognizable application shell | Brand continuity is good; sample, native, and playground shells still imply separate applications. | Converge task navigation and terminology before redoing graphics. Examples remain available under a clear secondary entry. |
| Import → inspect → turn content into reusable skills | The native surface contains these concepts; the sample entry obscures access. | Start with real local work, then show the next relevant action for the selected source. |
| Material-editor style composition | Flat workflow editor and shared dock layout are close to the intended pattern. | Strengthen selection, compatible ports, pending changes, Undo, and exact revision display. Do not rebuild the editor. |
| Skills library plus detail inspector | Structure exists; populated search, organization, and unsaved-state behavior need stronger qualification. | Make find → select → edit → save → reuse the primary journey. |
| Connectors and governance visible in context | Module catalog is improved; provider capability and availability remain separate concerns. | Keep module identity traceable from configuration to workflow and evidence; show per-function limitations. |
| Premium artwork and mascot | Artwork gives personality without needing to carry functionality. | Keep stable, theme-aware route art and small secondary mascot placements; flat symbols for actionable nodes. |
| Live operational visibility | No connected live source was established for this audit. | Provide a clear connection setup/empty state. Keep example playback visibly separate from actual telemetry. |

The original screenshots contain invented counts, activity, compatibility, and compliance examples. They are layout references, not data to copy into the real product.

## Recommended information architecture

Use one workspace identity and one task hierarchy. Avoid making users understand implementation names such as “native,” “synthetic,” or “engine boundary” just to find ordinary work. Put those distinctions in accurate state labels and contextual explanations.

| Primary area | Everyday tasks | Contextual or advanced tasks |
| --- | --- | --- |
| **Home** | Resume recent work; import a source; see items needing attention | Open examples; inspect connection status |
| **Sources** | Choose files; search saved sources; inspect original content | Bounded discovery, provenance, parsing details |
| **Skills** | Find, categorize, edit, save, and reuse skills | Revisions, dependencies, detailed evidence |
| **Workflows** | Build with nodes; connect; inspect; save | Definition view, advanced ports, historical evidence |
| **Connections** | Models, systems, and authority modules | Capability comparison, adapter details, exact versions |
| **Data & settings** | Workspace setup, backup, restore, storage state | Retention, diagnostics, recovery, CLI guidance |

Decompose, merge, split, and compile remain discoverable operations within relevant source/skill/workflow contexts, with direct navigation or command search where useful. This is a proposal, not authorization to remove current routes. Preserve deep links and stable record IDs when changing navigation.

Examples and playgrounds should be secondary learning tools. A sample record must never appear to be a saved local record. Keep the real workspace reachable while exploring examples.

## Screen-by-screen improvements

| Surface | Highest-value improvement |
| --- | --- |
| Command Center | Lead with resume work and real import. The orange primary CTA is already visually distinct at 1440px; do not claim all actions have equal visual weight. Group example/editor demonstrations under a secondary “Explore examples” entry. Replace fixture metrics with real local counts only when wired. |
| Import & Analyze | One clear source action, recent saved sources, and selected-source inspector. Explain file scope before reading. Put parser facts and detailed evidence behind one disclosure. Unavailable operations need one immediate recovery path. |
| Decompose / Merge / Split | Keep selected input and revision visible. Provide an affected-content preview before saving, clear Apply/Cancel state, and accessible non-drag alternatives. Expose advanced recipe/definition editing only when requested. |
| Skills | Search, category/tag filters, stable selection, revisions, and one clear Save action. Resolve dirty-state handling before adding bulk organization. Later include bounded discovery, duplicate suggestions, and reference-aware deletion; do not silently scan devices. |
| Workflows | Preserve library/canvas/inspector. Highlight compatible targets during connection; explain refusals locally. Keep Undo and a consistent selection model across Canvas/Outline/Definition. Reduce repeated synthetic disclaimers to one persistent state plus action-specific warnings. |
| Compile & Export | Present source revision → output target → generated text → export. Separate content inspection from execution authority. Keep plaintext export disclosure at the export decision, not repeated throughout the page. |
| Models | Guide users through endpoint, model, payload review, explicit consent, and result. Keep advanced limits and protocol details under a disclosure. Preserve exact external-call consent and failure semantics. |
| Authority modules | Use the new catalog/inspector as the default. Display capabilities, compatibility, connection state, and evidence separately. Keep Compare and Plan optional. Rename generic meters by what is measured; never turn catalog coverage into a compliance percentage. |
| Data | Make setup, storage state, backups, restore, and recovery understandable before advanced controls. Surface lock/unlock only with the qualified backend. Keep encrypted-workspace adoption and recovery work on the existing technical path. |
| Flow / 3D | Keep camera focus on route or selected stage; no idle motion. Offer Overview, Focus selection, and a flat/Outline fallback. Show only real events as live; synthetic replay remains labeled. The graph definition and evidence identity should be shared across views. |

## Responsive, visual, and accessibility recommendations

Across the 26 controlled route/width/theme cases, no document-wide horizontal overflow was measured. That does not prove every field, dropdown, long label, graph, or translated string fits. At 320px, workflow was 4,072px tall and modules 2,665px. At 768px, workflow was 2,392px. The concern is hierarchy and travel distance, not horizontal reflow failure.

On narrow layouts, put the selected item summary and next action before long configuration sections. Start secondary provenance/configuration panels closed, preserve their state, and provide a quick route back to selection. Avoid hiding essential reasons, status, or consent behind these panels. Test long filenames, long node names, empty libraries, large libraries, and absent selections explicitly.

Use one component vocabulary: consistent field rows, compact toolbars, icon buttons with accessible names, severity/status badges, inspector sections, menus, tabs, and empty states. Keep route artwork stable and outside control hit areas. Use short selection/expand transitions and honor reduced motion; avoid continuous ornamental animation.

No global font-size or contrast failure is asserted from half-scale fixture screenshots. Review typography and contrast at native scale, then adjust actual failing tokens. Keep primary instructions readable and avoid using tiny metadata for a required action or warning. Maintain full contrast for headings, navigation, and recovery text even when the relevant operation is unavailable.

For accessibility acceptance, test keyboard operation, visible focus, meaningful names and selected states, status announcements, non-drag alternatives, contrast, and reflow. WCAG 2.2's AA target-size criterion is 24 × 24 CSS pixels with exceptions; a 44px design target is useful where space allows but is not the AA minimum. A graph can require two-dimensional layout; surrounding controls still need usable reflow. This audit is not a conformance certification. [W3C WCAG 2.2 reference](https://www.w3.org/WAI/WCAG22/quickref/).

Progressive disclosure fits Rangoon: show the core task first, with technical depth available on demand. Do not turn it into an entirely separate “advanced app” with duplicated navigation or hidden recovery. [Nielsen Norman Group: Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/).

## Proposed repair sequence

These are bounded proposals for the next implementation packets. They do not replace the accepted encryption/routing/recovery work or authorize publication.

| Packet | Exact initial ownership | Named acceptance and validation |
| --- | --- | --- |
| **A — task entry and truthful controls** | One frontend writer: `preview/app.mjs`, `preview/analyze.mjs`; existing tests selected before editing. Controller owns canonical docs and integration. | UA-01 persistent real-workbench link in initial/sample/empty states; UX26-02 accurate search scope; UA-05 authority theme updates both directions. Browser keyboard and pointer checks; `npm run check`, `npm run validate:vision`, relevant route tests, fresh UI review. |
| **B — draft safety and interaction recovery** | One writer: `preview/skills-model.mjs`, `preview/analyze.mjs`, `preview/workflow-view.mjs`, `preview/composition-view.mjs`, `preview/authority-planner-view.mjs`, plus exact associated tests. Sequence after A to avoid overlap. | Dirty skill drafts participate in unload handling; pending field edits have visible recovery; tabs/view buttons expose state; menus close when focus leaves. Targeted state/render tests and browser repros above; native populated skill save/switch/close qualification in an isolated workspace; fresh independent review. |
| **C — unified task hierarchy and narrow-screen refinement** | Design specification first; then one shared-shell writer owns agreed HTML/view/CSS files. Freeze the exact file list before implementation. | Preserve direct routes, record selection, draft state, focus, and original visual structure. Repeat 320/768/1440 light/dark checks, keyboard/reduced-motion/forced-colors checks, and a real source-to-export journey. No new UI framework required. |

Do not batch all three into a broad rewrite. A and B are likely to create more user value than another art or 3D pass. Continue substantive E1 engineering separately; simplified copy must not imply encrypted native storage or working integrations before they exist.

## Qualification still needed

Run an isolated native task study with representative users rather than measuring only model/unit tests:

1. Open Rangoon with no integrations, choose a Markdown source, inspect it, and save a local copy.
2. Create or extract a skill, edit it, switch records with pending changes, save a revision, and reopen it.
3. Compose a small workflow, attempt an incompatible connection, recover without losing work, and inspect the definition.
4. Generate instruction output, understand its target and limitations, export it, and reopen the saved workspace.
5. Find an optional module, compare one relevant capability, and explain whether that function is usable in the current workspace.

Record task completion, wrong turns, requests for help, recovery success, and data-loss incidents. Time-on-task can be compared before and after a repair; no arbitrary speed score is assigned here. Include keyboard-only use, reduced motion, an actual screen reader, stale records, cancellation, missing providers, interrupted native operations, long content, and large libraries. Performance needs measured representative datasets, not the six-node sample alone.

Open product decisions: whether browser mode is a supported local workbench or an explicitly separate preview; which first-time native task defines success; and which optional integration supplies the first real inspection events. Existing local-first and provider-independent boundaries remain accepted constraints.

## Validation and delivery state

- Root browser checks reproduced UA-01, UA-03, UA-05, search-scope mismatch, and workflow menu focus exit. Invalid self-connection refusal and Outline fallback were checked successfully.
- `node scripts/validate-vision.mjs`: PASS — all eight reference hashes, eleven feature mappings, shared-shell contracts, and native-unavailable controls.
- The accessibility lane ran `npm run check` successfully and `node --test tests/authority-view.test.mjs tests/authority-planner-view.test.mjs tests/authority-planner-drag.test.mjs tests/term-help.test.mjs tests/workflow-view.test.mjs tests/composition-view.test.mjs tests/editor-workspace-layout.test.mjs`: 71 passed, 0 failed. Root repeated these checks to retain exact command output in `check.log` and `focused-tests.log`; `vision.log` retains the vision validator output. These existing tests do not close the rendered findings above. The complete build suite was not rerun for this report-only change.
- Root preservation check: all 548 pre-existing file identities unchanged; all 88 preview clone hashes matched both source and fixture; HEAD and index unchanged. Only this report was added. Relative report links and trailing-whitespace checks passed; `git diff --check -- docs/ui-ux-audit-2026-10-13.md` passed (the independent content check also covers the untracked report).
- Normal audit tab returned no warning/error console entries. The iframe fixture returned two `MutationObserver.observe` errors without a source URL. Their source is unassigned; they were not reproduced in the normal tab and are not counted as confirmed application defects. Controlled rendering and interactions completed despite them.
- Temporary browser viewport was reset; normal preview theme restored to light; audit-owned tabs and fixture server were closed. The existing preview server and original user tabs were retained.
- No application implementation, provider configuration, dependency, database, credential, live telemetry, release, or security setting was changed. No commit, push, merge, deployment, or release was performed.

An initial second theme pass reused the playground's ephemeral preference and was rejected during evidence review. All 26 retained captures were then verified against their own rendered `data-theme` attribute; the final matrix is rebuilt from those per-capture JSON receipts. This correction changed audit evidence only.

This report's recommendations are ready for prioritization. They are not completed fixes. A fresh, read-only `gpt_reviewer` (OpenAI GPT-5.6-Terra xhigh), separate from the root report producer, returned PASS with no actionable P1/P2/P3 findings after the evidence corrections. The review checked source claims, theme/width receipts, validation logs, preservation, and cleanup. `report-review.json` retains the result alongside the source-preservation receipt.
