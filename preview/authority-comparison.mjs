// Dated public product research, never an adapter capability or permission grant.
export const AUTHORITY_RESEARCH_DATE = '2026-10-09';
const freeze = value => {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
};

export const AUTHORITY_GLOSSARY = freeze([
  { term: 'Permission', meaning: 'Who may perform which action on which resource, under stated conditions.' },
  { term: 'Approval', meaning: 'A separate reviewer decision for a bounded request. A policy allow result is not Rangoon approval.' },
  { term: 'Enforcement', meaning: 'The component that refuses an actual attempt. Controls cover only paths that pass through it.' },
  { term: 'Guardrail', meaning: 'A check for unwanted content or behavior. Passing it does not grant permission.' },
  { term: 'Evidence', meaning: 'A record of a decision, review or outcome. A decision log alone does not prove an action completed.' },
]);

export const AUTHORITY_COMPARISONS = freeze([
  {
    id: 'agentgateway',
    purpose: 'A checkpoint between an agent client and the servers it calls.',
    permissions: 'Rules select reachable MCP tools, prompts and resources using request context and identity claims.',
    placement: 'The gateway evaluates mediated MCP requests; bypassing the gateway bypasses this coverage.',
    approval: 'Access rules are not a human approval chain. Separate review and execution requirements still apply.',
    evidence: 'Access logs describe mediated requests; log configuration and redaction need qualification.',
    privacy: 'The gateway sees routed traffic. Hosting, payload logging and retention must be reviewed.',
    example: 'Illustration: permit a support agent to read a ticket, while denying a destructive tool.',
    limit: 'This target is standalone agentgateway; Kubernetes coverage and Rangoon connectivity are not established.',
    references: [{ label: 'MCP authorization', url: 'https://agentgateway.dev/docs/standalone/latest/documentation/mcp/mcp-authz/' }, { label: 'Access logs', url: 'https://agentgateway.dev/docs/standalone/latest/documentation/observability/access-logs/' }],
  },
  {
    id: 'cerbos',
    purpose: 'Answers whether a specific identity may perform an action on a resource.',
    permissions: 'Policies evaluate roles and attributes supplied for the principal, resource and requested actions.',
    placement: 'The application asks the policy decision point, then must enforce the returned decision itself.',
    approval: 'An action called approve can be permission-checked; that does not collect a human approval or establish a review chain.',
    evidence: 'CheckResources returns action decisions, optional policy metadata and a call ID for audit correlation.',
    privacy: 'A self-hosted PDP is possible. Request attributes and audit retention still require review.',
    example: 'Illustration: a finance reviewer may view an invoice only in the appropriate department.',
    limit: 'Correct identity/context and caller enforcement remain essential; an allow result is not an execution receipt.',
    references: [{ label: 'CheckResources API', url: 'https://docs.cerbos.dev/cerbos/latest/api/index.html' }],
  },
  {
    id: 'permit',
    purpose: 'Defines application access using roles, attributes and relationships.',
    permissions: 'Role-based rules follow job roles; attribute rules use facts; relationship rules follow ownership or membership. Tenant context remains explicit.',
    placement: 'The application checks authorization through its configured PDP and enforces the result.',
    approval: 'Access policy is distinct from a request-specific human approval workflow. Rangoon receives no approval from an allow result.',
    evidence: 'Authorization decision evidence requires separately qualified provider log/API access.',
    privacy: 'PDP deployment, hosted configuration, identities, credentials and retention need explicit qualification.',
    example: 'Illustration: a project member can edit files belonging to that project.',
    limit: 'Permit authorization is its own target; middleware and commercial gateway availability do not follow from it.',
    references: [{ label: 'Roles, attributes and relationships', url: 'https://docs.permit.io/how-to/build-policies/overview/' }, { label: 'Authorization documentation', url: 'https://docs.permit.io/' }],
  },
  {
    id: 'permit-fastmcp',
    purpose: 'Adds permission checks inside a FastMCP server you operate.',
    permissions: 'Maps server methods to resource/action pairs and tool calls to tool-name actions. Tool arguments can become policy attributes.',
    placement: 'Middleware runs inside the owned server and asks the configured Permit PDP.',
    approval: 'Middleware authorization does not itself establish a bounded human review chain.',
    evidence: 'Decision inspection would require a qualified middleware/PDP evidence path; Rangoon has none.',
    privacy: 'Identity and tool arguments may reach the configured PDP; secrets and argument disclosure require review.',
    example: 'Illustration: permit a named tool only when its numeric argument meets an attribute rule.',
    limit: 'Requires correct server identity mapping and role assignment. It is not Permit MCP Gateway.',
    references: [{ label: 'Middleware mapping and argument rules', url: 'https://github.com/permitio/permit-fastmcp/blob/main/README.md' }],
  },
  {
    id: 'permit-mcp-gateway',
    purpose: 'A commercial gateway that checks access before forwarding MCP tool calls.',
    permissions: 'Administrators grant server access and maximum trust levels; user consent bounds the agent level. Tool calls are checked against those levels.',
    placement: 'The gateway verifies the client token, asks Permit and forwards or denies the routed call.',
    approval: 'Session consent is documented. It is not proof of per-action human approval or a Rangoon approval chain.',
    evidence: 'Documented audit entries record agent, tool, server and allow/deny; this is decision evidence, not consequence proof.',
    privacy: 'Hosted sessions, upstream authentication and traffic retention need separate consent and review.',
    example: 'Illustration: an agent consented at medium trust cannot call a high-trust tool.',
    limit: 'Listing a tool does not mean it can be called. Commercial access is separate from FastMCP middleware.',
    references: [{ label: 'Consent, trust levels and audit logs', url: 'https://docs.permit.io/permit-mcp-gateway/guide/' }],
  },
  {
    id: 'invariant-local',
    purpose: 'Finds unwanted patterns in agent messages and tool-call sequences.',
    permissions: 'Guardrail rules inspect behavior; they do not assign identity-based permissions or grant execution authority.',
    placement: 'Local evaluation analyzes supplied traces; a separately configured proxy can intercept covered traffic.',
    approval: 'No reported violation is not approval. A human review path must be designed separately.',
    evidence: 'Violation reports describe rule matches in the supplied trace, not all possible agent behavior.',
    privacy: 'Local evaluation does not guarantee no egress: detector implementations and dependencies must be reviewed and isolated.',
    example: 'Illustration: flag reading an inbox followed by sending its contents to an unknown address.',
    limit: 'Imported policies and detectors are not trusted code. Rangoon does not load or run them.',
    references: [{ label: 'Guardrails and trace rules', url: 'https://github.com/invariantlabs-ai/invariant' }],
  },
  {
    id: 'invariant-remote',
    purpose: 'Evaluates guardrail rules through a separate remote service.',
    permissions: 'Evaluates trace/policy content; a guardrail result does not grant resource access.',
    placement: 'RemotePolicy sends evaluation data to its configured endpoint. A response is an evaluation result, not an execution check.',
    approval: 'Evaluation success is not human approval or Rangoon permission.',
    evidence: 'Returned violations are provider-reported evaluation evidence; authenticity and compatibility need qualification.',
    privacy: 'Policy and trace data leave the machine. A future Rangoon adapter needs endpoint, payload and retention consent and must never switch modes automatically.',
    example: 'Illustration: submit an explicitly selected trace for a remote rule check after consent.',
    limit: 'Remote operation is a distinct target, with no qualified Rangoon endpoint, provider version or adapter.',
    references: [{ label: 'Remote evaluation request source', url: 'https://raw.githubusercontent.com/invariantlabs-ai/invariant/main/invariant/analyzer/remote_policy.py' }],
  },
  {
    id: 'lnsat',
    purpose: 'Targets exact action authorization and evidence of what happened afterward.',
    permissions: 'Source contracts bind action, target, resources, adapter, configuration, identities and expiry to a specific request.',
    placement: 'A qualified execution path must recheck bound authorization and durable attempt state before dispatch.',
    approval: 'Source contracts separate required human approval from narrower execution authorization. UI clicks alone are not trusted approval evidence.',
    evidence: 'October 11 public source distinguishes approval, one-time consumption, attempt, consequence receipt and recovery. Exact-object reads have source support; monitoring snapshot/watch remain proposed. Uncertain outcomes stay unresolved until authorized reconciliation.',
    privacy: 'Identity, request and receipt contents require host/transport/redaction qualification.',
    example: 'Illustration: approval for one exact Git patch must not authorize a changed patch or target.',
    limit: 'LNSAT 0.1.0 source reference, checked October 11 at commit 5cad4719. Experimental bounded Git proof is distinct from unfinished real Docker and supported release qualification. Rangoon A1 integration operations and all catalog operations remain unavailable.',
    references: [{ label: 'Pinned source contracts and release boundaries', url: 'https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/README.md' }, { label: 'Pinned canonical implementation status', url: 'https://github.com/hypler-dev/LNSAT/blob/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/docs/PROJECT_STATUS.md' }],
  },
]);

export const getAuthorityComparison = id => AUTHORITY_COMPARISONS.find(item => item.id === id) ?? null;
