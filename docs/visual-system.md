<!-- intent-driven-delivery:spec:v1 -->
# Specification: Rangoon Foldline visual system

Status: accepted
Intent: [intent.md](intent.md), visual quality continuation
Owner: Jeff; primary controller owns visual direction and integration
Last updated: 2026-10-06

## Behavior

Give Rangoon a coherent custom icon family and polished, restrained motion while preserving the existing dark/light workbench and every functional state. Keep the user-selected OS logo unchanged. The first visual slice used a decorative illustration on the sample Command Center. The accepted October 6 continuation adds theme-aware terrain banners to native workbench headers and splash surfaces; engine status and authority evidence stay restrained. Later October 6 steering requires distinct art per menu and mascot accents. A nine-cell light/dark atlas provides individual intake, decomposition, merge, split, skills, compile, model, workspace and disconnected-engine illustrations; none conveys operational status. The original mascot appears at the rail bottom on roomy desktop views and in the Import empty state.

The visual build follows the dual-track rule in [product-experience.md](product-experience.md). Preserve the original eight-screen structure and shared shell: grouped navigation, compact theme-aware banner, central work area, and right inspector. Route-specific theme art may vary, but each route keeps stable composition and control meaning. Visual completion never implies technical completion.

The Foldline family uses folded corners, fine rounded strokes, open shapes and small connection anchors. Keep functional silhouettes recognizable: search still looks like search, theme like a light/dark toggle, and navigation keeps visible labels. Distinct icons must not depend only on color. Avoid emoji, an icon font, third-party icon CDN, or vendor logos masquerading as supported integrations.

## Interfaces and contracts

One local `icons.mjs` module returns vetted inline SVG by closed name and supported size (16, 20, 24, 40). SVG uses a 24-unit viewBox, currentColor, rounded joins/caps and approximately 1.75-unit strokes. Decorative icons are hidden from accessibility and cannot receive focus. Icon-only buttons retain explicit names; no raw SVG, path, URL or attribute string comes from external data.

Replace navigation, graph, connector, workflow and action glyphs in the synthetic preview; add the same family to the real source workbench and engine page. Preserve meaningful labels and operation state. OS platform names remain text; do not invent vendor certification marks. A specimen page displays every icon in multiple sizes and both themes for inspection.

Preserve native IDs, security disclosures, provenance labels, and fail-closed unavailable states when styling or composing a route. Artwork must remain stable per route and must not use random motion, fake readiness, simulated progress, or overlays that cover controls. The compact banner may explain sample or unavailable status; it must not imply runtime, provider, LNSAT, release, or OS qualification.

The generated `capability-assembly-v1.png` is a transparent orange/ivory/graphite origami assembly. It is decorative, has fixed intrinsic dimensions, appears at modest desktop size, and is omitted from narrow layouts. Original logo/mascot/reference art remains available and unmodified. [Asset provenance](visual-assets.json) records source, method, prompt and hash.

## States and failure handling

Hover changes color/border/background over 120–180 ms; optional icon translation is at most 2 pixels. Route entry uses opacity and at most 4 pixels of translation for 220 ms and runs only on navigation, not every state rerender. Native disclosure chevrons may rotate briefly. No loop, fake progress, loading shimmer, simulated compliance pulse or animation indicating connection/readiness.

Keyboard focus is immediate and visibly outlined. Reduced-motion preference disables new animation/transforms and smooth scrolling. Functional text and controls remain usable if an image cannot load. Artwork never covers a control or changes its hit target. Keep browser/native unavailable and failed states truthful.

## Data, privacy, and permissions

No new provider, network, native command, storage, telemetry, font download or permission in the application. The first assembly used the built-in image tool with a descriptive prompt only. The October 6 scene atlas used the generated terrain as a style reference and a matched light-theme edit; the existing mascot is reused unchanged. The terrain pair used the user-supplied Governed Command Center image as an authorized visual reference, followed by an edit of the generated light artwork. No application source, credentials or customer data were sent. The chosen output is copied into this repository. SVG assets are authored locally. No existing logo pixels are altered.

## Compatibility and migration

Presentation-only packet, no data migration. Existing source snapshot and engine-status contracts stay unchanged. Keep scoped graph-edge CSS from accidentally positioning/hiding inline icon SVGs. Respect narrow navigation, light/dark contrast, forced colors and minimum 36-pixel icon-only controls. No new animation library or per-icon request. Reverting this packet restores prior presentation without affecting local data.

## Acceptance mapping

- Check the closed icon catalog, invalid input handling and decorative SVG contract with a bounded asset validator; run syntax, existing Node tests and artifact/hash/link checks.
- Browser QA covers navigation, palette, graph/list, merge, workflow, connector, engine and source pages; dark/light; 320/768/desktop widths; keyboard labels/focus; reduced-motion rules and no missing assets.
- Rendered checks cover 320, 768 and 1440 pixels in dark and light themes, no horizontal overflow, keyboard focus, and reduced motion. `npm run validate:vision` is a narrow contract gate, not aesthetic or runtime proof.
- Native build and a focused rendered smoke verify bundled local assets. Retain actual screenshots and clearly distinguish browser samples from native source functionality.
- Fresh independent UI/accessibility/performance review checks exact diff and evidence. CI remains a source gate, not installer or supported-OS proof.

## Non-goals and open questions

No feature completion claims, authority activation, LNSAT change, OS logo replacement, app rename, released installer, deployment, application data transfer or licensing decision. Microcopy may become clearer, but unavailable functions cannot appear enabled. A future animation system and extension icon policy should follow real interaction needs rather than expanding this bounded visual pass.


## UI-P1: responsive builder interaction and local identity assets

Authority: Jeff's builder-polish/security continuation in intent.md. This is a presentation and ephemeral interaction packet, not encrypted adoption, provider/runtime qualification or persisted content delivery. Root owns implementation, validation and integration after fresh Terra-xhigh read-only design. Ownership refinement before worker edits: native OpenAI Terra-high owns workflow-view.mjs/workflow.css, the two new playground entry files and workflow-view.test.mjs only; root retains workflow-model.mjs/workflow-drag.mjs, model/drag/vendor tests, all vendor/profile assets and protected source/docs/Git. One writer per file; the worker preserves mixed changes. Exact scope: preview/workflow-model.mjs, workflow-view.mjs, workflow.css, new workflow-playground.html and workflow-playground.mjs, new workflow-drag.mjs, new vendor-marks.mjs and assets/vendors/ fixed SVGs plus provenance.json, compilation-view.mjs/compilation.css, model-assistance-view.mjs/model-assistance.css, tests/workflow-model.test.mjs/workflow-view.test.mjs and new workflow-drag.test.mjs/vendor-marks.test.mjs; README.md, docs/intent.md, visual-system.md, development.md and continue.md. Do not change native commands, persistence, providers, approval/consent, manifests/dependencies/locks, icons.mjs catalog, CI, defaults or parent marketing. Existing mixed work remains intact.

The native #workflows surface stays closed without its bridge and links to a standalone synthetic playground. An explicit controller playground mode has no bridge, fake invoke, real workflow identity, native command, persistence or provider path. It starts an ephemeral graph and permits local add/edit/connect/reorder/undo/redo/pan/zoom only. All native library/history/reference loading, report/inspect/candidate/ack/save paths are omitted from playground and refused in its controller, even if a bridge argument is supplied. Labels say synthetic, in memory, not saved, not inspected and no native/provider action. Refresh or exit discards the canvas; native records never populate it.

Live dragging keeps pointer ID, original node/position, current coordinates and at most one animation frame in binding-local state. Movement previews only CSS variables and attached wire geometry; it does not mutate the controller, rerender the whole graph or save. Release commits one rounded move through existing bounds/undo behavior. Pointer cancel, lost capture, Escape, state/render invalidation and disposal cancel the frame and restore prior visual positions without mutation. Refuse pending, read-only, no-session and field-edit states; match the originating pointer and node/session identity. Zoom-correct SVG endpoint coordinates remain attached during drag, pan and zoom. Outline retains keyboard-equivalent editing. Hover/selection/press feedback uses short native CSS transitions, with no animated execution flow or fictional readiness/progress; reduced motion and forced colors preserve usable state.

Allow narrowly scoped local third-party display marks in addition to Foldline controls: Claude beside CLAUDE.md format-only compilation, OpenAI beside the explicitly optional OpenAI model target if an official asset can be obtained. AGENTS.md keeps a generic document mark because it is provider-independent. Runtime remains unqualified; marks never say supported/connected/approved or replace names/state. A closed static module supplies only local approved paths and sizes; downloaded SVGs are fixed inert img assets, never inline user-supplied markup. Record exact official provenance/hashes, inspect closed element/attribute vocabulary, reject script/events/foreignObject/external href/style/CSS/font/animation content in named tests. Preserve original vendor colors/proportions and acknowledge vendor ownership separately from Apache source licensing. No runtime downloads or third-party code are added.

Acceptance: named syntax/visual/vision/Node/artifact/intent/whitespace checks; zero-bridge playground fixtures, unchanged native refusal, drag commit-once/cancel/mismatch/dispose/one-frame fixtures, hostile text escaping, closed assets and viewport/zoom geometry. Render dark/light 320/768/1440 views with focus, reduced motion, no page overflow and actual add/connect/drag/undo behavior. Fresh independent read-only implementation/UI review must close actionable findings. Native embedding build can prove bundled source only; no GUI, provider, encrypted-data or release qualification is inferred. Exact evidence and later publication remain in the ledger.
