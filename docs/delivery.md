# Rangoon website delivery

Canonical authority: [intent.md](../intent.md). Implementation and deployment are authorized by the owner's direct requests. Final visual acceptance remains with the owner.

## Implemented

- 32 substantive static pages with accessible mega menus, mobile navigation, local search, screenshot tabs and expansion dialogs, connector filtering, and reduced-motion support.
- Restrained ivory, sage, and copper styling; outlined actions with highlight and glow effects; layered application previews in the hero.
- Isolated mascot and supplied symbol, two additional generated mascot scenes, optimized WebP artwork, self-hosted fonts, and original screenshot downloads.
- Downloads and systems page with actual website source and explicitly planned LNSAT desktop, server, and mobile directions. Application installers are not available.
- Markdown companions for every route, sitemap, search index, llms.txt, llms-full.txt, ai.txt, agents.txt, and project-specific JSON policy metadata.
- A dependency-free static server with response headers, true 404 handling, method restrictions, path containment, and symlinked release support.

## Validation and review

- `npm run build`: PASS, 32 pages.
- `npm run check`: PASS, 3,226 local references, page metadata, Markdown companions, sitemap, and AI resources.
- `npm test`: PASS, five HTTP and release-symlink regression tests.
- The sandboxed Node test subprocess hit a native assertion. The same tests passed in the normal execution environment; no test or assertion was bypassed.
- Browser checks: desktop and mobile layout, no horizontal page overflow, hero screenshot dialog and Escape dismissal. Earlier interaction checks covered navigation, keyboard tabs, search, filters, and reduced motion.
- UI production/integration: controller plus OpenAI GPT Terra-high content worker; exact documentation by OpenAI GPT Luna-medium.
- Independent read-only review: OpenAI GPT Terra-xhigh. No unresolved source or UI findings. Source ZIP availability must be verified after the initial push and before public launch.
- Other-provider review attempt: requested GLM-5.3; no model response. Runner rejected first oversized snapshot, then smaller snapshot because the model registry timestamp was stale/future. Native review used; registry restrictions were preserved.

## Release state

Source is prepared for initial publication at `https://github.com/hypler-dev/rangoon`. Public launch follows successful source archive verification and origin smoke tests. Final deployment evidence will be recorded after verification.

## Boundaries

No LNSAT source changes, application runtime release, package publication, customer data, account collection, analytics tracking, mail changes, or unrelated site changes. Product screenshots remain design previews with illustrative data. Final product licensing remains an owner decision.

## Continue

Start with `git status --short --branch`, `git log -1 --oneline`, then read `intent.md` and this file. Build with `npm run build`, validate with `npm run check && npm test`. Keep product availability truthful and credentials out of public source. Deployment uses the existing dedicated Rangoon service; unrelated infrastructure is outside scope.
