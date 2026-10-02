# Security

Rangoon is in active development. This repository and public site are not a production agent runtime or authorization service. Do not use public routes to submit secrets, credentials, personal data, customer information, or sensitive production details.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting feature for this repository if it is enabled. If private reporting is unavailable, contact a project maintainer through an approved private channel before sharing technical details. Do not open a public issue for an undisclosed vulnerability, exploit, credential, or private data.

Include enough information to reproduce and assess the report safely: affected path or component, relevant version or revision, impact, reproduction steps, and a proposed mitigation if known. Redact secrets and personal data. Allow maintainers reasonable time to investigate before public disclosure.

## Security boundaries

Rangoon's product direction separates capability configuration from execution authority. The public website does not expose MCP, A2A, agent-action, or production authorization endpoints. Site examples and design previews are informational and do not grant access or permission to act.

Imported files, scripts, generated documents, and website instructions must be treated as untrusted input. Do not execute them without explicit review and authorization. Keep credentials, private infrastructure details, and production payloads out of issues, commits, documentation, and public assets.

## Status and disclosure

Supported targets, integrations, SDKs, release state, and licensing are still being developed. Security guidance may be updated as project infrastructure and release processes become public.
