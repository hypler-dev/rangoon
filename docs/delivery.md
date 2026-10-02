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
- Independent read-only review: OpenAI GPT Terra-xhigh. No unresolved source or UI findings. The source ZIP gate passed after the initial push and before public launch: the public archive was downloaded and its contents inspected.
- Other-provider review attempt: requested GLM-5.3; no model response. Runner rejected first oversized snapshot, then smaller snapshot because the model registry timestamp was stale/future. Native review used; registry restrictions were preserved.

## Release state

- Runtime/source commit: `cda2dbc`, published to `hypler-dev/rangoon` on `main`.
- Public source archive downloaded and inspected: 59 ZIP entries, including the downloads page and static server.
- Exact source built on home-mac: 32 pages, 3,226 references, five server tests passed.
- Dedicated loopback origin and Cloudflare Tunnel are running. Existing website parking records were replaced with Rangoon's tunnel route. The existing www alias reaches the same site; unrelated DNS records were preserved.
- Public HTTPS checks: **80 PASS** covering all 32 HTML pages, all 32 Markdown companions, AI metadata, assets, real 404 behavior, and www.
- Live browser: refined homepage renders, developer mega menu includes downloads and AI access, no console errors. Desktop and mobile layouts reviewed.
- Cloudflare's existing browser-integrity rules reject the default Python urllib user agent (1010). Browser and curl checks pass. Security controls were not weakened; crawler access remains subject to Cloudflare policy.
- A conflicting DNS submission was stopped by automatic approval review. The second obsolete parking A record was removed, the single remaining apex was reverified, and the corrected CNAME save succeeded. Prior record values and origin release remain available for rollback.
- Local Node repaired using an official checksummed Node 24 LTS distribution in the user's local tools directory. Login-shell node/npm resolve successfully. Homebrew reinstall was unavailable because Command Line Tools were missing; no broad system upgrade was performed.
- Technical acceptance: PASS. Owner visual acceptance: pending feedback. No outstanding deployment blocker.

## Boundaries

No LNSAT source changes, application runtime release, package publication, customer data, account collection, analytics tracking, mail changes, or unrelated site changes. Product screenshots remain design previews with illustrative data. Final product licensing remains an owner decision.

## Continue

Start with `git status --short --branch`, `git log -1 --oneline`, then read `intent.md` and this file. Build with `npm run build`, validate with `npm run check && npm test`. Keep product availability truthful and credentials out of public source. Deployment uses the existing dedicated Rangoon service; unrelated infrastructure is outside scope.

## Exact initial change set

```text
.gitignore
CONTRIBUTING.md
README.md
SECURITY.md
docs/delivery.md
docs/source-copy.txt
intent.md
package.json
public/.well-known/ai-policy.json
public/agents.txt
public/ai.txt
public/app.js
public/assets/Manrope-OFL.txt
public/assets/Manrope-latin.woff2
public/assets/brand-symbol.png
public/assets/brand-symbol.webp
public/assets/command-center.png
public/assets/command-center.webp
public/assets/connectors.png
public/assets/connectors.webp
public/assets/decompose.png
public/assets/decompose.webp
public/assets/favicon.png
public/assets/governed-command-center.png
public/assets/governed-command-center.webp
public/assets/import-analyze.png
public/assets/import-analyze.webp
public/assets/merge-split.png
public/assets/merge-split.webp
public/assets/rangoon-mascot.png
public/assets/rangoon-mascot.webp
public/assets/rangoon-mobile.png
public/assets/rangoon-mobile.webp
public/assets/rangoon-studio.png
public/assets/rangoon-studio.webp
public/assets/skills.png
public/assets/skills.webp
public/assets/social-card.png
public/assets/workflows.png
public/assets/workflows.webp
public/robots.txt
public/styles.css
scripts/build.mjs
scripts/check.mjs
scripts/serve.mjs
src/downloads.mjs
src/extra.mjs
src/icons.mjs
src/pages.mjs
src/site.mjs
tests/server.test.mjs
```
