# Validation and independent review

Date: October 4, 2026 (America/Los_Angeles). Authority: [intent.md](intent.md). This is the historical R0 research and prototype validation receipt. Current publication and follow-on work are recorded in [development.md](development.md). Architecture, license, implementation roadmap, actual platform support, publication and grant submission remain proposed; no release acceptance is implied.

## R0 completion snapshot, before the later push instruction

- Application origin: `https://github.com/hypler-dev/rangoon.git`.
- Application checkout: `/Users/jeff/hypler/code/rangoon/app-review`.
- Branch: `codex/app-vision-preview`; base/HEAD remains `4dfcb746db076ca59455773527c381ae4359de7e`.
- Changes: README, package metadata, `preview/`, `scripts/`, `tests/`, and `docs/`. The [file inventory](changed-files.txt) enumerates exact packet paths. Original art and the historical brief are copied unchanged; derived art and browser captures have separate provenance.
- Commit: no. Push: no. PR: no. Merge: no. Deployment: no. Grant registration/contact/submission: no. LNSAT source changes: no.
- Outer marketing checkout remains `website-local` at `0ea1a2d9b6bc904867a62965a80487aa0d46b52d`, with no tracked changes. Its pre-existing untracked `.codex/` is untouched; the new nested `app-review/` is also untracked by that outer checkout.
- A worker initially wrote server/package/test changes into the outer website checkout. The controller restored only those task-owned tracked changes to their original HEAD bytes and moved the new test into the app checkout. The final outer tracked diff is empty. This correction is not counted as website work.

## Commands and results

Executed in the application checkout using Node `v24.21.0`:

| Check | Result | Scope |
| --- | --- | --- |
| `npm run check` | PASS | Syntax of app, model and static server |
| `npm test` | PASS, 4 tests, 0 failures | Inert compilation rejects conflict/loss; unknown has no retry; filter edge cases; static server confinement |
| `node scripts/validate-review.mjs` | PASS | Eight original image hashes, historical brief hash, local Markdown link targets and authored-text whitespace |
| `python3 /Users/jeff/.agents/skills/intent-driven-delivery/scripts/validate_artifact.py docs/intent.md` | PASS | Intent schema |
| Same validator with `docs/plan.md` | PASS | Plan schema |
| `git diff --check` | PASS | Tracked patch; the separate review validator covers new authored files |
| Outer `git diff --stat` / `git status --short --branch` | PASS isolation | No marketing tracked diff; only `.codex/` and `app-review/` untracked |
| Fixed-commit `git show eebfa485a9b4c45199da4a3243c36f2cded9d20d:<path>` | PASS citation recheck | Supplemental LNSAT paths/lines in claims register; no tests executed |

The server test binds a random loopback port and uses a disposable directory. It verifies root/asset response, content type, CSP, GET/HEAD behavior, and rejection of traversal, a symlink escape, hidden files, directories, missing assets, a NUL path and POST. It does not test a production server.

An initial preview-server start received sandbox `listen EPERM`. A loopback-only run was then approved and succeeded; no public interface or tunnel was opened. The local user preview is at `http://127.0.0.1:4377`; `npm start` restarts it when needed.

## Rendered QA and accessibility checks

Performed by the controller through the Codex in-app browser, with actual DOM/accessibility inspection and screenshots. The desktop override requested 1600×1050; the browser's existing zoom reported 1454 CSS pixels wide. Responsive checks used measured CSS widths 1024, 768, 390 and 320, across all nine routes. They are browser layouts, not native platform qualification.

Verified:

- All nine sidebar routes render distinct content; theme toggle renders dark and light versions.
- Sample import produces an inert inventory; source candidate selection changes highlighted lines; selected drafts produce an explicit sample result.
- Merge compilation is disabled before resolving the conflict. Retaining the restrictive boundary enables an inert draft; reopening invalidates it. Split has a distinct draft set. History does not offer a misleading compile action.
- Skill filtering retains text input and selection; connector selection shows the chosen operation, access and authority requirements.
- Workflow unknown-outcome simulation shows no external actions and disabled automatic retry; the displayed gate status matches the selected result.
- Evidence filtering selects the matching entry; initial release plan includes macOS, Windows and Linux with missing-evidence labels.
- Empty workspace has a route-specific heading and recovery action. Partial/blocked/unknown scenarios disclose their limits.
- `Cmd+K`, palette filtering and Enter navigation work. The native dialog supplies modal semantics. Route changes and brand/hash navigation focus a heading; browser Back also retained heading focus; source review verified the shared Back/Forward handler. Escape exits focus mode. Compact mode changes spacing. Graph has a selectable list alternative.
- An initial narrow-grid overflow was fixed. At the final 320-CSS-pixel check, root `scrollWidth` and `clientWidth` both measured 305 (the vertical scrollbar accounts for the difference from `innerWidth`). All nine routes passed this final check; [browser-checks.json](browser-checks.json) preserves measured dimensions. No viewport overflow remained. Navigation and data tables may scroll within their own regions.
- Browser error/warning log query returned an empty list after the interaction checks.

The [gallery](preview-gallery.md) contains nine dark screens, light captures and a narrow capture. Captures are synthetic view evidence, not real operations or product data. Reduced-motion and forced-color styles were source-inspected, not OS-mode tested. No screen-reader certification, automated accessibility audit, complete contrast audit, or Windows/Linux native interaction check was performed.

## Performance scope

This local prototype has no frontend dependencies, external fonts, analytics or network APIs. The server CSP disallows connections, and the browser module performs no network request itself. Source-resolution art is deliberately retained: the panorama is about 1.8 MB and the brand asset about 984 KB. Optimize derived display assets before a production release. No Lighthouse/Core Web Vitals, native startup, memory, parser or compiler benchmark is claimed.

## Independent review and corrections

| Lane | Provider / model | Result |
| --- | --- | --- |
| History and exact-file evidence reader | OpenAI `gpt_reader`, GPT-5.6-Luna medium | Focused source/conversation extraction with explicit retrieval gaps |
| LNSAT readiness reviewer | OpenAI `gpt_reviewer`, GPT-5.6-Terra xhigh | Targeted source/test/status review; native-reader and real-Docker proof gaps retained |
| UI design | OpenAI `gpt_ui_design`, GPT-5.6-Terra high | Graph workbench direction and state recommendations |
| Initial preview producer | OpenAI `gpt_worker`, GPT-5.6-Terra high | Initial implementation; controller substantially rewrote UI and integrated fixes |
| Architecture/docs reviewer | OpenAI `gpt_reviewer`, GPT-5.6-Terra xhigh, fresh | Two findings: missing validation ledger (P1), moving supplemental citations (P2); both corrected; independent correction review PASS |
| UI source reviewer | OpenAI `gpt_ui_review`, GPT-5.6-Terra xhigh, fresh | Two P2 focus findings; both corrected and independent correction review PASS; no remaining correction-scope finding |
| Integration, architecture, browser QA and final judgment | Configured primary OpenAI controller | Owns final changes and evidence; no claim of independent self-review |

Other-provider UI review was attempted through the configured no-tool Z.AI runner. The first local preflight rejected the full app source with `glm-secure: snapshot too large`; no provider request was sent for that attempt. A smaller secret-free synthetic behavior excerpt plus CSS was submitted. The configured runner recorded `status=retryable http=000` for `glm-5.3` active attempt 1 and its one retry, then for the single `glm-5.3-flash` fallback; final exit 75. No model response was returned, so no returned-model identity or GLM review result can be reported. No further retries were made. Adequate fresh native review was available; Spark and retired local model lanes were not used. Payload contained only new synthetic UI text/CSS, not conversations, private LNSAT source, credentials, customer data or operational logs.

## Practical limits and next step

The preview has fixed synthetic examples, no durable workspaces, no real folder import, no production compiler, no engine integration and no installers. Several file rows deliberately explain that their viewer is not connected. Historical retrieval is substantial but not exhaustive. The LNSAT audit was targeted and source-based, not a full security review or runtime proof. Government program fit is a proposal, not verified applicant eligibility.

Next review is the local packet in [review-submission.md](review-submission.md). After the owner accepts the relevant decisions, the next production slice is R1: license/contracts and the three-OS shell qualification spike. The [continuation prompt](continue.md) preserves the repository and evidence boundaries.
