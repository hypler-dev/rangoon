# VUSL architecture — upstream reference

The canonical specification is maintained in the separate [VUSL repository](https://github.com/hypler-dev/vusl). Read the [pinned vusl architecture](https://github.com/hypler-dev/vusl/blob/b55e01955e9c59cd6ab652afe9a54c43589b09b8/docs/architecture.md). This file is a compatibility pointer, not a second normative copy.

Rangoon consumes the experimental D0a subset through its existing offline `rangoon-language` parser. Existing `rangoon.rusl.*` identifiers and digest domain remain exact; the VUSL name does not add a supported wire alias. Parser acceptance does not establish provider support, authorization or verified effects. LNSAT and OpenShell remain independent optional systems.

[The consumer manifest](vusl-upstream.json) pins the upstream revision and hashes for Rangoon's offline fixture copies. Updates require explicit version review and conformance replay. Rangoon's implementation, integration status and release gates remain in this repository.

<!-- Preserve historical deep links; follow the pinned upstream guide above. -->
<a id="acceptance-mapping"></a>
<a id="behavior"></a>
<a id="compatibility-and-migration"></a>
<a id="content-transport-and-event-semantics"></a>
<a id="data-privacy-and-permissions"></a>
<a id="envelope-and-provenance"></a>
<a id="extensions-and-version-negotiation"></a>
<a id="future-adaptersdk-boundary"></a>
<a id="inert-examples"></a>
<a id="interfaces-and-contracts"></a>
<a id="invariants"></a>
<a id="minimal-conceptual-entities"></a>
<a id="non-goals-and-open-questions"></a>
<a id="operation-stages-and-authority"></a>
<a id="semantic-families"></a>
<a id="specification-vusl-draft-v01"></a>
<a id="states-and-failure-handling"></a>
<a id="translation-and-support-labels"></a>
<a id="specification-rangoon-universal-ai-systems-language-rusl-draft-v01"></a>
