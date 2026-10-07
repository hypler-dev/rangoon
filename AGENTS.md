# App review build rule

Every feature packet follows two linked tracks:

1. **Experience:** preserve the original eight-screen structural baseline and shared shell: grouped navigation, compact theme-aware banner, central work area, and right inspector. Route artwork may vary, but layout hierarchy, control meaning, provenance language, and unavailable states stay stable.
2. **Technical:** record the real implementation outcome, evidence, gaps, and tests for the same feature. A visual-only packet is allowed only when explicitly labeled UI-only; it never counts as technical progress. Subsequent technical work remains on the roadmap.

Each packet records both tracks in the canonical development ledger. Preserve native IDs, controls, security disclosures, and provenance. Automated checks enforce narrow contracts; they do not prove visual quality or runtime, security, release, OS, or V1 qualification.

Visual work stays stable per route: no random motion, fake readiness, simulated progress, or artwork that changes control hit targets. Validate rendered 320, 768, and 1440 pixel views in dark and light themes, with no overflow, keyboard focus, and reduced motion. Obtain fresh independent review for meaningful UI or behavior changes.

Scope authority: `docs/intent.md`, `docs/product-experience.md`, and `docs/visual-system.md`. Do not edit source, policy, release, CI, or unrelated documentation while applying this rule unless a packet explicitly names those files.

## README publication gate

Jeff requires a README update with every push or merge to `main`. Include a substantive `README.md` update in every publication packet before publishing it: describe the delivered change, current feature boundaries, and any changed roadmap or setup information. Documentation-only packets should summarize their relevant clarification. Do not add empty edits or promote planned, synthetic, source-tested, or partially qualified behavior into released capability. Keep exact validation and publication receipts in `docs/development.md`; link that evidence from the README. The controller must verify the README update is present in the reviewed publication diff and matches the final scope. A previous packet's README update does not satisfy a later publication.
