# App review build rule

Every feature packet follows two linked tracks:

1. **Experience:** preserve the original eight-screen structural baseline and shared shell: grouped navigation, compact theme-aware banner, central work area, and right inspector. Route artwork may vary, but layout hierarchy, control meaning, provenance language, and unavailable states stay stable.
2. **Technical:** record the real implementation outcome, evidence, gaps, and tests for the same feature. A visual-only packet is allowed only when explicitly labeled UI-only; it never counts as technical progress. Subsequent technical work remains on the roadmap.

Each packet records both tracks in the canonical development ledger. Preserve native IDs, controls, security disclosures, and provenance. Automated checks enforce narrow contracts; they do not prove visual quality or runtime, security, release, OS, or V1 qualification.

Visual work stays stable per route: no random motion, fake readiness, simulated progress, or artwork that changes control hit targets. Validate rendered 320, 768, and 1440 pixel views in dark and light themes, with no overflow, keyboard focus, and reduced motion. Obtain fresh independent review for meaningful UI or behavior changes.

Scope authority: `docs/intent.md`, `docs/product-experience.md`, and `docs/visual-system.md`. Do not edit source, policy, release, CI, or unrelated documentation while applying this rule unless a packet explicitly names those files.
