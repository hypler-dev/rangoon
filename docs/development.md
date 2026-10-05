# Development status

Authority: [intent.md](intent.md). Jeff authorized publishing the application work to `hypler-dev/rangoon` and continuing development on October 4, 2026. The repository is an application in development. Preview completeness does not imply product or runtime qualification.

## Publication

The reviewed R0 packet is being committed to `codex/app-vision-preview` and pushed for review. The publication receipt will record exact commits and the PR after verification. Main remains protected from unrequested merge. The marketing-site repository stays separate.

R0 validation: [validation.md](validation.md). Final root-owned changes were independently reviewed by OpenAI GPT-5.6-Terra; review corrections passed. A publication check inspects the exact outgoing packet for accidental private payloads.

## Active implementation slice: R1a

Build a platform-neutral Rust domain and source-analysis foundation with an explicit-file CLI. Accept bounded UTF-8 instruction files only; preserve original byte hashes and exact line spans; detect Markdown sections deterministically; represent extracted sections as unreviewed proposals, never executable grants. The caller chooses every file. No recursive scan, archives, network, models, hooks, scripts, execution authority, persistence or installation.

This is a bounded early implementation of R1 contracts and the pure-analysis prerequisite for R2/R3, not completion of those packets. Establish three-OS source-test CI without claiming all three platforms have been qualified before their actual CI results exist. Tauri/React shell qualification, license selection and real installers remain R1/R6 follow-ups.

Controller owns architecture, contracts, integration, validation and git state. A bounded native worker may own the analyzer implementation and its exact tests after a contract handoff. A fresh independent reviewer checks the resulting diff. Required evidence: Rust formatting, unit/integration tests, Clippy where available, existing preview checks, review-validator checks, explicit unsupported/hostile input cases and exact remote push verification.

## External action boundaries

Commit and push of reviewed app work, review PR creation, and correcting the repository description to its app purpose are within the current publication request. Merge, deployment, production data changes, LNSAT mutations, license adoption, paid infrastructure and government application submission remain closed.
