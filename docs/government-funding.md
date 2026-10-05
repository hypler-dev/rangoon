# Government funding preparation

Prepared October 4, 2026. Authority: [intent.md](intent.md). Jeff named government grants but did not identify a program or deadline. This document prepares a research case and a U.S. federal candidate-program shortlist as a working assumption; applicant location and eligibility are unverified. No application, registration, agency contact, eligibility certification, budget commitment, or submission has been made.

## Best research case

Frame the research around **portable, verifiable constraints for agent capabilities and consequential actions**. Rangoon supplies a practical workbench and evaluation surface; LNSAT supplies a separate reference authority/evidence foundation. Keep their code, responsibilities, expenditures, and research deliverables distinct so the same work is not charged twice or described as two independent inventions.

The technical question is whether a canonical capability representation can preserve source provenance and safety-relevant meaning across heterogeneous harnesses, and whether the resulting actions remain bound to exact authorization and reconstructable outcomes through failures. This is a hypothesis requiring prior-art review, controlled experiments, and reproducible results. A dashboard, common file parser, protocol wrapper, or ordinary installer is not automatically research novelty.

## Candidate programs, checked against official sources

### NSF SBIR/STTR — first candidate to investigate

[NSF 26-510](https://www.nsf.gov/funding/opportunities/small-business-innovation-research-small-business-technology/nsf26-510/solicitation) lists November 4, 2026, March 4, 2027, and July 7, 2027 proposal dates. Phase I is listed up to $305,000 over 6–18 months. These are program limits and listed dates, not a Rangoon award estimate or a confirmed feasible filing date. Phase I requires an invited Project Pitch path. Applicant eligibility, ownership, PI commitments, registrations, and current submission conditions still need verification against the exact solicitation.

NSF's [Project Pitch process](https://seedfund.nsf.gov/apply/project-pitch/) emphasizes technical innovation, high-risk R&D, market need, and a capable team, and excludes straightforward engineering or incremental product improvements. Its listed pitch sections are technology innovation and technical objectives (3,500 characters each), market opportunity and company/team (1,750 each). A listed full-proposal deadline is not permission to bypass the pitch. With invitation status unknown, do not build a schedule assuming November 4 is reachable.

Fit is an inference: the research hypotheses above could support a pitch if prior-art analysis and experiments demonstrate a real technical advance. Cross-platform packaging alone is a weak research case. Do not claim eligibility or guaranteed program fit.

### NSF PESOSE — possible open ecosystem route

[NSF 26-506 PESOSE](https://www.nsf.gov/funding/opportunities/pesose-pathways-enable-secure-open-source-ecosystems/nsf26-506/solicitation) targets secure, sustainable open-source ecosystems. The next listed deadline after October 4 is March 2, 2027; September 1, 2026 has passed. Track 1 lists up to $300,000 for planning over one year; Tracks 2/3 list up to $1.5 million over two years. U.S. for-profit organizations can be eligible subject to the solicitation's conditions; eligibility is not established here.

Potential fit is LNSAT/community interoperability, governance, secure maintenance, and ecosystem adoption, rather than ordinary Rangoon product development. The program asks for an existing public open-source product and evidence of development, users/contributors and community need; Track 1 is planning rather than a product-build grant. Prepare the required external collaboration evidence, including the specified 3–5 letters, only from real willing users/contributors. A source repository without adoption proof is not enough to assert readiness for a large ecosystem award.

### DHS and defense programs — watch exact topics

[DHS SVIP](https://www.dhs.gov/science-and-technology/svip) is a mission-oriented prototyping route using Other Transaction arrangements; it should not be casually labeled a grant. No applicable currently open topic was verified in this scan. A future candidate would need an exact homeland-security problem and transition partner.

[DoD SBIR/STTR BAA information](https://business.defense.gov/Programs/SBIR-STTR/BAA-Schedule/) provides topic-driven intake. No matching active topic/window was verified here. Do not send a generic “AI control plane” pitch without matching the component's objectives, eligibility and transition requirements. Funding vehicle, rights, security restrictions and proposal rules depend on that exact opportunity.

Recheck all official pages and the actual solicitation immediately before preparing a submission. Search snippets, old dates and program homepages cannot establish an open application window.

## Draft research work packages

The targets below are proposed experiment acceptance criteria, not current achievements. Final quantities and costs must be adjusted to the selected solicitation and approved scope.

| Work package | Hypothesis / uncertainty | Experiment and baseline | Evidence to submit |
| --- | --- | --- | --- |
| G1: capability semantics | A canonical representation can expose safety-critical translation loss across three harnesses | Curated public/synthetic corpus with instruction scope, inheritance, tools, context and hooks; compare manual transfer and naive text conversion with a constraint-aware compiler | Versioned corpus, labeling guide, independent adjudication, transform report, silent-loss and unsupported-detection measurements |
| G2: action integrity | Exact binding and durable reconciliation can prevent substituted/duplicate effects under bounded failure models | Isolated disposable Git connector; replay, changed digest, expiry, approval change, dropped response, process crash before/after effect; compare prompt-only and ordinary retry baselines | Threat model, trace/receipt chain, failure-injection scripts, no-blind-retry results, residual bypass conditions |
| G3: operator utility | Provenance and evidence views reduce unsafe-change review mistakes | Real developers perform predefined import/refactor/export and incident-reconstruction tasks with current tools versus prototype | Consent/privacy-reviewed study, task completion time, errors, usability observations, negative results and raw-data handling plan |
| G4: interoperable public artifact | Other developers can implement an adapter without copying the core | Independent reference adapter built against public schema/fixtures; compatibility and adversarial corpus | Open schema, SDK docs, conformance runner, external feedback, reproducibility and sustainability plan |

Separate routine delivery costs—desktop shell, settings, installer polish, ordinary UI implementation—from high-risk technical investigation. Some engineering can support a research experiment, but it must be justified by that experiment rather than relabeled as novel research. Avoid arbitrary TRL or completion percentages; describe concrete evidence maturity.

## Submission preparation checklist

1. **Eligibility and program fit:** legal applicant/entity, U.S. ownership/control and work location, size, PI identity/employment/time commitment, current registrations, prior awards or overlapping applications, exact solicitation and topic. Facts are currently unknown; do not invent them from founder copy.
2. **Technical case:** concise problem, known prior art, differentiated hypothesis, risks, failure criteria, experiment design, reproducible artifacts and current source snapshot. Explain what may fail and why funding would reduce that uncertainty.
3. **Customer evidence:** interviews with platform teams and real integrators, a specific first use case, baseline cost/pain, alternatives, willingness to pilot/pay, letters where required. No invented testimonials or agency endorsement.
4. **Commercialization:** free OSS base and sustainable revenue from hosted operations, support and organizational needs; distribution/adoption strategy, partner model, competitor/prior-art map, intellectual-property and contributor policy. Do not equate a permissive license with lack of defensibility; justify the business rather than making unsupported claims.
5. **Team and execution:** actual contributors, responsibilities, relevant experience, missing expertise and hiring/partner plan. The number of AI agents used for this preview is not a research team credential.
6. **Budget:** people/time at justified rates, test devices for three OSs, independent evaluation, infrastructure, dissemination, indirect costs and allowed subawards. No dollar request is selected in this packet. Keep Rangoon and LNSAT deliverables/budgets traceable.
7. **Assurance and management:** security and data-management plans, dependency/license inventory, accessibility evaluation, conflict-of-interest/other support and any required human-subjects process. Requirements come from the chosen program, not a generic badge list.
8. **Final gate:** validate forms, character/page limits, dates, registrations, invitation and evidence; have a human owner approve the exact application and submission. Do not infer submit authority from review of this plan.

## Short working pitch

Rangoon is developing an open capability workbench for heterogeneous AI agent systems. The proposed research investigates how to retain provenance and safety-relevant constraints when instructions, skills, and workflows move between harnesses, and how to connect those capabilities to exact action authorization and recoverable evidence. LNSAT provides an independent reference authority foundation. The project will test semantic loss and failure recovery against defined baselines and publish synthetic corpora, compatibility reports, and conformance artifacts. Commercial delivery is intended to combine a free desktop/self-hosted base with paid enterprise operations and support. Novelty, market demand, eligibility, and technical performance remain to be demonstrated.

This is a drafting aid, not an application or an assertion that the hypothesis is unprecedented. The best next grant step is selecting a program and producing real prior-art and customer evidence, alongside the first reproducible technical experiment.
