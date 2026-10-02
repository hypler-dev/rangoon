<!-- intent-driven-delivery:intent:v1 -->
# Intent: Rangoon public website

Status: authorized for implementation and deployment; final visual acceptance pending
Authority: This record, based on Jeff's October 1, 2026 requests in this chat
Owner: Jeff
Accepted by: Jeff's direct request to build the site and deploy on home-mac under rangoon.ai
Last updated: 2026-10-01

## Problem and evidence

The workspace is empty and is not a Git repository. Jeff supplied brand art, eight application design screenshots, and detailed product copy in `docs/source-copy.txt`.

## Desired outcome

A polished, complete, responsive public website centered on open source, with substantial product, developer, solution, and resource pages; accessible mega menus; purposeful animation and transitions; isolated mascot and logo; and intelligently presented supplied screenshots.

## Users and systems

Developers, AI platform teams, security teams, and prospective contributors. The public site is deployed on home-mac and exposed at rangoon.ai through Cloudflare Tunnel.

## Constraints

- Rangoon leads the brand. LNSAT is explained as its execution authorization and evidence foundation, with room for additional policy engines and custom adapters.
- Product screenshots are design previews; their metrics are sample data. Features, licensing, SDKs, and adapter contracts are in active development.
- Do not invent availability, customer proof, GitHub counts, repository URLs, install commands, compatibility certifications, or licenses.
- Preserve screenshot pixels; crop and style them through presentation. Use image generation for the requested mascot cutout.
- Use a restrained ivory, sage, and copper palette with outlined controls, soft highlights/glows, layered application hero previews, and additional mascot scenes.
- Add a downloads and systems page plus a homepage entry; distinguish website source from candidate desktop/server distribution and source-only mobile plans.
- No changes to unrelated sites, databases, mail, or fleet routing. Only Rangoon's public route and dedicated origin are authorized.
- Keep secrets and private infrastructure out of public assets.

## Non-goals

Implementing the governed agent application, collecting accounts, processing payments, publishing packages, or changing LNSAT code.

## Assumptions and verified facts

- Verified: home-mac SSH access works; rangoon.ai delegates to Cloudflare and responds over HTTPS.
- Verified: supplied copy explicitly marks the product in active development and final licensing pending.
- Assumption: a statically generated site with small browser JavaScript is appropriate for this content-led website.

## Risks

Marketing may overstate roadmap functionality; previews and integration status must be clear. Deployment may affect existing routing; inspect first, use an isolated service and retained rollback, and smoke-test the public URL.

## Acceptance evidence

- Build and route/link/asset checks pass.
- Desktop and mobile screenshots reviewed; navigation, tabs, filters, preview dialog, theme, and reduced-motion behavior checked.
- Fresh independent UI/code review resolved before deployment.
- Origin and public HTTPS smoke tests pass for all generated routes, assets, and error handling.
- Exact implementation, validation, and deployment evidence recorded in `docs/delivery.md`.

## Source-of-truth links

This file is the canonical work record. Supporting product copy: `docs/source-copy.txt`. Evidence and operations: `docs/delivery.md`.
