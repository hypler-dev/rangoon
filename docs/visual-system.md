<!-- intent-driven-delivery:spec:v1 -->
# Specification: Rangoon Foldline visual system

Status: accepted
Intent: [intent.md](intent.md), visual quality continuation
Owner: Jeff; primary controller owns visual direction and integration
Last updated: 2026-10-05

## Behavior

Give Rangoon a coherent custom icon family and polished, restrained motion while preserving the existing dark/light workbench and every functional state. Keep the user-selected OS logo unchanged. Use a new decorative illustration on the sample Command Center, not on engine status or authority evidence.

The Foldline family uses folded corners, fine rounded strokes, open shapes and small connection anchors. Keep functional silhouettes recognizable: search still looks like search, theme like a light/dark toggle, and navigation keeps visible labels. Distinct icons must not depend only on color. Avoid emoji, an icon font, third-party icon CDN, or vendor logos masquerading as supported integrations.

## Interfaces and contracts

One local `icons.mjs` module returns vetted inline SVG by closed name and supported size (16, 20, 24, 40). SVG uses a 24-unit viewBox, currentColor, rounded joins/caps and approximately 1.75-unit strokes. Decorative icons are hidden from accessibility and cannot receive focus. Icon-only buttons retain explicit names; no raw SVG, path, URL or attribute string comes from external data.

Replace navigation, graph, connector, workflow and action glyphs in the synthetic preview; add the same family to the real source workbench and engine page. Preserve meaningful labels and operation state. OS platform names remain text; do not invent vendor certification marks. A specimen page displays every icon in multiple sizes and both themes for inspection.

The generated `capability-assembly-v1.png` is a transparent orange/ivory/graphite origami assembly. It is decorative, has fixed intrinsic dimensions, appears at modest desktop size, and is omitted from narrow layouts. Original logo/mascot/reference art remains available and unmodified. [Asset provenance](visual-assets.json) records source, method, prompt and hash.

## States and failure handling

Hover changes color/border/background over 120–180 ms; optional icon translation is at most 2 pixels. Route entry uses opacity and at most 4 pixels of translation for 220 ms and runs only on navigation, not every state rerender. Native disclosure chevrons may rotate briefly. No loop, fake progress, loading shimmer, simulated compliance pulse or animation indicating connection/readiness.

Keyboard focus is immediate and visibly outlined. Reduced-motion preference disables new animation/transforms and smooth scrolling. Functional text and controls remain usable if an image cannot load. Artwork never covers a control or changes its hit target. Keep browser/native unavailable and failed states truthful.

## Data, privacy, and permissions

No new provider, network, native command, storage, telemetry, font download or permission in the application. Image generation used the built-in tool with a descriptive prompt only; no source files, credentials or customer data were sent. The chosen output is copied into this repository. SVG assets are authored locally. No existing logo pixels are altered.

## Compatibility and migration

Presentation-only packet, no data migration. Existing source snapshot and engine-status contracts stay unchanged. Keep scoped graph-edge CSS from accidentally positioning/hiding inline icon SVGs. Respect narrow navigation, light/dark contrast, forced colors and minimum 36-pixel icon-only controls. No new animation library or per-icon request. Reverting this packet restores prior presentation without affecting local data.

## Acceptance mapping

- Check the closed icon catalog, invalid input handling and decorative SVG contract with a bounded asset validator; run syntax, existing Node tests and artifact/hash/link checks.
- Browser QA covers navigation, palette, graph/list, merge, workflow, connector, engine and source pages; dark/light; 320/768/desktop widths; keyboard labels/focus; reduced-motion rules and no missing assets.
- Native build and a focused rendered smoke verify bundled local assets. Retain actual screenshots and clearly distinguish browser samples from native source functionality.
- Fresh independent UI/accessibility/performance review checks exact diff and evidence. CI remains a source gate, not installer or supported-OS proof.

## Non-goals and open questions

No feature completion claims, authority activation, LNSAT change, OS logo replacement, app rename, released installer, deployment, external data transfer or licensing decision. Microcopy may become clearer, but unavailable functions cannot appear enabled. A future animation system and extension icon policy should follow real interaction needs rather than expanding this bounded visual pass.
