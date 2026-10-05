# Application Git review

Canonical authority: [intent.md](intent.md). Current work is an isolated clone of `hypler-dev/rangoon`, initially at `4dfcb746db076ca59455773527c381ae4359de7e`, branch `codex/app-vision-preview`. It contains application planning/prototype work only. Marketing-site history and source are excluded.

## Review 1: vision, evidence, and interactive preview

Suggested title: `Define Rangoon application plan and dark preview`

Suggested description:

> Rangoon's application repository previously contained only a placeholder README while its feature vision lived across conversations, LNSAT documents, and interface concepts. This change consolidates a source-linked product plan, evidence-qualified claims, government-funding research, and a local dark/light interactive preview with synthetic data.
>
> The initial desktop plan targets macOS, Windows, and Linux together. Production architecture, license, runtime qualification, cloud/enterprise packaging and grant submission remain review decisions. The preview does not import user files, authorize actions, call models, or deploy anything. Validation and independent-review evidence are recorded in `docs/validation.md`.

Review questions:

- Does the first useful workflow—import, trace, refactor, test, compile, export—solve the right problem?
- Is the graph-oriented dark direction preferable to literal reproduction of the lighter concepts?
- Accept the proposed Rust/React/Tauri product architecture and all-three-OS qualification spike, or record a specific alternative before production scaffolding.
- Accept the free-base scope and decide the software license before product publication.
- Confirm which LNSAT runtime gates must precede governed execution and which management features can ship independently.
- Select a government funding program and approve a bounded research experiment/prior-art exercise before drafting a submission.

## Subsequent review slices

Use the R1–R12 rows in [plan.md](plan.md) as bounded candidate PRs. Each starts from the accepted prior decision and includes exact scope, changed files, conformance fixtures, OS support evidence where applicable, independent review and rollback. Avoid a single PR that mixes parser security, desktop packaging, cloud identity, and external connector execution.

LNSAT corrections and core runtime work belong in the LNSAT repository with its own controls. This task did not edit, commit, push, or message that active work. The stale Rust/TypeScript summary gets a docs-only reconciliation packet. Do not copy a proposed worker/reader containment solution into Rangoon as a shortcut.

## External action boundary

Jeff subsequently authorized pushing this application packet to `hypler-dev/rangoon` and continuing development. Publish a development branch and review PR; retain the exact validation evidence. This does not authorize merge, deployment, production mutation, license adoption, or grant submission. Sanitized summaries and user-supplied art are included; raw chat transcripts and operational files are excluded. See [development.md](development.md) for publication receipts and ongoing work.

The remote repository description still says website; the current publication will align it with the existing app-only README. Do not push the website branch to resolve that metadata mismatch.
