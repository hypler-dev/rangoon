# Contributing to Rangoon

Rangoon is in active development. Interfaces, supported harnesses, extension contracts, package boundaries, and licensing may evolve before stable release. Read the repository `README.md`, `SECURITY.md`, `intent.md`, and any current `AGENTS.md` before making changes.

## Before you start

Use a focused issue or task description. State the intended behavior, affected surface, constraints, validation, and any unresolved assumptions. Keep product direction, source-of-truth claims, licensing, security policy, and release status grounded in repository evidence.

Do not add credentials, private infrastructure details, customer data, production payloads, or invented availability, compatibility, certification, repository, or license claims.

## Local checks

Use Node.js 20 or newer. From the repository root, run the checks relevant to your change:

```sh
npm run build
npm run check
npm test
```

Run `npm start` when browser inspection helps. Describe commands and results in the change record. Keep generated output consistent with repository instructions and review the complete diff before sharing it.

## Changes and review

Prefer small, reviewable changes with one clear purpose. Preserve existing work from other contributors. Update documentation when behavior, route contracts, or project status changes. Public claims need direct source evidence; proposed or illustrative API examples are not callable contracts.

Security-sensitive changes, authentication, payments, databases, secrets, production data, infrastructure, deployment, and release decisions need explicit maintainer review. A passing check does not by itself authorize merge, deployment, or release.

## Licensing

Final project licensing and edition boundaries are pending stable publication. Contributions remain subject to the terms provided by the project when published; do not add a license header or make legal-rights claims without maintainer direction.
