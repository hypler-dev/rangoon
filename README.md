# Rangoon.ai

Rangoon is a design-preview control plane for composing, governing, testing, and preparing portable AI agent capabilities across changing harnesses. It is intended to help teams manage agents, skills, instructions, workflows, tools, connectors, policies, tests, versions, deployment targets, and evidence in one inspectable model.

The project is in active development. Product interfaces, supported harnesses, extension contracts, package boundaries, and licensing may change before the first stable release. Screenshots and metrics on the site are design previews with sample data. They are not customer or production evidence.

Rangoon is designed around LNSAT as its execution authorization and evidence foundation. The site itself is a static public website. It does not expose a callable agent runtime, MCP server, A2A endpoint, or production authorization API.

## Repository

The canonical public source location is:

```text
https://github.com/hypler-dev/rangoon
```

Final licensing and edition boundaries are also pending stable project publication. Do not infer a license from public availability or site documentation.

## Local development

The site uses zero-dependency Node.js tooling and requires Node.js 20 or newer. From a source checkout:

```sh
git clone https://github.com/hypler-dev/rangoon.git
cd rangoon
node --version
npm run build
npm run check
npm test
npm start
```

`npm run build` generates the static site. `npm run check` checks generated routes, links, assets, and content invariants. `npm test` runs the repository test suite. `npm start` serves the local output for inspection.

## Downloads

The public [`/downloads/`](https://rangoon.ai/downloads/) page describes candidate desktop and mobile product directions. It is product planning content; no downloadable desktop or mobile application release is claimed.

## Documentation

- Public machine-readable overview: https://rangoon.ai/llms.txt
- Extended overview: https://rangoon.ai/llms-full.txt
- AI-oriented page index: https://rangoon.ai/ai/
- Downloads and product directions: https://rangoon.ai/downloads/
- Contribution guidance: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security guidance: [SECURITY.md](SECURITY.md)

## Project direction

Rangoon is intended to remain open and inspectable around capability management, portability, testing, simulation, and governance. Exact supported targets, adapters, SDKs, integrations, and release commitments will be documented when they are ready. No current page should be read as a claim that a planned integration is shipped or certified.
