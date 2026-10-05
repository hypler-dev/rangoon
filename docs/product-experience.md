# Product experience: make the visual promise useful

Authority: [intent.md](intent.md). Jeff's October 5 clarification requires reviewing the supplied images as workflows and building a strong, usable platform, not stopping at styled screens. This record translates that direction into behavior and acceptance. The [architecture plan](plan.md) remains the full sequence; [development.md](development.md) owns implementation evidence.

## Purpose and first useful outcome

Rangoon helps a developer understand and improve the instructions driving their agents. The first useful outcome is concrete: choose an existing instruction file, see what it contains, identify reusable sections, and trace every proposed change to its source. Later, a reviewed capability can become a portable bundle with a visible compatibility report and a deliberate export. This work remains useful without an AI subscription, cloud account, or execution engine.

The images share an effective structure: familiar left navigation, a page-level primary action, a dense central work area, and an inspector that explains the current selection. Preserve that structure, orange identity and dark/light styling. Avoid copying fictional fleet counts, compliance scores, unsupported integration logos or unexplained model-confidence percentages. Every displayed count must describe actual workspace data or explicitly labeled samples.

## Image-to-behavior contract

All eight originals in [the reference manifest](reference-manifest.json) were visually re-reviewed on October 5 by the independent UI design lane.

| Reference | User outcome | Required real behavior | Empty, failure and recovery |
| --- | --- | --- | --- |
| S02 Import & Analyze | Bring known instructions into a workspace | Explicit source selection, bounded inventory, exact bytes, visible parser coverage, review before creating assets | First-run guidance; cancellation is quiet; rejection explains the next step; prior successful analysis survives a failed replacement |
| S08 Decompose | Understand reusable boundaries without losing context | Source/section/inspector panes, exact source highlights, retained unassigned content, later reviewed capability proposals | Empty file, no sections, unsupported syntax and unclosed fences remain visible; no invented confidence; undo for future transforms |
| S01 Merge & Split | Reorganize reviewed material without silent loss | Compare selected revisions, account for every moved/retained/duplicated span, resolve conflicts before draft creation | No selection guidance, incompatible inputs, explicit blocking conflicts and recoverable drafts |
| S07 Skills | Find, inspect and reuse a trusted local asset | Search/filter actual revisions, source history, review status, dependencies and qualified compatibility | Honest empty library/no matches; unknown compatibility is not a supported logo; deprecated versions stay inspectable |
| S04 Workflows | Assemble a clear sequence and understand its outcomes | Declared steps, versioned inputs, static checks, separately configured test/execution paths and durable run records | Draft/blocked/failed/unknown states, cancellation and reconciliation; no blind retry after uncertain effects |
| S03 Connectors | Understand exactly what a connection can do | Real adapter/configuration status, bounded operations, credential scope, separate action authority and health evidence | Not configured, unavailable, permission denied, disconnected and uncertain state are distinct; credential access is not permission to act |
| S05/S06 Command Center | Know what needs attention in the actual workspace | Data-derived inventory/activity, drill-down evidence and the next useful action | New workspace routes to import; unavailable evidence is not zero; operational indicators require a defined denominator |

## First implementation: one real file lifecycle

The desktop spike starts in **Import & Analyze**. One primary action opens the native picker. A real successful analysis displays the chosen basename, exact digest/counts, original text, section proposals and diagnostics. Selecting a section highlights its source lines and opens the byte/line metadata in the inspector. **Save locally** creates a bounded immutable snapshot only after explicit user action; the saved list shows local records, identical basename and bytes deduplicate, and changed bytes or basename create a separate record. Reopening after restart reanalyzes the stored bytes. The current implementation does not turn a Markdown heading into a reviewed skill.

The existing nine-screen design preview remains a clearly identified sample experience. It does not receive real source records. This prevents a real imported file from appearing to have passed a fictional review, test, connector or deployment. When actual local capability revisions exist, migrate the relevant screen from fixtures to the application contract with its own state and recovery tests.

## Usability acceptance

1. A first-time user sees one obvious action without signing in or choosing an engine. The action works by keyboard and restores focus after the picker closes.
2. Cancellation keeps the current report. An unreadable, unsupported or oversized replacement leaves useful prior work visible and offers a retry.
3. Filename, counts and digest match the selected file. Every section selection highlights the exact reported line span; original text is never rendered as executable HTML.
4. Empty, partial, busy, failed and unavailable states have plain explanations. Technical codes and hashes are available in details rather than dominating the primary flow.
5. Dark/light views at 320, 768 and 1440 pixels retain reachable controls, readable source, visible keyboard focus and accessible status announcements. Long source lines scroll inside the source pane, not the entire page.

Use real pilot observations for time-to-first-useful-result, confusing labels, failed imports, correction rate and lost-work reports. No performance or usability percentage is claimed before measurement.

## Build a platform through complete workflows

The next product slice is durable local workspaces: immutable source snapshots, explicit project ownership, recoverable jobs and reviewed capability revisions. A saved asset must survive restart, retain its source identity and support an explicit changed-file/reimport decision. Database/schema design remains controller-owned and requires migration, disk-full and crash/recovery evidence before adoption.

Then implement real Decompose and Merge/Split around a coverage ledger, followed by two pinned format adapters and a reviewable export. Only then should the command center summarize a meaningful working inventory. Workflow execution and connectors follow their own qualified authority path; they do not become safe because a canvas or button exists. R2a source records retain `authority: none` and unreviewed fragments; they are local source history, not reviewed capabilities.

Keep domain analysis independent of desktop IPC and UI frameworks. Desktop and future server modes may share versioned records and conformance fixtures, while filesystem selection, credentials, identity and execution remain host-specific adapters. A cloud service must not inherit unrestricted local filesystem access, and a subscription must never grant consequence authority. This preserves room for self-hosted teams and enterprise operation without requiring a distributed platform before the local workflow is useful.
