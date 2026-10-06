# Composition workbench design

Status: reviewed product direction translated into an implementation design; native R3 interface not implemented
Authority: [composition contract](composition.md), under the accepted [version 1.0 intent](intent.md)
Design: primary controller direction and native OpenAI GPT-5.6-Terra high UI design; implementation receives separate fresh review

## Layout and navigation

Use the existing graphite/orange application shell, custom Foldline icons, dark/light tokens and reduced-motion behavior. The references are [Decompose](references/decompose.png) and [Merge & Split](references/merge-split.png). Their three-column workbench and connected input/output cards guide layout; sample counts, confidence percentages and simulated actions do not enter real routes.

`#decompose` takes one saved source. `#merge` takes two to sixteen pinned skill revisions and produces one output. `#split` takes one pinned revision and produces two to sixteen outputs. Each route identifies actual workspace content and the in-memory unsaved draft. The native picker/import route remains the way to add a source; these routes do not imply folder access.

At wide widths, use a 240-pixel input/coverage column, a flexible source/recipe or graph area, and a 320-pixel inspector. At 768 pixels, move coverage to a horizontal summary, place source/recipe tabs centrally and disclose the inspector below. At 320 pixels, use one column and a complete list representation of the graph. Contain source-code horizontal scrolling within its pane; every required action must remain visible without drag or hover. A bottom action bar must not cover the focused field or final content.

## Core components

| Component | Purpose |
| --- | --- |
| Header | Operation, selected input identities, draft status and one primary next action |
| Input picker | Saved source or exact revision selection, digest, explicit historical badge and ordering |
| Coverage list | Actual input-byte totals by disposition; jump to first unassigned span, duplicate or conflict |
| Source pane | Read-only original text with line numbers and selectable character-safe spans |
| Output recipe cards | Output title, Create new skill / Update existing skill destination, ordered Copy/Authored/Replace pieces, provenance and unreviewed state |
| Graph/list view | Two synchronized representations of the same inputs, pieces and output records |
| Inspector | Selected piece/span, bounded reasons, before/after text and optional identity details |
| Preview footer | Host validation, unresolved counts, preview freshness and exact-preview acknowledgment |

Display digests and byte math in details panels. They support inspection without becoming the primary workflow. Do not expose editable byte-index boxes. Host-provided ranges must map to displayed Unicode characters, and renderer selections must convert UTF-16 positions to verified UTF-8 boundaries before requesting a preview.

## Editing workflow

For Decompose, show analyzer sections as an initial complete partition, including preamble and separators. A source selection offers Assign to output, Duplicate deliberately, Replace, and Exclude. The user can choose an existing output or name a new one. Unassigned input stays visible and blocks saving; moving a boundary does not delete the intervening text.

For Merge, preserve exact chosen revision content and its displayed order. Any separator is an explicit authored piece. For Split, show one source revision and assign a complete partition among outputs. Shared context is deliberate duplication with a reason, not an implicit exception to coverage.

Copy pieces are read-only source references. Edit new prose in Authored pieces; edit transformed prose in Replace pieces that retain the exact original range. Initial implementation edits pieces rather than allowing an unrestricted output textarea to silently destroy provenance. Reordering has keyboard-accessible Move up/down controls; dragging is optional. Each reasoned operation has Apply and Cancel with draft-preserving behavior.

Conflict declarations show the selected source ranges together with their context and nullable resolution. A recorded resolution is labeled as a local explanation, not proof of semantic safety. Preview shows actual output text, piece origins and replaced/excluded text.

Each output explicitly chooses Create new skill or Update existing skill under the proposed [destination contract](composition-destinations.md). Existing destinations show the actual title and pinned current head with a before/after preview. An output cannot silently select a historical input as its target head, and two outputs cannot target the same skill. The last step names which skills will be created or updated and acknowledges the exact preview; editing any input, recipe or destination makes that acknowledgment stale. Save opens actual created or updated skills with unreviewed new revisions. A changed target head preserves the draft and requires explicit refresh and review. Skills shows both the stable birth origin and the selected revision's provenance.

## State and accessibility rules

Loading and native-unavailable states are distinct from an empty library. An incomplete draft remains editable while showing its blocking spans. During preview, retain useful prior output with a stale marker. A failed or stale save retains inputs, recipes, reasons, selection and scroll; the recovery action is explicit and does not repeat a write automatically. Pending save locks recipe mutations. Success lists actual returned skills, never sample counts.

The draft is in memory until explicit save; navigation that would discard it needs a clear preservation/discard decision. Theme and layout changes do not discard it. Keyboard users must complete all three workflows through the list/editor view. Use visible focus, text labels in addition to disposition colors, a polite live region for preview completion and clear field-linked validation messages. Restore focus after inline Apply/Cancel to the originating piece/action. Under reduced motion, disable animated edges and automatic scrolling; offer an explicit Reveal selected span action.

## Acceptance evidence

Verify the complete native flow for each operation with exact bytes, BOM/CRLF, Unicode and terminal newlines. Verify rejected gaps/mixed dispositions, deliberate duplicates, reasoned replacement/exclusion, unresolved conflicts, historical inputs, new and existing destinations, mixed atomic outputs, target-as-input, duplicate-target rejection, stale target heads, stale confirmation, failure draft retention and returned unreviewed output records. Check keyboard-only completion and 320/768/1440 dark/light layouts. Synthetic layout fixtures may support visual QA but cannot stand in for native mutation/restart evidence. All-three-OS GUI and installer/release qualification remain separate gates.
