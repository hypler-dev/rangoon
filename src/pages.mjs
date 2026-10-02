export const pages = [
  {
    path: '/product/',
    eyebrow: 'Product',
    title: 'One control plane.',
    accent: 'Many agent systems.',
    description: 'Rangoon is a design-preview control plane for composing, governing, testing, and preparing portable agent capabilities across changing harnesses.',
    image: 'command-center',
    label: 'Design preview',
    sections: [
      { title: 'Compose from durable parts', text: 'Model instructions, skills, workflows, connectors, context, and policy requirements as related assets instead of scattered files.', items: [{ title: 'Canonical capability model', text: 'Keep provenance, dependencies, compatibility notes, and versions close to the capability they describe.' }] },
      { title: 'See operational context', text: 'The Command Center preview brings fleet state, pending approvals, policy signals, deployments, and connector health into one operator view.', items: [{ title: 'Evidence before assumptions', text: 'Preview the configuration and relationships behind a change before treating it as ready for a target environment.' }] },
      { title: 'Prepare deliberate delivery', text: 'Compilation, deployment, activation, and execution authorization remain separate decisions in the planned product workflow.', items: [{ title: 'Active development', text: 'Product interfaces and supported integrations are evolving before a stable release.' }] }
    ]
  },
  {
    path: '/product/agents/',
    eyebrow: 'Agent fleet',
    title: 'Build a fleet.',
    accent: 'Keep it accountable.',
    description: 'Design agent profiles with explicit roles, capability bundles, harness assignments, policy posture, and operational ownership before they enter a governed workflow.',
    image: 'governed-command-center',
    label: 'Design preview',
    sections: [
      { title: 'Profiles with useful detail', text: 'An agent profile can connect purpose, model and harness choices, assigned skills, context sources, memory settings, connectors, and owners.', items: [{ title: 'Effective configuration', text: 'Inspect the exact versions and assets that would shape an agent at a given point in time.' }] },
      { title: 'Operate across runtimes', text: 'A shared view is intended to help teams compare active, paused, degraded, and offline agents without erasing runtime differences.', items: [{ title: 'Compatibility is visible', text: 'Harness-specific constraints remain part of the record instead of becoming hidden assumptions.' }] },
      { title: 'Change with traceability', text: 'Planned lifecycle actions include cloning, simulation, testing, promotion, pausing, and inspection with version and audit history.', items: [{ title: 'Capability is not authority', text: 'An assigned skill does not itself authorize consequential execution.' }] }
    ]
  },
  {
    path: '/product/skills/',
    eyebrow: 'Skill registry',
    title: 'Make skills durable.',
    accent: 'Make reuse deliberate.',
    description: 'Turn reusable procedures and instructions into versioned, inspectable capabilities with provenance, dependencies, activation conditions, and target-aware compatibility.',
    image: 'skills',
    label: 'Design preview',
    sections: [
      { title: 'Treat instructions as assets', text: 'A planned registry records identifiers, content digests, owners, inputs, outputs, tools, references, tests, and rollout state.', items: [{ title: 'Lifecycle clarity', text: 'Draft, validate, test, review, publish, deploy, deprecate, and archive are distinct stages.' }] },
      { title: 'Merge without erasing source', text: 'Compare overlap, conflicts, shared procedures, and dependencies before choosing a composite, shared-core, sequential, or conditional design.', items: [{ title: 'Preserved ancestry', text: 'Original assets and proposed relationships remain traceable through review.' }] },
      { title: 'Split for a reason', text: 'Decomposition proposals can separate always-on project context from focused procedures, tool requirements, or policy-sensitive work.', items: [{ title: 'Human review stays central', text: 'Suggested classifications and splits are evidence-backed proposals, not automatic truth.' }] }
    ]
  },
  {
    path: '/product/workflows/',
    eyebrow: 'Workflows',
    title: 'Map the work.',
    accent: 'Expose the decisions.',
    description: 'Design multi-step agent operations as understandable flows of skills, tools, approvals, policy gates, data transformations, and completion evidence.',
    image: 'workflows',
    label: 'Design preview',
    sections: [
      { title: 'A graph with semantics', text: 'Planned workflow nodes can represent triggers, agents, skills, tools, branches, waits, subflows, exception handling, and audit events.', items: [{ title: 'Meaningful connections', text: 'A data flow, required dependency, approval gate, and tool invocation should read as different relationships.' }] },
      { title: 'Govern the consequential moments', text: 'Policy and approval gates belong within the operational path where people can see what is requested and why.', items: [{ title: 'Receipts and states', text: 'Completion evidence and outcome states are designed to be explicit rather than implied by a chat transcript.' }] },
      { title: 'Start simple, extend carefully', text: 'The workflow experience is being designed for understandable small flows as well as more advanced orchestration.', items: [{ title: 'Preview status', text: 'Workflow editing and runtime behavior remain in active development.' }] }
    ]
  },
  {
    path: '/product/capability-studio/',
    eyebrow: 'Capability Studio',
    title: 'Configuration becomes software.',
    accent: 'Not loose prompt text.',
    description: 'Bring existing agent configuration into a structured lifecycle: discover, decompose, compose, test, compile, and prepare for governed deployment.',
    image: 'import-analyze',
    label: 'Design preview',
    sections: [
      { title: 'Discover structural evidence first', text: 'Planned import can inspect supported configuration bundles and known files while treating imported content and scripts as untrusted input.', items: [{ title: 'No execution on import', text: 'Discovery is intended to analyze structure and content, not run imported scripts.' }] },
      { title: 'Propose reusable parts', text: 'Source lines may become project context, scoped coding rules, testing procedures, deployment skills, or supporting references.', items: [{ title: 'Line-level provenance', text: 'Reviewers can see what source evidence produced a proposed asset.' }] },
      { title: 'Compose an explicit graph', text: 'Connect instructions, schemas, tools, conditions, policies, approvals, references, and dependencies with their intended meaning.', items: [{ title: 'Design, then verify', text: 'The studio is a preview of planned product behavior, not a released runtime.' }] }
    ]
  },
  {
    path: '/product/connectors/',
    eyebrow: 'Connectors',
    title: 'Connect systems.',
    accent: 'Keep boundaries clear.',
    description: 'Plan explicit operations for external systems so access, credentials, permissions, policy authorization, and execution never collapse into one assumption.',
    image: 'connectors',
    label: 'Design preview',
    sections: [
      { title: 'Operations, not ambient access', text: 'Connector contracts are intended to describe supported operations, inputs, outputs, side effects, idempotency needs, and policy-relevant metadata.', items: [{ title: 'System families', text: 'Potential adapters span source control, cloud, databases, CRM, ticketing, communications, observability, and security tools.' }] },
      { title: 'Separate every boundary', text: 'Installing a connector, enabling it, assigning credentials, granting permission, and authorizing a request are distinct stages.', items: [{ title: 'Clear review surface', text: 'Operators should be able to inspect the intended resource and expected side effect before approval.' }] },
      { title: 'Governed execution path', text: 'LNSAT provides the proposed authorization and evidence foundation; connector coverage and contracts are still being developed.', items: [{ title: 'No implied integration', text: 'This page does not claim any connector is currently shipped or configured.' }] }
    ]
  },
  {
    path: '/product/harnesses/',
    eyebrow: 'Harness Manager',
    title: 'Know the target.',
    accent: 'Respect its limits.',
    description: 'Track the runtime environments that may host managed capabilities and make compatibility evidence part of every planned deployment decision.',
    sections: [
      { title: 'Describe the runtime honestly', text: 'A planned harness record can cover runtime family and version, asset support, model and tool capabilities, hooks, context behavior, health, and drift.', items: [{ title: 'Target-aware preparation', text: 'Deployment planning starts from what a target can represent, not from a promise that all harnesses behave alike.' }] },
      { title: 'Classify compatibility', text: 'Proposed reports distinguish fully compatible, compatible with adaptation, partially compatible, and unsupported outcomes.', items: [{ title: 'Explain the reason', text: 'Warnings may point to missing tools, unsupported hook lifecycle, context limits, schemas, or permission models.' }] },
      { title: 'Use adapters as versioned boundaries', text: 'Harness adapters are planned to discover, import, map, compile, test, and report compatibility for a supported target.', items: [{ title: 'Support is evolving', text: 'No individual harness adapter is represented here as released or certified.' }] }
    ]
  },
  {
    path: '/product/policies/',
    eyebrow: 'Policy',
    title: 'Put policy in the graph.',
    accent: 'Where work happens.',
    description: 'Design governance as visible operational context for tools, connectors, data, environments, approvals, and consequential agent requests.',
    sections: [
      { title: 'Policy belongs near capability', text: 'Policy requirements can be associated with agents, skills, workflows, connectors, environments, data classes, and deployment targets.', items: [{ title: 'Concrete controls', text: 'Planned controls include tool access, sensitive data, external communication, infrastructure mutation, financial operations, secrets, and time windows.' }] },
      { title: 'Treat changes as operations', text: 'The intended lifecycle is draft, diff, simulate, review, approve, stage, roll out, observe, and roll back.', items: [{ title: 'Impact before rollout', text: 'A policy change should reveal affected capabilities, changed approvals, exceptions, and simulation evidence.' }] },
      { title: 'A foundation, not a claim', text: 'Rangoon is designed around LNSAT for execution authorization and evidence, with planned policy-engine adapters alongside it.', items: [{ title: 'Integration honesty', text: 'OPA, Cedar, and other policy-engine integrations are not claimed as shipped.' }] }
    ]
  },
  {
    path: '/product/test-lab/',
    eyebrow: 'Test Lab',
    title: 'Test behavior before trust.',
    accent: 'Compare evidence.',
    description: 'Prepare repeatable evaluation for activation, instruction adherence, tool selection, output structure, policy behavior, compatibility, and cross-harness differences.',
    sections: [
      { title: 'Test the right behavior', text: 'Planned suites cover whether capabilities activate when needed, stay quiet when they should, follow required steps, and avoid prohibited behavior.', items: [{ title: 'Tool and output checks', text: 'Evaluation can examine declared permissions, selected tools, structured output, and edge cases.' }] },
      { title: 'Compare targets carefully', text: 'The same capability can be measured across supported harnesses for adherence, success, policy violations, latency, token use, estimated cost, and errors.', items: [{ title: 'Differences are data', text: 'The goal is to reveal target-specific behavior, not flatten it into a compatibility claim.' }] },
      { title: 'Simulate before activation', text: 'Planned simulations explore configuration, skill, connector, model, policy, and rollout changes without activating them.', items: [{ title: 'Preview only', text: 'Test Lab capabilities are in active development.' }] }
    ]
  },
  {
    path: '/product/deployments/',
    eyebrow: 'Deployments',
    title: 'Deliver exact bundles.',
    accent: 'Keep activation separate.',
    description: 'Prepare versioned capability bundles with pinned components, compatibility evidence, approvals, and rollback readiness before any target receives a change.',
    sections: [
      { title: 'Pin the intended configuration', text: 'A planned managed bundle may include agent profile, skills, instructions, context, workflows, policy requirements, connector requirements, adapter version, and tests.', items: [{ title: 'Immutable identity', text: 'A bundle digest is intended to make the reviewed configuration part of later evidence.' }] },
      { title: 'Review a real plan', text: 'Before a deployment, the product direction includes target connectivity, dependencies, policy needs, tests, conflicts, and rollback readiness.', items: [{ title: 'Four separate questions', text: 'Target, assignment, permissions, approval, and activation each need an explicit answer.' }] },
      { title: 'Roll out with observation', text: 'Planned stages include development, staging, pilot, percentage or workspace rollout, pause, and rollback.', items: [{ title: 'No release claim', text: 'Deployment orchestration is a planned product surface and not an available production service.' }] }
    ]
  },
  {
    path: '/product/analytics/',
    eyebrow: 'Analytics',
    title: 'Measure operating reality.',
    accent: 'Not vanity.',
    description: 'Plan operational analytics around execution outcomes, approvals, policy decisions, capability use, target health, latency, and evidence quality.',
    sections: [
      { title: 'Metrics that guide action', text: 'The planned view includes successful and failed runs, blocked actions, approval rate, policy violations, utilization, compatibility, and connector reliability.', items: [{ title: 'Trace to a decision', text: 'A useful metric should help an operator improve a workflow, policy, target, or capability.' }] },
      { title: 'Understand distribution', text: 'Intended reporting can show how skills, workflows, agents, models, connectors, and harnesses are actually used.', items: [{ title: 'Context matters', text: 'Estimated costs and token consumption should be interpreted alongside outcomes and target behavior.' }] },
      { title: 'Keep claims grounded', text: 'Preview metrics in design imagery are sample data and should not be read as customer, production, or performance evidence.', items: [{ title: 'Active development', text: 'Analytics implementation and data contracts remain under development.' }] }
    ]
  },
  {
    path: '/developers/',
    eyebrow: 'Developers',
    title: 'Extend with intent.',
    accent: 'Keep extensions inspectable.',
    description: 'Rangoon is being designed around versioned extension points for skills, adapters, connectors, workflow nodes, policy packs, tests, and interfaces.',
    sections: [
      { title: 'Build at explicit boundaries', text: 'Planned extension areas include harness adapters, connector operations, workflow nodes, model profiles, test packs, deployment adapters, and UI extensions.', items: [{ title: 'Version the contract', text: 'Extension boundaries are intended to evolve through reviewable, documented compatibility rules.' }] },
      { title: 'Expose meaningful metadata', text: 'Third-party capability should be visible, inspectable, versioned, and governable instead of operating as an unexplained private bypass.', items: [{ title: 'Authority remains separate', text: 'Extension availability does not itself grant authority for consequential operations.' }] },
      { title: 'Follow development', text: 'The future source repository is available as a development reference; SDK and contract publication are still pending.', items: [{ title: 'Source link', text: 'https://github.com/hypler-dev/rangoon/' }] }
    ]
  },
  {
    path: '/docs/',
    eyebrow: 'Documentation',
    title: 'Read the direction.',
    accent: 'Build with context.',
    description: 'Documentation will describe Rangoon’s evolving canonical model, adapter boundaries, operational concepts, and governance principles as the project matures.',
    sections: [
      { title: 'Development source checkout', text: 'The future repository can be cloned to inspect work in progress. This is a source checkout, not a released install or runtime API.', code: 'git clone https://github.com/hypler-dev/rangoon.git', items: [{ title: 'Pre-release status', text: 'Interfaces, contracts, package names, and supported targets may change before stable release.' }] },
      { title: 'Start with the model', text: 'Planned documentation will explain capabilities, provenance, compatibility, policies, approvals, bundles, targets, and evidence.', items: [{ title: 'No invented commands', text: 'There is no published installation, deployment, or runtime command on this page.' }] },
      { title: 'Contribute with care', text: 'Open-source contribution details, licensing, and governance will be published with stable project materials.', items: [{ title: 'Development preview', text: 'Current site content reflects product direction, not a final reference manual.' }] }
    ]
  },
  {
    path: '/developers/adapters/',
    eyebrow: 'Harness adapters',
    title: 'Translate with evidence.',
    accent: 'Never pretend equivalence.',
    description: 'Versioned adapters are planned to discover native configuration, map supported concepts, compile canonical assets, and explain target-specific limits.',
    sections: [
      { title: 'A precise adapter job', text: 'An adapter should discover supported inputs, map them into the canonical model, report capabilities, generate target artifacts, and validate output.', items: [{ title: 'Version independently', text: 'Harness behavior changes quickly, so adapter evolution must remain inspectable and separately versioned.' }] },
      { title: 'Show the transformation', text: 'Planned compilation views should surface generated file structure, transformation rules, unsupported behavior, warnings, dependencies, and output differences.', items: [{ title: 'Compatibility is not binary', text: 'Adaptation and partial support need a clear explanation before a team relies on them.' }] },
      { title: 'Still being designed', text: 'No adapter SDK, supported harness list, certification, or production compatibility guarantee is announced here.', items: [{ title: 'Build for review', text: 'The intended ecosystem prioritizes traceable adapters over opaque conversions.' }] }
    ]
  },
  {
    path: '/developers/connectors/',
    eyebrow: 'Connector SDK',
    title: 'Describe the operation.',
    accent: 'Preserve the boundary.',
    description: 'Planned connector interfaces describe external actions explicitly so policy, approvals, side effects, credentials, and receipts can be evaluated with context.',
    sections: [
      { title: 'Declare what can happen', text: 'A connector contract may specify operations, required inputs, outputs, credentials, network access, side effects, idempotency, and receipt behavior.', items: [{ title: 'Metadata serves review', text: 'Policy-relevant operation details should be available before an action is authorized.' }] },
      { title: 'Keep credentials separate', text: 'Connector installation, enablement, credential assignment, agent permission, and action authorization are intentionally different controls.', items: [{ title: 'No ambient power', text: 'A connector should not turn a generic instruction into unbounded access.' }] },
      { title: 'Authority at execution time', text: 'Rangoon is designed to use LNSAT for consequential execution authorization and evidence.', items: [{ title: 'SDK status', text: 'Connector contracts and SDK materials are planned, not released.' }] }
    ]
  },
  {
    path: '/solutions/engineering/',
    eyebrow: 'Engineering',
    title: 'Keep agent practice portable.',
    accent: 'Keep review concrete.',
    description: 'Plan shared coding-agent context, testing procedures, repository rules, and delivery workflows across compatible tools without losing target-specific behavior.',
    sections: [
      { title: 'Bring scattered guidance together', text: 'Engineering teams often maintain project instructions, tool configuration, hooks, scripts, skills, and local conventions in unrelated places.', items: [{ title: 'A structured inventory', text: 'Rangoon is designed to identify and organize those capabilities with provenance and review state.' }] },
      { title: 'Reuse procedures intentionally', text: 'Testing and review skills can be modeled as versioned assets, then adapted for compatible harnesses with visible transformations.', items: [{ title: 'No false portability', text: 'Target constraints stay visible when a capability cannot be represented completely.' }] },
      { title: 'Govern delivery boundaries', text: 'Deployment procedures and infrastructure-changing actions can be designed around explicit policy and approval points.', items: [{ title: 'Active development', text: 'These are planned engineering workflows, not a claim of current production integration.' }] }
    ]
  },
  {
    path: '/solutions/security/',
    eyebrow: 'Security',
    title: 'Make authority explicit.',
    accent: 'Make evidence useful.',
    description: 'Design security-agent operations around declared capabilities, bounded connector actions, policy context, specific approvals, and reconstructable execution evidence.',
    sections: [
      { title: 'Configuration is not authority', text: 'Giving an agent a security skill should not silently authorize every operation that skill describes.', items: [{ title: 'Decision-time controls', text: 'The planned model evaluates consequential requests with their target, scope, policy, and expected side effect.' }] },
      { title: 'Review the actual request', text: 'Approval designs should name the requesting agent, resource, data, risk, policy, scope, expiration, and supporting evidence.', items: [{ title: 'No vague grants', text: 'The objective is to approve or deny an actual operation rather than an indefinite permission.' }] },
      { title: 'Reconstruct the path', text: 'Audit Explorer direction links proposal, effective configuration, policies, approvals, authorization, connector result, receipt, and evidence.', items: [{ title: 'Preview status', text: 'Security workflows and integrations are in active development.' }] }
    ]
  },
  {
    path: '/solutions/operations/',
    eyebrow: 'Operations',
    title: 'Run systems with context.',
    accent: 'Escalate the right moments.',
    description: 'Plan operational agent workflows that make tools, systems, policy gates, human approvals, outcome states, and recovery signals visible to the people responsible.',
    sections: [
      { title: 'See the operating picture', text: 'The Command Center direction includes workflow activity, approval queues, policy signals, deployment state, and connector health.', items: [{ title: 'Useful exceptions', text: 'Warnings, blocked actions, failures, recoveries, and pending reviews should surface before they become invisible backlog.' }] },
      { title: 'Model the procedure', text: 'Workflows can express triggers, decisions, branches, waits, subflows, exception handling, notifications, and audit events.', items: [{ title: 'Bounded automation', text: 'Policy and approval gates are designed to remain visible inside the work path.' }] },
      { title: 'Change with a recovery plan', text: 'Simulation, staged rollout, observation, pause, and rollback are planned operational primitives.', items: [{ title: 'Not a deployed service', text: 'Operations capabilities described here are product direction in active development.' }] }
    ]
  },
  {
    path: '/solutions/enterprise/',
    eyebrow: 'Enterprise',
    title: 'Scale governance.',
    accent: 'Keep one trust model.',
    description: 'Plan organization-wide capability management with environment hierarchy, delegated administration, shared libraries, policy lifecycle, evidence export, and rollout control.',
    sections: [
      { title: 'One platform, more structure', text: 'Potential enterprise capabilities include multiple workspaces, environment hierarchy, enterprise identity, advanced roles, and separation of duties.', items: [{ title: 'Shared without becoming opaque', text: 'An organization-wide registry can preserve ownership, provenance, dependency, and lifecycle information.' }] },
      { title: 'Control organizational change', text: 'Planned policy hierarchy, simulation, staged rollout, approval chains, evidence export, and retention controls address broad operational scope.', items: [{ title: 'Observe rollout health', text: 'Fleet-wide visibility should connect changes to outcomes and exceptions.' }] },
      { title: 'Licensing remains open', text: 'The project intends an open core with optional commercial value, but final licensing and edition boundaries are not yet published.', items: [{ title: 'No purchasing claim', text: 'This page does not offer an enterprise product, contract, or availability date.' }] }
    ]
  },
  {
    path: '/open-source/',
    eyebrow: 'Open source',
    title: 'Open where trust matters.',
    accent: 'Build in the open.',
    description: 'Rangoon is intended to be primarily free and open source so teams can inspect, self-host, extend, and contribute to governed agent management.',
    sections: [
      { title: 'A planned open foundation', text: 'Core product direction includes agent and capability management, workflow composition, portable formats, harness interfaces, testing, simulation, and local operation.', items: [{ title: 'Inspectable systems', text: 'Security, interoperability, portability, and verification are central reasons to keep foundational work visible.' }] },
      { title: 'Optional value at the edges', text: 'Potential commercial areas include specialized extenders, certified integrations, hosted operation, support, compliance packages, and lifecycle services.', items: [{ title: 'No private authority bypass', text: 'Paid extensions should not create a hidden path around LNSAT authority.' }] },
      { title: 'Follow development', text: 'The project’s future public repository is https://github.com/hypler-dev/rangoon/.', items: [{ title: 'Licensing TBD', text: 'Final license and edition boundaries will be published before stable release.' }] }
    ]
  },
  {
    path: '/lnsat/',
    eyebrow: 'LNSAT',
    title: 'Manage capability.',
    accent: 'Govern authority.',
    description: 'Rangoon is designed on the LNSAT execution authorization and evidence engine, keeping agent configuration separate from real-world authority to act.',
    sections: [
      { title: 'A separate responsibility', text: 'Rangoon addresses what agents exist, how they are configured, which assets they use, and where they may run.', items: [{ title: 'LNSAT addresses authority', text: 'LNSAT evaluates what is requested, which policy applies, whether approval is required, and what evidence records the outcome.' }] },
      { title: 'A conceptual path', text: 'Intent → Packet → Gateway → Policy → Approval → Authorization → Adapter → Receipt → Audit.', items: [{ title: 'Execution-time evaluation', text: 'A configured capability is not automatically authorized in every situation.' }] },
      { title: 'An ecosystem foundation', text: 'LNSAT is intended as an independent open-source authority layer usable by Rangoon, other products, and custom integrations.', items: [{ title: 'Product status', text: 'Rangoon integration surfaces are under active development.' }] }
    ]
  },
  {
    path: '/architecture/',
    eyebrow: 'Architecture',
    title: 'One canonical model.',
    accent: 'Many target outputs.',
    description: 'Rangoon is being designed around a structured intermediate representation that preserves agent capability meaning while adapters generate target-specific artifacts.',
    sections: [
      { title: 'Model the capability', text: 'A canonical representation can include identity, activation, instructions, inputs, outputs, tools, dependencies, resources, permissions, policies, compatibility, provenance, and tests.', items: [{ title: 'Targets are outputs', text: 'Harness-specific files become compilation targets rather than the permanent underlying source of truth.' }] },
      { title: 'Preserve provenance', text: 'Imported assets can retain repository, file, revision, lines, detection method, confidence, user modifications, ancestry, generated targets, and deployment history.', items: [{ title: 'No unexplained blobs', text: 'A capability should retain the evidence needed to understand where it came from.' }] },
      { title: 'Keep authority external to configuration', text: 'The architecture separates composition and management from LNSAT evaluation for consequential execution.', items: [{ title: 'Design in progress', text: 'This architecture describes planned product direction rather than a stable public contract.' }] }
    ]
  },
  {
    path: '/roadmap/',
    eyebrow: 'Roadmap',
    title: 'Build the foundation.',
    accent: 'Earn each layer.',
    description: 'Rangoon is in active development. The public direction focuses on a durable capability model, reviewable adapters, governed operations, and evidence-led delivery.',
    sections: [
      { title: 'Foundation', text: 'The intended starting point is a canonical capability model with provenance, dependencies, versioning, and compatibility evidence.', items: [{ title: 'Why first', text: 'Portable configuration needs a clear source model before target-specific compilation can be trustworthy.' }] },
      { title: 'Operational surfaces', text: 'Planned product areas include Capability Studio, Agent Fleet, Workflows, policy lifecycle, Test Lab, deployment planning, and Analytics.', items: [{ title: 'Iterative release', text: 'Interfaces and extension contracts may change as each layer is validated.' }] },
      { title: 'Ecosystem work', text: 'Future work includes harness adapters, connector boundaries, test packs, policy-engine adapters, and community extension paths.', items: [{ title: 'No dates promised', text: 'This roadmap is directional and does not announce release dates, certifications, or availability.' }] }
    ]
  },
  {
    path: '/community/',
    eyebrow: 'Community',
    title: 'Build the ecosystem.',
    accent: 'Keep the work inspectable.',
    description: 'Rangoon invites future contributors to help shape portable, governed agent-management systems through open discussion, source review, and practical extensions.',
    sections: [
      { title: 'A shared problem', text: 'Agent configuration is fragmented across instructions, skills, tools, workflows, connectors, local rules, and runtime-specific formats.', items: [{ title: 'A practical goal', text: 'Make useful capabilities easier to understand, transfer, test, and govern without pretending every target is identical.' }] },
      { title: 'Paths to contribution', text: 'The planned ecosystem includes core platform work, adapters, connectors, workflow nodes, policy packs, test packs, documentation, and design feedback.', items: [{ title: 'Evidence over guesswork', text: 'Contributions should preserve provenance, compatibility context, and clear operational boundaries.' }] },
      { title: 'Follow development', text: 'The future public repository is https://github.com/hypler-dev/rangoon/. Community process and contribution guidance are still being prepared.', items: [{ title: 'Early stage', text: 'No community program, forum, or support channel is claimed as live on this page.' }] }
    ]
  },
  {
    path: '/about/',
    eyebrow: 'About Rangoon',
    title: 'Capability needs control.',
    accent: 'Control needs evidence.',
    description: 'Rangoon is building toward an open, portable control plane for teams that want capable agent systems without treating configuration as unlimited authority.',
    sections: [
      { title: 'Why Rangoon exists', text: 'Modern agent systems can be capable while their instructions, rules, tools, and runtime configuration remain fragmented and difficult to inspect.', items: [{ title: 'A common layer', text: 'Rangoon is intended to organize those assets while preserving their target-specific differences.' }] },
      { title: 'What guides the work', text: 'Capability should be portable. Authority should be explicit. Governance should be operational. Open systems build trust. Humans remain in control.', items: [{ title: 'Design principle', text: 'Models may classify, explain, recommend, or escalate; they should not silently override deterministic policy.' }] },
      { title: 'Where it stands', text: 'Rangoon is in active development, with product surfaces, integrations, extension contracts, and licensing still evolving.', items: [{ title: 'Stay close to source', text: 'Follow the future repository at https://github.com/hypler-dev/rangoon/.' }] }
    ]
  }
];
