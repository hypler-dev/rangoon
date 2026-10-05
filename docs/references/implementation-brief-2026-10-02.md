# Rangoon Interface Implementation Brief

Codex preparation and Astra implementation handoff

Prepared 2 October 2026 • Revision 1 • Editable implementation specification

## Purpose and decision

Codex must give Astra a bounded, evidence-backed implementation packet for the Rangoon.ai interface. The packet must include the eight supplied visual references, verified repository facts, page and component contracts, deterministic sample data, integration boundaries, and named acceptance checks. Astra should implement the agreed interface from that packet without inferring a working backend, installer, authority engine, or connector from a screenshot.

Rangoon manages agents and the capability lifecycle: Discover → Decompose → Compose → Test → Compile → Deploy. LNSAT remains a separate execution-authorization and evidence system that Rangoon is intended to bundle or download during setup. Rangoon must also be designed for future compatible systems, SDKs, modules, plugins, and extensions. Compatibility must be explicit and tested; installing a connector or extension does not grant action authority.

## Authority and evidence labels

The current user request is the canonical work authority for this document. It authorizes the brief, including the later architecture clarification and supplied references. It does not establish production implementation or authorize a deployment.

- **Required direction** means a current user instruction or a visible feature of an explicitly supplied reference.
- **Conversation direction** means product architecture described in the referenced assessment. It informs the specification but is not proof that software exists.
- **Proposed contract** means a concrete implementation recommendation introduced by this brief. Routes, interfaces, state names, default dimensions, and build order are proposals until reconciled with the repository and accepted packet.
- **Unverified** means Codex must provide implementation evidence or mark the capability unavailable. A screenshot value is not such evidence.

## Source register

- **U1** — Current request for this editable brief and its named feature scope.
- **U2** — Current clarification that LNSAT is separate, bundled or downloaded by Rangoon, with future compatible systems and an SDK and extension architecture.
- **C1** — “Branch · LNSAT Project Assessment,” conversation 6a66d38b-6f80-83e8-960a-b588df6c9769. The product overview was fully retrieved; one earlier assessment response was truncated. No older-page cursor was available.
- **C2** — User priorities within C1: Merge & Split is central to compiling and managing capabilities; Decompose must have its own page.
- **S01–S08** — Eight images supplied in this chat. Embedded with filenames, dimensions, hashes, and annotations in the reference appendix.

Implementation readiness depends on section 2. No product repository was inspected; this workspace is not a Git repository. Recheck historical LNSAT maturity claims before reuse.

<!-- PAGE -->
# 2 What Codex must deliver to Astra

## A single bounded packet

Provide the following named artifacts, or map each item to an existing canonical repository document. These are proposed packet names, not claims that files already exist. Do not maintain duplicate acceptance records.

| Packet item | Required content and completion evidence |
| --- | --- |
| Handoff record | Canonical issue or packet, user outcome, scope, non-goals, acceptance owner, allowed files, explicit external-action boundaries, and stop conditions. |
| Repository snapshot | Absolute path, remote identity, branch, HEAD, clean or dirty status, relevant AGENTS.md, exact first-read docs, and owned versus unrelated changes. |
| Implementation inventory | Framework, language, router, package manager, lockfile, UI/chart/graph libraries, runtime mode, existing routes, design tokens, tests, fixtures, and working start command. Cite actual files. |
| Reference manifest | S01–S08 originals, hashes, route and state mapping, chosen Command Center variant, measured layout, asset ownership or provenance, and approved deviations. |
| UI specification | This brief reconciled with repository facts; route map, tokens, component interfaces, forms, state transitions, error copy, and responsive layouts. |
| Data and integration packet | DTO schemas, redacted examples, mock/live adapter boundary, support matrix, authority-provider contract, job behavior, and error catalog. |
| Evidence and review packet | Exact validation commands, expected results, fixture IDs, viewport matrix, screenshot captures, independent review scope, unresolved decisions, and delivery checklist. |

## Repository discovery requirements

Start with repository, branch, HEAD, and status; then read required control documents. If a Graphify index exists, perform a targeted query before a broad source scan. Inspect only the UI and integration surfaces needed for the named slice. Report unavailable tools instead of inventing outputs.

Do not select a new framework or replace existing infrastructure merely because it is familiar. If Rangoon has no implementation repository, Codex must explicitly establish a new-project packet, stack decision, writable location, and persistence model before delegating production code.

## Evidence that cannot be inferred

Codex must identify whether the application is browser-only, desktop-assisted, locally hosted, or hybrid. Folder scanning, installed-harness discovery, daemon setup, and local filesystem deployment require a verified host bridge or service; a browser UI alone does not establish them. Show unavailable source options honestly when that bridge is absent.

Codex must also provide supported import formats and limits, schema versions, identity and workspace boundaries, adapter versions, storage rules, actual API methods, supported targets, installer availability, and permission checks. If these do not exist, Astra may build the accepted fixture-backed interface behind an explicit demonstration mode. Mock behavior must never be reported as live integration.

<!-- PAGE -->
# 3 Product architecture and authority boundary

Required direction: U2 and C1. The module division below is a proposed implementation contract.

## Separate products with coordinated setup

Rangoon owns the management experience, canonical capability assets, composition, harness compatibility, tests, compilation, deployment planning, and operational views. LNSAT owns its own implementation, lifecycle, versioning, execution authorization, and evidence semantics. Bundling or downloading LNSAT is a distribution relationship; it does not merge the two products or make Rangoon an authority engine.

The proposed layer order is: interface → Rangoon application services → capability and job stores → versioned adapters. Adapter families connect to importers, semantic analysis, harness compilers, test runners, connectors, deployment targets, and authority providers. Keep UI view models separate from wire contracts and persisted records.

## Capability flow and execution flow

The capability flow converts selected source evidence into reviewed assets, a composed graph, test evidence, compiled artifacts, and a deployment plan. Each stage records immutable input versions and lineage.

The consequential execution flow submits an exact proposed action through the selected authority provider. For LNSAT, preserve its documented path: canonical action packet → deterministic policy → scoped human approval when required → one-time authorization → bounded adapter → receipt and outcome reconciliation → evidence. Obtain actual contract types from LNSAT; do not recreate them from this summary.

## Future compatible authority providers

A proposed AuthorityProviderAdapter exposes identity, version, health, supported semantics, policy evaluation, approval linkage, execution submission or authorized handoff, status lookup, receipt retrieval, and reconciliation support. Method names and transports must come from a versioned SDK contract. UI components consume a normalized status view while retaining raw provider identifiers and evidence references.

Compatibility is evaluated against a versioned Rangoon authority profile. Record action-scope binding, approval binding, replay behavior, consumption semantics, policy failure behavior, unknown outcomes, evidence identity, and reconciliation support. Mark each requirement supported, transformed, unsupported, or unverified. A transport connection alone is insufficient.

If a provider cannot meet a profile, allow only explicitly supported non-consequential functions or block the affected operation. Do not simulate missing one-time semantics in frontend state. Never silently switch providers, retry an ambiguous action elsewhere, or treat evidence from different providers as interchangeable.

Provider migration requires a separate plan for in-flight actions, approvals, evidence continuity, pinned versions, and rollback. A disconnected provider may leave editing and read-only history available; actions requiring current authority stay blocked. The UI must display provider identity and the reason for any degraded behavior.

<!-- PAGE -->
# 4 SDK modules plugins and extensions

Required direction: U2. These terms need distinct contracts so the interface remains extensible without weakening governance.

| Term | Meaning and boundary |
| --- | --- |
| Module | An internal Rangoon feature boundary, such as asset management or compilation. It does not imply separately installable code. |
| SDK | Versioned developer types, schemas, adapter interfaces, lifecycle rules, examples, validators, and conformance fixtures. It is not an authority bypass. |
| Plugin | An installable package with a manifest, declared extension points, dependencies, configuration schema, required permissions, and compatibility range. |
| Extension | A registered contribution: importer, harness adapter, connector, workflow node, test evaluator, policy pack, model profile, UI panel, deployment target, or authority adapter. |
| Extender | The product catalog and management presentation for packaged extensions, including possible future commercial offerings. No marketplace or billing implementation is established. |

## Codex must provide the SDK packet

Specify manifest fields for package ID, publisher, version, SDK range, supported Rangoon versions, entry points, contribution kinds, dependency constraints, configuration schema, requested data/tool access, execution location, network and filesystem scope, provenance or digest, and optional signature metadata. Never label an unsigned or unverified package verified.

Define lifecycle events for discover, validate, configure, enable, disable, upgrade, rollback, and uninstall. Define typed error handling, timeouts, cancellation, resource limits, logging redaction, and schema migration policy. Include one small example per extension family used in the first slice, plus a deliberately incompatible fixture.

Specify the actual runtime isolation mechanism separately. A plugin API does not establish sandboxing. Native code, remote services, browser panels, and local processes have different trust boundaries. Codex must name which are allowed and how they are restricted; Astra must not invent a plugin execution host.

## Interface requirements

The Extenders page must expose installed and available packages, type, version, publisher, compatibility, health, and permission changes. Detail tabs should cover Overview, Configuration, Contributions, Permissions, Versions, and Diagnostics. Show which pages, workflow nodes, connectors, or adapters each package contributes.

Before enabling or upgrading, present the exact version, configuration requirements, permission delta, affected assets, compatibility results, and recovery option. Disabling a package must show dependent workflows and deployment effects. A disabled contribution stays visible in historical records with an explanatory label.

## Acceptance evidence

An incompatible SDK range cannot enable a plugin. A removed node type renders as an unresolved node without losing its stored configuration. Permission expansion requires the applicable backend approval path. Plugin crashes do not take down the shell. Secrets never appear in frontend fixtures, exports, or diagnostics. An authority extension cannot override provider identity, approval scope, or receipt semantics privately.

<!-- PAGE -->
# 5 Information architecture and navigation

Reference direction: S02–S08 use a consistent grouped sidebar. Use that grouping as the proposed primary shell; S01 is an alternate Command Center treatment. Route paths below are proposals to reconcile with the actual router.

| Group | Page labels and proposed routes |
| --- | --- |
| Home | Command Center — /command-center |
| Build | Agents — /agents; Skills — /skills; Workflows — /workflows; Templates — /templates; Registry — /registry |
| Discover | Import & Analyze — /studio/import; Decompose — /studio/decompose; Merge & Split — /studio/merge-split |
| Test & Simulate | Test Lab — /test-lab; Simulations — /simulations; Evaluations — /evaluations |
| Govern | Policies — /policies; Approvals — /approvals; Impact Analysis — /impact-analysis; Audit Explorer — /audit |
| Deploy | Compile — /compile; Deployments — /deployments; Environments — /environments |
| Connect | Connectors — /connectors; Harnesses — /harnesses; Extenders — /extenders |
| Analyze | Analytics — /analytics |
| System | Settings — /settings; Activity through Command Center and a stable /activity deep link |

## Shared navigation behavior

Exactly one primary page is active. The Workflows reference also highlights Command Center; correct this inconsistency rather than reproduce two active destinations. Preserve group labels and user-facing terminology. Capability Studio is the connected lifecycle across discovery, asset editing, testing, compilation, and deployment; it need not become a competing duplicate sidebar.

Give list, detail, selected tab, filters, and job records stable addresses. Proposed patterns are /skills/:id, /workflows/:id, /connectors/:id, /studio/decompose/:jobId, and /deployments/:id. Store shareable filters in the URL and transient drag or dialog state locally. Browser Back must restore the prior view without losing saved selections.

The top bar contains search, theme control, notifications, and workspace identity. Search returns typed results and routes to their details. The visible “ask Rangoon” concept in S01 is not proof of a conversational assistant; expose it only if the packet defines that service and its data boundary.

Unavailable modules must lead to a useful, explicitly unavailable state with a reason and next step. Do not ship dead links or indistinguishable fake pages. Templates, Registry, Approvals, Audit, and Environments need minimum coherent states even if their full backend arrives later.

## Selection and context

Carry workspace, environment, selected asset version, harness, and authority provider into downstream actions. Show those choices again at Test, Compile, and Deploy. A row selection opens an inspector; a distinct open action navigates to a full detail route. Checkbox selection and inspector selection must not unexpectedly change each other.

<!-- PAGE -->
# 6 Design system and reference fidelity

Observed direction: S02–S08 show a white and pale-neutral shell, black headings, muted blue-gray secondary text, orange active states, fine borders, rounded cards, compact controls, and wide illustrated banners. The orange Rangoon mascot, red/green/blue companion characters, mountain scenery, and handwritten decorative phrases are central brand assets.

## Proposed tokens for calibration

These are starting values, not extracted production tokens or exact font matches. Codex must measure screenshots at native size and reconcile them with supplied brand assets.

| Token family | Proposed starting contract |
| --- | --- |
| Layout | Sidebar 232 px; top bar 56 px; content gutter 16 px; card gap 12–16 px; inspector 340–380 px at wide desktop. Use flexible grids. |
| Spacing and shape | 4 px base scale; controls about 36–40 px high; card radius 10–12 px; 1 px neutral borders; restrained shadows. |
| Typography | Existing project sans-serif if suitable; UI 14 px, secondary 12–13 px, section headings 20–24 px, hero 40–48 px at desktop. Exact font is unverified. |
| Semantic color | Canvas #F7F9FC; surface #FFFFFF; ink #0B1220; muted #53617A; border #E2E7EF; brand orange around #FF5A0A. Validate contrast before adoption. |
| State color | Green success, amber attention, red failure, blue information, purple capability metadata. Always pair color with text or icon labels. |

## Fidelity rules

Use S03 for the proposed Command Center layout and common shell because it matches the detailed page family. Retain S01 as an approved alternate visual reference, not a silent source of conflicting navigation. Record this precedence as an implementation choice, not a claim that the user rejected S01.

Use page-specific references: Skills S02, Decompose S04, Merge & Split S05, Import & Analyze S06, Connectors S07, Workflows S08. Other pages inherit the same shell, typography, density, cards, tabs, and inspector conventions; their layouts are proposed.

Implement text, controls, rows, charts, and graph nodes as real interface elements. Never place an entire screenshot behind clickable hotspots. Preserve banner artwork separately from live page headings and actions. The supplied images are flattened mockups; clean mascot, logo, banner, icon, and font assets have not been established. Codex must supply licensed or user-owned source assets or record the chosen interim asset treatment. Do not claim to have extracted editable layers.

Keep artwork decorative and out of the working area. Allow compact hero treatment for dense or narrow layouts. The references show a theme control but no dark-theme specification; dark mode requires a derived, reviewed palette and readable artwork treatment, not automatic color inversion.

<!-- PAGE -->
# 7 Screenshots and sample content

## Reference handling contract

Preserve the eight original files unchanged. The appendix embeds all eight, identifies their native dimensions and SHA-256 digests, and records observed structure. Codex must provide originals alongside the implementation packet, plus a manifest linking reference ID, route, viewport, theme, state, selected entity, and any approved difference.

The document images are review copies. Use originals for implementation comparison; do not measure the scaled Word preview. Record browser viewport, device pixel ratio, zoom, font readiness, fixture seed, and disabled animations for every capture. Compare at each reference’s native canvas size before evaluating responsive variants.

For visual review, capture the full page and important working-area crops. Check hierarchy, sidebar grouping, banner size, grid proportions, table density, inspector alignment, typography, actions, icons, borders, and selected states. Use overlays or image differences to locate drift, then assess it visually. Numeric screenshot tolerances must be set in the packet; do not invent a passing score.

## Required interpretation corrections

- Dashboard counts, percentages, dates, people, usage totals, customer quotes, and “All Systems Operational” are sample content. Use deterministic, internally consistent fixtures or verified live data. Never imply a real customer testimonial from S06.
- S02 status totals do not form a reliable partition. Define which categories are exclusive, whether “Shared” overlaps, and how counts are computed.
- S03 reuses 142 in different contexts. Distinguish active runs, historical runs, total capabilities, and policy-evaluated actions.
- S04 confidence and 78% progress are illustrations. Display confidence only with a documented interpretation, and determinate progress only when the job supplies a meaningful denominator.
- S05 token reduction and overlap percentages need measurements and baselines. Until available, label them estimates or show “Not measured.”
- S07 logos and connection badges illustrate a catalog; they do not establish working integrations or permission grants. Correct malformed labels without preserving image-generation artifacts.
- S08 lists Local, Staging, and Productions beneath “Harness Compatibility.” Separate environment compatibility from harness compatibility and normalize “Production.” Its finance workflow is a synthetic example, not an approved operating policy.

## Demonstration and live modes

The fixture-backed implementation must have an obvious demonstration label and isolated data adapter. Live mode must use service-provided identifiers, timestamps, support checks, counts, and authority state. Loading or disconnected data must never display an unconditional green health banner. Redacted sample projects should remain coherent across all screens so a user can follow one asset from import through deployment preview.

<!-- PAGE -->
# 8 Command Center

References: S03 primary proposal; S01 alternate. Purpose: understand fleet health, current work, and the next meaningful action.

## Visible layout

Reproduce the hero with the two-line message “Build capable agents. Keep them in bounds.”, a short product description, Create Agent, and Run Simulation. Follow it with six summary cards: Agents, Skills, Workflows, Policy Compliance, Pending Approvals, and Active Runs. Preserve the broad card-and-table hierarchy and mascot treatment.

The main row contains Agent Fleet, System Activity, and Capability Distribution with Runs Over Time. The lower row contains the lifecycle guide, a policy summary, and Quick Actions. Keep charts secondary to operational content. S01’s alternative metrics and full mountain background remain optional styling decisions recorded in the reference manifest.

## Data and interactions

Fleet rows expose agent name and summary, harness identity, version, status, last-run time, selection, and row actions. Harness pills filter the table. View All opens Agents with the same filter. A row opens the agent inspector or detail page. Quick Actions route to Import, Skills, Workflows, Test Lab, Policies, Deployments, Registry, and Analytics.

Activity entries expose event type, subject, timestamp, environment, and a stable record link. “Live” requires a functioning subscription or a clearly labeled polling cadence. A time-range control updates related charts together. Each metric card drills into the filtered source records used to calculate it.

Policy compliance must state its population and period, such as evaluated actions, with unknown or unevaluated outcomes separate. Active Runs must count only active jobs. Pending Approvals must match the queue for the current workspace and permissions.

## States and acceptance

First use shows a setup checklist: configure workspace, make an authority provider available where required, import a project or create an asset, select a supported harness, and run a test. Avoid a wall of zero charts.

Loading uses stable placeholders. A failed analytics panel does not erase the fleet. Stale data shows last refresh and a retry action. A disconnected authority provider appears in status details and blocks only affected operations.

Accept when card counts reconcile with their lists, all primary actions resolve, event links open the correct records, failed and unknown runs remain distinguishable, and the page matches S03’s composition at 1536 × 1024. S01 styling must not introduce a second navigation system.

<!-- PAGE -->
# 9 Capability Studio and Import and Analyze

Reference: S06. Conversation direction: C1. Capability Studio maintains one asset lineage across Discover → Decompose → Compose → Test → Compile → Deploy. Navigation may leave a stage, but returning must restore the saved job and selected source version.

## Source page

Use the S06 hero, five-step rail, source panel, Recent Imports, supporting-format tiles, and restrained guidance area. Preserve the step labels Source, Analyze, Review, Configure, Import. Replace the mock testimonial with documentation or an evidence-based help panel unless approved customer copy is supplied.

Source tabs are Local Files, Git Repository, Installed Harness, Archive Upload, and Template. Each requires a distinct contract. Folder and installed-harness access depend on the actual host bridge; Git requires a repository and pinned revision; archives require file limits and safe extraction; templates identify a registry version. Provide keyboard file selection as well as drag and drop. Auto-Scan is scoped to a selected location, never an implicit whole-machine scan.

## Analysis and review flow

Show a preflight manifest with included and excluded files, format support, size limits, revision or digest, and analysis destination. Imported content is untrusted data. Do not execute scripts, hooks, configuration commands, or instructions during discovery. Do not send source content to a semantic-analysis provider before the configured data policy permits that transfer.

Run deterministic discovery and schema or syntax parsing first, static extraction next, then optional semantic classification. Present parsing evidence separately from model suggestions. Unsupported files remain visible with a reason. A source error must not disappear into a confident capability recommendation.

Review groups candidates into skills, context, rules, workflows, tools, connectors, and policy-sensitive operations. The user can accept, reject, rename, reclassify, or send a candidate to Decompose. Configure resolves output names, versions, dependencies, target harnesses, and workspace destination. Import creates reviewed draft records and a receipt listing created, skipped, and failed items.

## States and acceptance

Support no source, invalid source, permission denied, scanning, semantic analysis disabled, partial parse, review required, import conflict, canceled job, failure, and completion. Retry only the failed safe stage; do not duplicate already imported assets. A source revision change invalidates stale analysis.

Accept when a synthetic project can be selected, inspected, analyzed, reviewed, and imported without executing its content; every candidate links to source evidence; duplicates require a choice; and refresh resumes the correct job. Recent Imports must link to actual import records, including partial failures.

<!-- PAGE -->
# 10 Decompose

Reference: S04. Explicit priority: C2. Purpose: turn large instruction sets into reviewable, reusable assets while preserving context and traceability.

## Screen anatomy

Preserve the five stages Select Source, Analyze, Review & Refine, Create Assets, Complete. Below the hero, show a Source Files tree on the left; source text and analysis tabs in the main area; a Detected Modules list; and a Module Details inspector. At wide desktop these read as four coordinated regions. The progress summary includes candidate skills, context blocks, workflows, and tool references, computed from the same job.

The source tree supports search, selected-file filtering, bounded multi-selection, and an explicit Change Source action. The viewer provides line numbers, highlighted evidence spans, copy, and View Full File. Tabs switch Source Content, Analysis, and Detected Capabilities. Imported text remains inert.

## Candidate contract and actions

Each module has a proposed name, type, description, source spans, confidence metadata if available, extracted elements, dependencies, policy-sensitive flags, and review status. Selection in the module list updates the inspector and evidence highlight. The inspector offers Overview, Content, and Dependencies.

Suggested actions include create a reusable skill, retain a context block, merge with similar content, and refine before creating. These are proposals requiring review. “Create Skill from This Module” must open or apply the same explicit review gate as the bulk Create Assets step; it must not silently bypass it.

Review & Refine allows boundary editing, naming, classification, preservation of shared context, dependency correction, exclusion of irrelevant content, and comparison with the original. Flag unassigned required content, duplicate inclusion, circular dependencies, and missing policy requirements. Create Assets proposes immutable output versions and records their ancestry; the original source remains unchanged by default.

## Failure and acceptance requirements

Distinguish zero candidates from failed parsing and from semantic analysis being unavailable. A low-confidence result stays editable and clearly advisory. Canceled analysis preserves selected files and any saved review draft. Source or dependency changes make affected analysis stale.

Acceptance scenario: select a synthetic CLAUDE.md plus AGENTS.md, inspect at least three candidate types, refine one boundary, keep one context block, reject one candidate, and create draft assets. Each output must resolve to original file digest and exact source spans. The selected count must match the candidates being created. No generated candidate gains execution permission through decomposition.

<!-- PAGE -->
# 11 Merge and Split

Reference: S05. Explicit priority: C2. This is a core capability-management workspace, not a minor dialog. Merge Skills, Split Skill, and History must be equally discoverable. S05 shows the merge state; split and history behavior below are proposed contracts.

## Merge flow

Preserve the source library at left, Visual Builder / Text View / Analysis Results in the center, and output preview at right. The library supports search, category and harness filters, multi-selection, and selected counts. Require at least two pinned source versions. Analyze & Merge creates a proposal rather than overwriting inputs.

The graph shows source assets, a resulting asset, and optional components. Selecting an edge explains provenance or dependency; node selection synchronizes text and preview. Support zoom, Fit, full-screen view, and keyboard alternatives. Analysis groups overlap, complementary behavior, conflicts, and estimated optimization with evidence and uncertainty.

Conflict resolution must cover contradictory instructions, input/output schema mismatch, dependency versions, activation differences, tool or connector requirements, policies, and target-specific behavior. Shared context can become a common dependency with target variants. Never solve a conflict by silently dropping a restriction or choosing the broadest permission.

The output preview edits name, description, category, icon, tags, target harnesses, dependencies, and destination. Create New Skill is the safe default. Update Existing creates a new version after impact review; it does not mutate a published version in place. Save as Draft preserves the proposal. Review & Compile opens an explicit diff, unresolved findings, and compilation preflight.

## Split flow and history

Split starts with one pinned asset version. Show source content and dependency graph, selectable boundaries, output buckets, and an assignment ledger. Users can extract independent procedures, preserve a shared core, define target-specific variants, and preview each resulting asset. Required content must be assigned, deliberately retained as shared context, or explicitly excluded with a reason. Warn on duplicated or orphaned behavior.

History records operation kind, actor, inputs, output versions, decisions, test evidence, and timestamp. Reopen creates a new draft from the record. Undo applies to uncommitted editing; reversing a completed operation requires a new version or explicit recovery plan, not deletion of ancestry.

## Acceptance

Demonstrate three-source merge with one unresolved policy conflict blocking compilation, a resolved draft preserving all source links, and a split into two assets plus shared context. Dependency changes invalidate prior analysis and tests. Neither operation claims behavioral equivalence without Test Lab evidence. Split and History need dedicated captures because no reference image shows them.

<!-- PAGE -->
# 12 Skills

Reference: S02. Purpose: browse, understand, version, and act on reusable capabilities.

## Library and inspector

Preserve the Skills banner, New Skill split action, five summary cards, toolbar, dense table, and right inspector. Table columns include selection, skill name and summary, category, harness compatibility, version, status, last updated, usage, and actions. Provide search, status/category/harness filters, sort, and a way to clear all filters.

Use explicit text alongside harness icons. Distinguish target support from measured portability. Usage displays a count and defined period; bars are comparative decoration, not the only way to read the value. The inspector tabs are Overview, Configuration, Tests, Usage, and Versions. Overview includes tags, dependencies, Used By links, version, and last-updated time.

## Creation and lifecycle

New Skill offers Blank, Import, and From Existing where supported. A draft editor requires identity, purpose, activation mode, instructions or structured procedure, inputs/outputs, dependencies, tools/connectors, policy requirements, tests, and target preferences. Advanced fields may be grouped, but governance fields cannot disappear from review.

Edit creates or changes a draft. Duplicate creates a separate identity with a provenance link. Publish, deprecate, archive, and restore use explicit lifecycle rules from the data service. Do not equate Published with deployed, tested, compatible, or authorized. Deploy routes through Compile and deployment preflight for the selected version.

Versions displays immutable versions, parent lineage, diff, test and compile evidence, affected dependents, and deployment links. A version recommendation must explain whether compatibility or behavior changed; do not fabricate automatic SemVer judgment.

## States and acceptance

Provide empty library with Create and Import actions, filtered-empty reset, loading, unavailable service, permission-limited view, stale dependency, draft conflict, and deleted or inaccessible asset states. A deprecated dependency stays visible and points to remediation; it does not disappear from the graph.

Accept when row selection opens the correct inspector, tabs preserve the selected version, editing has a dirty-state guard, duplicate preserves provenance without sharing mutable identity, and Used By links reach actual dependent fixtures. Publishing or deploying must not be a cosmetic status toggle. Status-card totals must use declared categories and reconcile with the library.

<!-- PAGE -->
# 13 Workflows

Reference: S08. Purpose: compose agents, skills, tools, policies, and approvals into an inspectable executable plan.

## Layout and graph editing

Preserve the hero, Create Workflow and Run Workflow actions, summary cards, left Workflow Library, central graph, and right inspector. Library filters include ownership, sharing, template origin, workflow type, and status. Keep the selected workflow name, version, Save, Test Run, and Deploy above the canvas.

The graph supports trigger, agent or skill step, tool step, policy check, condition, approval gate, and completion nodes. Additional node types come through declared SDK contributions. Ports carry typed input/output contracts. The inspector edits the selected node or, when no node is selected, shows workflow Overview, Versions, Runs, and Policies.

Provide add, connect, disconnect, move, delete, duplicate, zoom, Fit, minimap, undo, and redo. Offer an ordered outline and connection editor for keyboard use. Preserve node IDs through layout changes. Validate missing inputs, incompatible ports, dangling edges, unsupported nodes, unreachable steps, and cycles according to the workflow model selected by Codex.

## Runs and governance

Test Run uses Test Lab with a pinned workflow version. Run Workflow and Deploy require explicit environment, harness/runtime, connectors, and authority profile. A policy node expresses a requirement; it is not proof that an external effect is authorized. Approval nodes link to exact backend approval records. Branch conditions must not bypass a required execution-authority check.

Runs show node-level state, timing, redacted inputs/outputs, blocked steps, approval waiting, cancellation requests, and unknown outcomes. Parallel branches need deterministic join semantics supplied by Codex. Do not invent workflow execution semantics in the graph component.

The invoice example from S08 may be used only as synthetic fixture content. Separate environment support from harness compatibility in the inspector; never infer production readiness from Local/Staging/Production labels.

## Acceptance

Build a branching fixture, edit a node, save a draft, restore it after navigation, and run a deterministic simulation. A missing connector or policy requirement blocks the affected step with an explanation. Removing an extender leaves its node visible as unresolved. Graph and outline edits produce the same stored graph. Deploy opens preflight, and a successful simulation does not mark a workflow deployed.

<!-- PAGE -->
# 14 Connectors

Reference: S07. Preserve the distinction in its hero: access to a system and authority to perform an action are separate.

## Catalog and details

Use the banner, Add Connector action, five metrics, state tabs, filters, card grid, and right inspector. Cards display connector identity, category, tags, and connection status. Detail tabs are Overview, Configuration, Permissions, Usage, and Logs. Show the selected connector version, environment, last check, health details, and dependencies.

The screenshot catalog is illustrative. GitHub, AWS, Slack, Stripe, databases, and other logos do not establish implemented connectors. Codex must supply a support matrix that distinguishes installable, configured, connected, unavailable, disabled, and planned. “Available” must describe an actual supported package or be explicitly labeled a catalog preview.

## Configuration and permission behavior

Add Connector selects an implemented connector type, environment, credential reference, and allowed scope. The frontend must receive secret references or masked metadata rather than plaintext secret values. Actual credential entry, storage, connection testing, and revocation follow the verified service contract.

Permissions show two separate facts: connector access scope and effective requirements from the selected authority provider. In the default LNSAT integration, retain “via LNSAT” context. For a future provider, render its actual name and profile. A green “Connected” badge cannot turn a permission row into an execution grant.

Test Connection must identify whether the check is read-only or consequential. Edit Configuration displays permission changes and affected dependents. Disconnect presents impact on agents, workflows, and jobs, then reports the real service result. Historical evidence remains readable after disconnection.

## States and acceptance

Include pending configuration, checking, connected, degraded, expired credentials, denied access, incompatible adapter, disabled, unavailable, and unknown health. Redact logs by default and restrict access by workspace permissions. Retry cannot duplicate a consequential diagnostic call.

Acceptance uses one connected fixture, one expired-credential fixture, one unavailable adapter, and one connector with read access but no authorization for a write. Inspector actions must operate on the selected connector and environment. Disconnect must report dependent impact and cannot leave affected workflows falsely healthy. Grid counts and status tabs must reconcile.

<!-- PAGE -->
# 15 Agents and Harnesses

Sources: C1, Command Center fleet in S03, Skills compatibility in S02. Dedicated page layouts are proposed; use the same library and inspector patterns.

## Agents

The Agents page provides searchable list or table, status and harness filters, Create Agent, Import Agent, and a selected-agent inspector. Fields include identity, description, owner, harness and version, attached capability versions, assigned workflows, environment, runtime state, last run, and policy requirements.

Create or Edit selects a supported harness, model configuration reference where applicable, skills, connectors, policies, and deployment target. Model or harness choice must not imply expanded permissions. Show missing dependencies, incompatible assets, and data-policy restrictions before saving or running.

Separate configuration state from runtime state: a published configuration can have an offline agent; an online runtime may still be blocked from an action. Detail tabs should include Overview, Capabilities, Configuration, Runs, Policies, and Versions. Pause or Stop requests report the actual service result and in-flight work state.

Accept when a user creates a draft agent from a pinned skill, sees compatibility and connector issues, resolves a supported issue, and opens a test. An unreachable runtime shows unknown or offline status without inventing a stop confirmation. A copied agent receives a new identity and preserved provenance.

## Harnesses

The Harnesses page manages versioned import, compile, and test adapters plus detected or configured installations. Potential targets from C1 include Claude Code, Codex, Gemini CLI, GitHub Copilot, Cursor, Windsurf, and custom harnesses; each remains unverified until supported by adapter evidence.

Show harness identity, adapter version, installed runtime version if known, detection source, supported formats, activation features, tools/hooks support, compile support, test support, and compatibility restrictions. Detail tabs cover Overview, Capabilities, Configuration, Versions, and Diagnostics. Local detection requires the host integration described in section 2.

Use explicit compatibility levels: fully compatible, compatible with transformation, partial, unsupported, and manual review required. Add unverified when no evidence exists. Provide reasons at the feature level and identify the tested adapter/runtime pair. Compilation success alone is not proof of behavioral equivalence.

Accept when a target-version change recomputes compatibility and invalidates stale compile/test evidence; unsupported features remain visible; and a missing local installation does not masquerade as an unsupported file format. “Install” or “Update” appears only when a real installation mechanism is in scope.

<!-- PAGE -->
# 16 Test Lab simulations and evaluations

Conversation direction: C1. No dedicated screenshot was supplied. Use the shared shell with a suite library, run workspace, and result inspector.

## Test configuration

Select an immutable capability, agent, or workflow version; test suite; harness/runtime versions; fixture inputs; environment; authority profile; and evaluator versions. Distinguish static validation, simulated execution, and real harness execution before the user starts a job. State any external calls, data transfer, expected side effects, and known cost estimate when the backend can provide one.

The matrix compares activation accuracy, instruction adherence, tool selection, output correctness, schema conformance, policy behavior, portability, failure handling, latency, and token usage where measured. Each result includes method, input identity, evaluator, run ID, timestamp, and evidence link. A model judgment must be labeled as such rather than presented as deterministic proof.

## Results and interactions

The run header shows queued/running status, completed cases, elapsed time, and Cancel. The results table filters by target, test type, outcome, and regression. An inspector shows expected versus observed behavior, sanitized trace, policy decisions, and comparison against a selected baseline. Missing measurements display “Not measured,” not zero.

Rerun uses the same pinned inputs unless the user explicitly selects new versions. A changed dependency, evaluator, provider profile, or harness version creates a new run identity. Preserve failed and canceled evidence. Support partial results and timeout without marking the entire suite successful.

Simulations are controlled scenario runs with explicit fixture or sandbox boundaries. Evaluations are reusable scoring criteria and comparisons. They can share infrastructure with Test Lab, but their labels must not suggest live execution when only a simulation ran.

## Acceptance

Provide deterministic pass, failure, blocked, unsupported, canceled, and unknown fixtures. Demonstrate the same capability across at least two fixture target profiles with one transformation and one unsupported feature. This is interface evidence, not proof of live cross-harness execution.

The source conversation proposes three real target harnesses as a research objective; it does not establish that those adapters exist. Codex must select and prove live targets separately. No “equivalent” or “safe” badge may be inferred solely from a green chart, an average score, or one successful sample.

<!-- PAGE -->
# 17 Compile

Conversation direction: C1; entry point shown in S05 and the shared navigation. No dedicated Compile screenshot was supplied.

## Purpose and layout

Compile converts reviewed canonical assets into target-native artifacts through versioned harness adapters. It does not execute a workflow, install files, or deploy by itself. Use an input/version summary, target matrix, output file tree and diff viewer, and diagnostics inspector.

The preflight pins capability or workflow version, dependency lock, target harness and adapter versions, compile options, environment profile if relevant, and authority requirements. Report unsupported fields, transformations, manual actions, permission changes, unresolved dependencies, and test coverage before producing a bundle.

## Preview and output contract

For each target, show compatibility status, transformed features, omitted or unsupported behavior, proposed filenames and paths, artifact digest, and evidence links. Native artifacts may include instruction files, manifests, hooks, configuration, or scripts only where the adapter contract supports them. Do not claim a target filename convention from a logo; Codex must supply the actual adapter specification.

Output must retain source provenance and version identity through a manifest. Provide readable source-to-output mappings and a diff against the chosen baseline. Unresolved governance requirements and incompatible schema changes block release-ready output. Draft previews may remain available with blocking diagnostics clearly visible.

Review & Compile from Merge & Split must carry the reviewed transformation and selected output version into this preflight. Download or Export produces a local bundle only when implemented; Deploy proceeds to a separate plan with environment and authority checks.

## States and acceptance

Support no target, unsupported target, preflight blocked, queued, compiling, partially compiled, failed, canceled, and complete. Partial success must identify which targets produced artifacts and which did not. A new source edit makes prior output stale; it cannot silently reuse the old test or compile badge.

Accept when one synthetic canonical asset produces deterministic mock output for a supported target, exposes a transformation on another, and blocks an unsupported requirement. The same pinned input must identify the same intended artifact content under the chosen compiler contract. The packet must distinguish generated fixture files from real compiler output. No compile action may write to a live harness directory as a side effect.

<!-- PAGE -->
# 18 Deploy and runtime setup

Required direction: U2. Conversation direction: C1. Deployment and provider setup are related operational surfaces with separate state machines.

## Rangoon setup with separate LNSAT

The setup flow must support the intended distribution modes: use a compatible bundled LNSAT runtime, download an approved release when available, or connect to an existing compatible installation. These are product requirements, not evidence of a current supported LNSAT package or installer.

Show runtime identity, version, source, platform compatibility, integrity verification status, install location, service ownership, and update policy. Steps are detect, choose source, verify prerequisites, install or connect, check health, and check the authority profile. If no supported artifact exists, explain that setup path is unavailable and retain draft/offline UI access where appropriate.

Do not fabricate a download URL, silently start an unverified service, or treat a health response as semantic compatibility. Future compatible providers use the same setup envelope with provider-specific capability checks. Uninstalling Rangoon must not silently erase shared provider data or evidence.

## Deployment planning

Deploy selects immutable compiled artifacts, environment, destination, required connectors, harness/runtime versions, authority provider, and rollout strategy. Preview exact file changes or service operations, impacted assets, policy results, required approvals, and rollback prerequisites. A deployment plan is distinct from approval to execute it.

Approval requests must bind the exact plan or action scope according to the provider’s real contract. Changes invalidate affected approvals. A valid policy response alone must not be presented as an execution receipt. Submit once through the application service and track the returned operation identity.

## Operational states and acceptance

Show draft plan, blocked, awaiting approval, ready, queued, deploying, partially deployed, succeeded, failed, outcome unknown, and reconciliation required. On timeout or lost connection, preserve the operation ID and query status. Do not automatically replay deployment or fall back to another provider. Rollback is a new governed operation with its own plan and outcome.

Accept a fixture sequence from Compile to deployment preview, denied approval, successful authorized deployment receipt, and interrupted execution with an unknown outcome. The unknown case must not claim success or offer an unsafe blind retry. Production deployment, daemon installation, and provider migration remain separately authorized implementation lanes.

<!-- PAGE -->
# 19 Analytics and Policies

Sources: C1; visual patterns in S03 and S07. Dedicated layouts are proposed.

## Analytics

Use a time-range and scope toolbar, metric cards, trend charts, a comparison table, and evidence drill-down. Filters include workspace, environment, agent, skill/workflow version, harness, connector, and authority provider where data exists. Candidate measures include run outcomes, blocked actions, pending approval time, test regressions, deployment outcomes, latency, usage, and capability adoption.

Codex must provide each metric’s numerator, denominator, time basis, unit, refresh policy, source, and treatment of unknown or missing results. Show sample size and data freshness. Use accessible tables alongside charts. Tooltips must disclose the interval and category; color cannot be the sole distinction. Cost is shown only with verified pricing or recorded billing data.

Empty means no data in the selected scope, not zero risk. Partial telemetry requires a coverage note. Accept when chart totals reconcile with filtered records, blocked and failed remain separate, and switching periods updates every relevant panel. Do not label a system compliant from an unexplained percentage.

## Policies

Provide a searchable policy library, status and scope filters, policy detail, version history, simulation results, and linked assets. Policy scope can include tools, connectors, environments, models, data classes, skills, workflows, and deployments, as described in C1.

Policy configuration is a proposal until validated and accepted by the actual provider or governance service. Separate policy definition, policy attachment, effective evaluation, approval requirement, authorization, and execution outcome. Display provider, policy/profile version, evidence time, and unknown state.

Editing shows an explicit diff, affected dependents, and permission expansion or restriction. Simulation is labeled as simulation and does not grant authority. “Apply” or “Publish” must use the supported backend route and approval rules. Existing LNSAT semantics must come from its contract; a future provider must report differences through compatibility mapping.

Accept fixtures for allow, deny, approval required, invalid policy, stale evaluation, unsupported provider semantics, and unavailable evaluation. A client-side role or green badge must never bypass a denial. Exact action approval remains scoped to the provider record, and any changed target, arguments, version, or policy context must trigger the contract’s invalidation rules.

<!-- PAGE -->
# 20 Supporting pages and Extenders

These destinations are visible in S02–S08 or required by U1/U2. Their initial scope must be explicit so navigation forms a coherent product.

| Surface | Minimum useful interface and acceptance |
| --- | --- |
| Approvals | Queue by environment/provider/status, exact scope preview, requested actor, expiry, decision record, and evidence link. Approve/deny only through the verified service; stale or changed scope blocks action. |
| Audit Explorer | Searchable event timeline and detail with provider identity, correlation IDs, redacted evidence, source time, and integrity status when verified. Do not claim universal signatures or immutable storage. |
| Impact Analysis | Dependency graph and affected-item list for edits, version changes, permission changes, and plugin removal. Explicitly identify incomplete dependency coverage. |
| Environments | Named target profiles, configuration references, provider/harness bindings, health, and readiness checks. Local, Staging, and Production are examples, not evidence of deployed environments. |
| Templates | Browse, preview, version, and instantiate a template as a new draft with provenance. A template cannot secretly execute its setup content. |
| Registry | Search and inspect published asset/package metadata, versions, dependencies, compatibility, and provenance. Separate preview, download, install, and publish; no marketplace or license service is implied. |
| Settings and Activity | Workspace/profile preferences, theme, provider setup, supported integration settings, and readable activity records. Do not invent team membership or authentication implementation. |

## Extenders page

Use the Connectors catalog-and-inspector pattern for SDK-based plugins and contributions described in section 4. Offer Installed, Available, Updates, and Needs Attention views only when those records exist. Filters include extension family, publisher, compatibility, and status.

A package inspector shows its manifest, version, contributions, permission needs, configuration, dependencies, changelog, diagnostics, and provenance checks. Install and enable are separate states. An update displays permission and compatibility differences before applying. Removal shows dependent assets and offers a supported disable or migration path.

## Coherent initial scope

Every listed page must either provide its accepted minimum interface or clearly explain why its service is unavailable. Demonstration data can support navigation and state verification, but cannot imply real publishing, billing, identity, provider compatibility, or deployment support. Codex must record whether each page is visual-only, interactive with fixtures, or connected to a verified service.

<!-- PAGE -->
# 21 Component contracts

Proposed interface boundaries; reconcile names and types with the repository. Components receive typed data and emit intents. Application services own persistence, jobs, permission enforcement, and external effects.

| Component | Inputs and emitted events | Required invariant |
| --- | --- | --- |
| AppShell | workspace, navigation, health, theme; navigate, changeTheme | One active route; unavailable health is not green. |
| PageHero and MetricCard | title, summary, artwork, actions; activate or drillDown | Real text and buttons; metrics carry unit, period, and freshness. |
| AssetLibrary and Inspector | query, rows, selectedId, selectedVersion, tabs; select, filter, open, edit | Stable identity and keyboard selection; no accidental checkbox or row-action collision. |
| SourceTree and EvidenceViewer | immutable source snapshot, spans, selected file; select, revealSpan | Inert text; line references bind to a file digest. |
| PipelineStepper and JobPanel | stages, current state, job progress; navigateStage, cancel, resume | Cannot skip required review; unknown progress is indeterminate. |
| CapabilityGraph | nodes, typed edges, selection, diagnostics; graphChange, select, undo | Stable IDs; no hidden execution; synchronized outline representation. |
| TransformPreview and ConflictPanel | input versions, proposed output, findings; resolve, saveDraft, review | Unresolved blocking findings prevent release-ready output. |
| CompatibilityMatrix | target/profile versions, feature results, evidence | Support is per version and feature; unknown remains visible. |
| DiffViewer and EvidenceTimeline | before/after or events; selectChange, openEvidence | Preserve provenance, provider identity, and redaction. |
| ActionReview and ApprovalPanel | exact proposed scope, impact, provider state; requestApproval, submit | UI confirmation is not authorization; use backend evidence. |

## Form and action contract

Each mutation defines required fields, validation errors, dirty state, allowed roles or capabilities, pending state, success result, retry policy, and conflict behavior. Use version preconditions for concurrent editing. An edit conflict must preserve the local draft and offer comparison; it must not silently overwrite another revision.

All list components need loading, empty, filtered-empty, error, stale, and permission-limited variants. All dialogs need accessible names, initial focus, Escape behavior where safe, and focus restoration. Buttons must have visible pending and disabled explanations. Save as Draft never implies publish, deploy, or approve.

## Contract evidence

Codex supplies examples for every component state and identifies the owner of each type. Astra delivers reusable components plus route composition, not independent copies of tables or inspectors on each page. Component demos must use the same fixture records as the end-to-end lifecycle.

<!-- PAGE -->
# 22 State and data models

Proposed frontend and application models below are not an existing backend schema. Codex must supply versioned definitions, validation rules, and mappings to actual provider contracts.

| Record | Minimum fields and relationships |
| --- | --- |
| SourceSnapshot and EvidenceSpan | ID, source kind, location reference, revision/digest, file path, byte or line range, parser version, captured time, redaction status. |
| Capability and CapabilityVersion | Stable ID, kind, name, owner/workspace; immutable version, content digest, activation, instructions, inputs/outputs, dependencies, tools, scripts/references, policy requirements, tests, lineage, compatibility. |
| Candidate and Transformation | Candidate type, evidence spans, classifier metadata, review decisions; merge/split/decompose operation, pinned inputs, mapping ledger, conflicts, outputs, actor, timestamp. |
| WorkflowGraph | Workflow/version, nodes with type and schema version, typed ports/edges, dependency refs, validation findings, layout, runtime semantics reference. |
| Agent Harness Connector | Agent configuration and runtime identity; harness and adapter versions/support; connector instance, environment, credential reference, access scope, health. |
| TestRun and CompileArtifact | Input/dependency identity, target versions, evaluator/compiler version, job state, results, evidence, output manifest, transformations, diagnostics, digest. |
| Deployment and AuthorityEvidence | Plan/artifact identity, environment, provider/profile, operation ID, approval and decision refs, receipt refs, status, unknown/reconciliation metadata. Preserve native records. |
| ExtensionPackage | Manifest/version, SDK range, contribution kinds, dependencies, configuration refs, requested permissions, validation/enable state, provenance. |

## State dimensions remain separate

An asset lifecycle may be draft, in review, published, deprecated, or archived. Compatibility may be full, transformed, partial, unsupported, review required, or unverified. A job may be queued, running, awaiting input, cancel requested, canceled, succeeded, failed, or outcome unknown. These must not collapse into one generic status field.

Readiness is derived from selected versions, resolved dependencies, applicable test evidence, compile evidence, environment support, and authority requirements. Never persist a frontend “safe” boolean as authority. A passed test is scoped to its inputs and versions; a new dependency makes it stale.

## Transport and persistence

For each proposed service operation, Codex supplies request, response, validation, error shape, authentication/workspace scope, idempotency behavior, timeout, and polling or event semantics. Service families include list/detail, draft save with expected revision, import/analyze, transform, test, compile, plan deploy, submit governed action, and reconcile.

Use structured errors with code, readable message, field issues, retryability, correlation ID, and job ID when present. Frontend errors must not expose raw secrets or provider payloads. Store view preferences separately from business records; define whether unsaved drafts survive refresh and how sensitive content is retained or removed.

<!-- PAGE -->
# 23 Interactions responsiveness and accessibility

## Shared interaction rules

Filters and sorting preserve selection when possible and explain when the selected item leaves the result set. Save feedback reflects actual persistence. Autosave, if chosen, must distinguish saving, saved, failed, and conflict states. Navigation away from unsaved edits offers save, discard, or cancel. Cancellation is a request until the service confirms it.

A repeated read or safe analysis may be retried according to the service contract. Consequential operations use recorded operation identity and reconciliation; an ambiguous result must not trigger blind replay. Event updates need ordering or revision handling so stale messages cannot overwrite newer state.

## Proposed responsive behavior

At 1440 px and above, preserve the reference’s sidebar, working grid, and inspector. Around 1024–1439 px, allow a narrower or collapsed sidebar, fewer metric columns, and an inspector drawer or tab. Below 768 px, use an off-canvas navigation menu, stacked cards, compact banners, a single main task region, and a dedicated detail view.

For Decompose, progressively replace the four-pane layout with Source, Candidates, and Details tabs while retaining evidence links. Merge/Split and Workflows offer canvas plus an editable outline; the page itself must not require horizontal scrolling. Wide data tables may have their own labeled scroll region or a card alternative. Sticky controls cannot cover focused fields or the last row.

Validate at 1536 × 1024, 1672 × 941, 1280 × 800, 1024 × 768, 768 × 1024, and 390 × 844. Include long names, many tags, translated-length copy fixtures, large counts, and 200% text zoom. These are proposed test sizes, not a claim of existing support.

## Accessibility target

Target WCAG 2.2 AA. Provide semantic landmarks and headings, keyboard operation, visible focus, sufficient contrast, meaningful labels, status announcements, reflow, and error identification. Use text contrast of at least 4.5:1 for normal text and 3:1 for large text; necessary control and graphical boundaries need 3:1. Meet 24 × 24 CSS pixel minimum targets or applicable spacing exceptions. Provide alternatives to dragging and avoid obscuring keyboard focus. Source: W3C WCAG 2.2 Quick Reference, https://www.w3.org/WAI/WCAG22/quickref/.

Product-specific checks include keyboard graph connections, accessible code/source viewing, table headers and selection announcements, chart data alternatives, reduced motion, useful live-job announcements without log spam, and decorative-image alt treatment. Third-party icons need readable names; color and logos alone cannot convey harness or policy status.

Codex must name automated accessibility checks and manual keyboard/screen-reader checks in the packet. Passing an automated scan alone does not establish accessibility acceptance.

<!-- PAGE -->
# 24 Implementation sequence and acceptance gates

This is the proposed sequence. Codex selects exact owned files and commands after repository inspection. Do not assign durations or percent-complete claims without evidence.

| Stage | Bounded deliverable | Required exit evidence |
| --- | --- | --- |
| 0 Evidence packet | Repository truth, references, asset plan, stack/runtime facts, DTOs, scope and fixtures | Gaps explicitly resolved or assigned; exact authority and file ownership recorded. |
| 1 Shared shell | Tokens, navigation, heroes, metrics, tables, inspectors, dialogs, themes | Reference-size captures; keyboard navigation; no dead destinations. |
| 2 First lifecycle slice | Import → Decompose → Skills draft | Source evidence preserved; no source execution; refresh/resume; failure states. |
| 3 Composition | Merge, Split, History, Workflows graph and outline | Conflict blocking, assignment coverage, lineage, undo, version staleness. |
| 4 Target pipeline | Harness selection → Test Lab → Compile → Deploy preview | Explicit compatibility, scoped evidence, transformations, unknown-outcome fixture. |
| 5 Operations | Command Center, Agents, Connectors, Policies, Analytics and supporting pages | Counts reconcile; permissions remain separate from health; all drill-downs work. |
| 6 Extensions and setup | SDK contracts, Extenders, LNSAT setup and provider capability UI | Incompatible package/provider blocked; unavailable install path honest. |
| 7 Verified integrations | Only separately scoped backend/provider implementations | Real adapter tests, exact contracts, installation/runtime proof where authorized. |

## Acceptance categories

**Visual fidelity:** S02–S08 route captures reproduce the hierarchy, shell, cards, inspectors, and working layouts. S01 precedence and all deviations are recorded. Missing production artwork remains an explicit asset gap rather than a hidden substitution.

**Functional behavior:** Every visible control has an implemented result or a clear unavailable explanation. The canonical fixture travels through import, review, decomposition, merge/split, test, compile, and deployment preview with stable identity and lineage.

**Truth and governance:** Mock versus live is visible. No support claim comes solely from a screenshot. Connector access, policy evaluation, approval, authorization, and outcome remain distinct. Unknown outcomes block blind replay. LNSAT stays separate and future providers require compatible semantics.

**Quality:** Repository-native lint, type checks, relevant tests, production build, visual review, and accessibility checks pass where applicable. Codex records exact commands, exit results, and evidence paths; not-run checks remain not run.

Use fresh independent review against the accepted packet and exact changes. The producer cannot approve its own work. Codex resolves findings and owns integration; green tests or review do not authorize merge, deployment, or release.

<!-- PAGE -->
# 25 Concrete handoff checklist

## Before Codex assigns implementation

- [ ] Name the canonical issue or packet and human acceptance owner; attach this brief and the latest U2 clarification.
- [ ] Record repository path, branch, HEAD, status, first-read docs, allowed files, and unrelated dirt.
- [ ] Supply S01–S08 originals and manifest; choose S03/S01 precedence explicitly and identify production artwork gaps.
- [ ] Supply working start/build commands, stack versions, route inventory, existing components, and design tokens.
- [ ] Define browser versus host capabilities, actual storage, supported source formats, analysis destination, and data-transfer rules.
- [ ] Supply versioned DTOs, errors, fixture records, mock/live adapters, job transitions, and concurrency rules.
- [ ] Supply LNSAT and future-provider compatibility boundaries, setup availability, SDK/module/plugin contracts, and forbidden authority shortcuts.
- [ ] Identify exact first slice, owned files, acceptance cases, named validators, review lane, and separately authorized external actions.

## Assignment text for Astra

Implement the named Rangoon interface slice in the supplied repository and owned files. Treat this accepted packet as the work authority. Use the supplied screenshot originals, reference mapping, design tokens, component contracts, fixtures, and service interfaces. Preserve Rangoon as the capability-management product and LNSAT as a separate bundled or downloadable authority system; future providers must meet explicit compatibility requirements. Keep SDK contributions behind versioned contracts. Do not infer working integrations, installer support, compatibility, metrics, or authority from mockups. Report contract gaps before implementing dependent behavior. Deliver reviewable code, relevant tests, state captures, exact validation results, and unresolved limitations. Do not merge, deploy, install daemons, or mutate production unless the packet separately authorizes that action.

## Before Codex accepts the result

- [ ] Inspect exact changed paths and confirm one writer owned overlapping files.
- [ ] Run and record repository-native validators; inspect normal, empty, loading, error, blocked, stale, and unknown states.
- [ ] Compare each implemented reference route at native dimensions; review derived pages and responsive variants.
- [ ] Exercise keyboard, graph-outline, source-evidence, dirty-draft, conflict, and accessibility flows.
- [ ] Verify provenance, version invalidation, SDK compatibility, provider identity, and mock/live separation.
- [ ] Resolve fresh independent review findings; record any remaining gaps and their acceptance owner.
- [ ] Report acceptance state, producer/reviewer role and model/provider, commit/push/deploy state, and external actions not taken.

## Current document handoff state

The textual context and all eight references are available. Repository facts, live APIs, clean artwork layers, exact tokens, SDK schemas, supported installer artifacts, and verified adapter capabilities remain inputs Codex must obtain. This brief specifies how to make those inputs concrete; it does not certify them or approve the implementation.
