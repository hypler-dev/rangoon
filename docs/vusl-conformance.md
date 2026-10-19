# VUSL conformance — upstream reference

The canonical specification is maintained in the separate [VUSL repository](https://github.com/hypler-dev/vusl). Read the [pinned vusl conformance](https://github.com/hypler-dev/vusl/blob/b55e01955e9c59cd6ab652afe9a54c43589b09b8/docs/conformance.md). This file is a compatibility pointer, not a second normative copy.

Rangoon consumes the experimental D0a subset through its existing offline `rangoon-language` parser. Existing `rangoon.rusl.*` identifiers and digest domain remain exact; the VUSL name does not add a supported wire alias. Parser acceptance does not establish provider support, authorization or verified effects. LNSAT and OpenShell remain independent optional systems.

[The consumer manifest](vusl-upstream.json) pins the upstream revision and hashes for Rangoon's offline fixture copies. Updates require explicit version review and conformance replay. Rangoon's implementation, integration status and release gates remain in this repository.

<!-- Preserve historical deep links; follow the pinned upstream guide above. -->
<a id="change-process"></a>
<a id="compatibility-claims"></a>
<a id="existing-offline-implementation"></a>
<a id="reproduce-existing-evidence"></a>
<a id="required-refusal-behavior"></a>
<a id="version-and-artifact-identity"></a>
<a id="vusl-conformance-and-evolution"></a>
