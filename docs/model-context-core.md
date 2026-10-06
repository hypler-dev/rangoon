# M1a context-pack and advisory-response core

Authority: the accepted October 6 continuation in [intent.md](intent.md), with the security and product requirements in [model-assistance.md](model-assistance.md). This specification freezes the first pure implementation. It adds no native command, database, credential, network, tokenizer or model call. Implementation and review evidence belongs in [development.md](development.md).

## API and custody

`rangoon-model-assistance` exposes `prepare_pack(raw_json: &[u8]) -> Result<ContextPack, Diagnostic>` and `validate_response(pack: &ContextPack, raw_json: &[u8]) -> Result<ValidatedResponse, Diagnostic>`. Pack and validated response records are serializable, but their fields and constructors are private and they are not deserializable. Input records are decoded only through the bounded entry point. A pack can be cloned without changing its contents.

The pure API accepts already resolved content. It checks closed reference shape and exact content digest, not database existence, full revision ancestry, source-name identity derivation, peer identity or user consent. The future native resolver must open existing validated records by ID, reject deleted/stale dependencies and bind the operator's payload decision. No content from a renderer or model may substitute for those resolved records. This library does not make a payload authorized to send.

The pack body is a provider-neutral analysis document, **not a provider HTTP request**. M1b/M1c must bind the exact final adapter request, including message wrappers, endpoint/model configuration and overhead, to a fresh payload decision. Their final request remains bounded to 256 KiB even if this body alone fits. No claim of exact model-token accounting follows; M1a always reports `tokenAccounting: "unknown"` and does not accept caller-supplied token counts as proof.

## Closed request

All keys below are mandatory; null, unknown or duplicate keys reject. Field order in incoming JSON is immaterial. All integers are unsigned decimal JSON integer literals (no minus, fractional or exponent form). Schema version is exactly `rangoon.context-pack-request.v1`.

```text
Request { schemaVersion, task, target, maxBodyBytes, inputs }
task = "classify_v1" | "decompose_v1" | "compare_v1"
Target { profileId, profileSha256, model, maxOutputTokens }
ResolvedInput { input, scope, content, requiredProtectedRanges, selections }
Selection { startByte, endByte, protected }
```

`input` uses the closed source/revision union from the parent contract and existing domain `InputReference`. Each ID must have its exact `source:`, `capability:` or `revision:` prefix followed by 64 lowercase hexadecimal characters; digests are 64 lowercase hexadecimal characters. A repeated immutable input ID rejects, including one repeated with different digest or owner. Different revision IDs from the same capability are distinct permitted inputs.

`profileId` is a nonsecret local identifier: 1–64 ASCII letters, digits, `_`, `-` or `.`. `profileSha256` is the nonsecret connection configuration digest; the pure core checks its shape only. `model` is 1–128 printable ASCII characters, with no whitespace, backslash, quote or URL credentials. It is a model label, never a URL or a credential. `maxOutputTokens` is 1–32,768; `maxBodyBytes` is 1–262,144. These are explicit requested limits, not proof of provider support. `scope` is explicit inert operator/resolver context, 0–512 UTF-8 bytes without control characters; it must not be inferred from text or treated as permission.

Require 1–16 inputs; comparison additionally requires exactly two revision inputs. Each input must have at least one nonempty selected range. `requiredProtectedRanges` is a mandatory array (possibly empty) of `{startByte,endByte}` intervals supplied by the trusted native task/operator selection contract, never inferred from imported prose or model output. Existing saved records do not store protection flags. The future native gate must bind these obligations to the task and payload decision; a raw pure request cannot authenticate them. Validate every required interval and require its union to be fully covered by the union of selections marked `protected: true`; an omitted or downgraded required span is `range_invalid`. At most 256 required intervals are allowed across all inputs, separate from the 256 explicit selection ceiling. Exact UTF-8 content and all selected/protected ranges follow the parent contract. Empty content cannot satisfy a nonempty selection. Source content is never normalized, summarized or stripped. Invalid source/revision ID shape is `input_invalid`; a content/digest disagreement or conflicting immutable reference is `identity_mismatch`.

## Canonical packing

Preserve explicit input order. Sort each input's selections by `(startByte, endByte, protected)` and union overlapping/adjacent intervals into `selectedRanges`. Independently union required intervals into `requiredProtectedRanges` and protected selections into `protectedRanges`. Their original byte coverage is exact: protection never disappears when protected and ordinary selections overlap. Compute `omittedRanges` as the complement of selected ranges in `[0, content byte length)`; omit empty complements. No omitted text is copied into the body.

Visit the selected ranges in input order and ascending byte order. Each resulting block contains exact selected text. A byte-identical earlier block is reused; no substring, near-duplicate or semantic deduplication occurs. Keep one alias for every input/range, even when another input supplies the identical text. An alias is `{inputIndex,startByte,endByte,protected}`; its input reference and scope resolve through that exact input index. `protected` is true if any protected range overlaps the alias; the exact protected boundaries remain in the input record. The first visited block owns the text. Equal text never merges source identity, scope or authority.

Canonical compact UTF-8 JSON uses the following exact key order; nested objects follow the listed order. Strings use the Rust `serde_json` compact escaping convention: Unicode remains UTF-8, `"`, `\\` and standard control escapes are escaped, remaining U+0000–001F use lowercase `\u00xx`; no ASCII-only Unicode escaping or normalization. No terminal newline.

```text
Body {
  schemaVersion: "rangoon.analysis-body.v1",
  task, target, maxBodyBytes,
  templateVersion: "1", templateSha256, instructions,
  responseSchema: "rangoon.analysis-proposals.v1",
  inputs: [{input,scope,byteLength,requiredProtectedRanges,selectedRanges,protectedRanges,omittedRanges}],
  blocks: [{text,aliases:[{inputIndex,startByte,endByte,protected}]}],
  tokenAccounting: "unknown", authority: "none"
}
Range { startByte, endByte }
Target and InputReference key order follow their declarations above and the domain enum.
```

The fixed code mapping is `classify_v1` → `templates/classify_v1.txt`, `decompose_v1` → `templates/decompose_v1.txt`, and `compare_v1` → `templates/compare_v1.txt`, relative to the new crate. Templates are compiled with `include_str!`; no caller-supplied template or runtime path is accepted. Each file begins with the exact line `Task: <task>` for its mapped task; pack construction rejects a mismatch. Template instructions are three checked-in UTF-8 text files, each ending in one LF. Their digest covers those exact bytes. Every template specifies one task, untrusted input roles, the exact response schema, citation syntax, uncertainty behavior and the prohibition on authority or tool actions. The templates may influence a model but are not an enforcement mechanism. Changing template bytes changes `templateSha256` and therefore pack identity.

Serialize through a capped writer; exceeding `maxBodyBytes` returns `pack_over_budget` and no pack. Identity is `pack:` plus lowercase SHA-256 of `b"rangoon.context-pack.v1\0" || u64_be(body length) || body`. The serialized `ContextPack` has exact ordered fields `{schemaVersion:"rangoon.context-pack.v1",packId,bodyJson,bodySha256,bodyBytes,selectedBytes,uniqueTextBytes,omittedBytes,tokenAccounting:"unknown",authority:"none"}`. `selectedBytes` sums the union coverage of each input; `uniqueTextBytes` sums each transmitted block once; `omittedBytes` sums computed complements. These are text-byte measurements, not token savings or semantic-equivalence claims. Metadata overhead can make the final body larger than the selected text.

## Closed model response

The model returns only the following JSON. No wrapper, Markdown fence or extra prose is accepted. Every field is required, and all keys are closed.

```text
Response { schemaVersion:"rangoon.analysis-proposals.v1", task, proposals, uncertainties }
Proposal { kind, title, authoredText, explanation, citations }
kind = "classification" | "capability" | "difference"
Citation { input, startByte, endByte }
```

The response task must equal the pack task. Only the corresponding kind is allowed: `classify_v1` → `classification`, `decompose_v1` → `capability`, `compare_v1` → `difference`. Each proposal requires a nonempty title of at most 120 UTF-8 bytes, an explanation of 1–1,024 bytes, and 1–64 citations. `authoredText` may be empty and is always model-authored, even if it happens to repeat source text. Titles/explanations cannot be all whitespace; output remains inert text, including HTML, code and links. At most 16 proposals are accepted. An empty proposal list is valid and does not prove no issue exists. Uncertainties are nonempty strings of at most 1,024 bytes, with at most 32 entries.

Citations must match one complete tagged reference in the pack and remain wholly inside one selected union interval. Resolve their byte boundaries against the exact block text for that input alias; offsets are absolute in that input, never in a normalized string. Reject unknown references, omitted spans, mid-character offsets and cross-input substitution. A citation only demonstrates a valid reference to selected bytes; it does not prove the model's assertion follows from them.

Validation first enforces raw and structural bounds, then task, references and ranges. A bad response returns no validated record. Successful output is the closed ordered envelope `{schemaVersion:"rangoon.validated-analysis.v1",packId,responseSha256,task,proposals,uncertainties,contentKind:"model_authored",authority:"none"}`. `packId` and `task` come from the immutable local pack. `responseSha256` covers exact raw received bytes. The provider cannot assign these local metadata fields. There is no execution/review state, signed receipt, automatic apply or stored mutation in this envelope. A later transport must separately prove complete termination and fresh inputs before treating it as an inspectable completed proposal; parser success alone is not a transport-completion result.

## Allocation and negative-input qualification

Apply every limit in [model-assistance.md](model-assistance.md). Before typed deserialization, perform bounded structural scanning of the raw UTF-8: count container depth, values, array items, object fields, decoded string bytes, field-specific counts and aggregate content/text/citation totals. Detect duplicate object keys by their decoded values, including escaped aliases. Do not allocate complete value strings merely to measure them; scan JSON escapes and surrogate pairs while counting decoded UTF-8 bytes. Keys are bounded to 128 decoded bytes before allocation. Reject lone surrogates and invalid escapes. Numeric tokens are at most 20 decimal digits and must fit `u64`.

A bounded preflight followed by closed typed deserialization is acceptable only when all field-specific string/collection and aggregate limits have already passed. Structural malformed/unknown wire values map to `input_invalid` or `response_invalid`; size/depth/count/string ceilings map to `input_limit` or `response_limit`. No diagnostic echoes input. Resource limits apply even inside an unknown field, before eventual closed-schema rejection. Root is one container level; a nested array/object increments depth, scalar values do not.

Required checks include independent canonical pack vectors and digest framing; exact byte/range/protection/alias/omission accounting; model/task/profile/template changes altering identity; each ceiling and one-over rejection; escaped duplicate keys; unknown/future versions; hostile HTML and prompt-injection text retained as inert data; invalid UTF-8/surrogates; BOM/CRLF/multibyte boundaries; revised-content citations; forged/missing/omitted references; wrong task/kind; and a rejected response yielding no success record. Tests use public or synthetic data only. This source foundation does not qualify model quality, connectivity, token compression or application security.
