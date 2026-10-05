# Rangoon

The capability workbench for governed AI, under development.

This repository contains an application planning and design preview packet. It does not yet provide a released desktop application, working connector catalog, or qualified execution runtime. The initial product targets **macOS, Windows, and Linux**, with a free open-source release planned. The software license is still a review decision.

## Explore the preview

Requires Node.js 20 or later. No dependency installation is needed.

```sh
npm start
```

Open <http://127.0.0.1:4377>. Use the sidebar, theme button, compact/focus controls, and `Ctrl/Cmd+K`. Import the synthetic Atlas example, inspect source candidates, resolve the merge conflict, preview an inert draft, or simulate an unknown workflow outcome. All nine screens use sample data. Only the theme preference persists; reload resets the workspace. There is no account, provider call, actual file import, or external execution.

## Review the product direction

- [Accepted research/preview intent](docs/intent.md)
- [Architecture, functionality, three-OS launch, and build sequence](docs/plan.md)
- [Recovered features and source register](docs/sources-and-features.md)
- [LNSAT claims and evidence audit](docs/claims-and-evidence.md)
- [Government funding research plan](docs/government-funding.md)
- [Review submission proposal](docs/review-submission.md)
- [Validation, independent review, and limitations](docs/validation.md)
- [Preview gallery: nine dark screens and light/narrow variants](docs/preview-gallery.md)
- [Visual provenance](docs/artwork.md)

```sh
npm run check
npm test
```

These checks cover the local prototype and static server, not LNSAT runtime behavior or desktop qualification.

The public website is [rangoon.ai](https://rangoon.ai). Its marketing implementation is maintained separately. LNSAT remains the independent reference authority and evidence engine; Rangoon owns the user-facing capability lifecycle and references qualified engine contracts.
