"""Independent stdlib reference validator for D0a candidate profile.

This checker applies closed D0a JSON and semantic rules to candidate vectors.
It is offline test oracle only. It does not map host records, establish
provenance, or accept public wire contract.
"""

import base64
import hashlib
import json
import random
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
VECTORS = ROOT / "docs" / "rusl-d0a-canonical-vectors.json"
NEGATIVE = ROOT / "docs" / "rusl-d0a-negative-vectors.json"
PROFILE = b"rangoon.rusl.observe.v0.1\x00"

MAX_INPUT_BYTES = 131_072
MAX_DEPTH = 16
MAX_VALUES = 256
MAX_OBJECT_FIELDS = 32
MAX_ARRAY_ITEMS = 64
PROFILE_ID = "rangoon.rusl.observe.v0.1"


class D0aError(ValueError):
    """Fixed parser diagnostic with no source text or native-data echo."""

    def __init__(self, code):
        super().__init__(code)
        self.code = code


@dataclass(frozen=True)
class Inspection:
    kind: str
    canonical: bytes
    identity: str


def fail(code):
    raise D0aError(code)


def reject_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if len(result) >= MAX_OBJECT_FIELDS:
            fail("collection_limit")
        if key in result:
            fail("duplicate_field")
        result[key] = value
    return result


def reject_constant(_value):
    fail("invalid_json")


def parse_candidate(value):
    """Parse bytes or text after D0a transport and JSON preflight."""
    if isinstance(value, bytes):
        raw = value
        if len(raw) > MAX_INPUT_BYTES:
            fail("input_limit")
        try:
            value = raw.decode("utf-8")
        except UnicodeDecodeError:
            fail("invalid_encoding")
    elif not isinstance(value, str):
        raise TypeError("candidate input must be bytes or str")
    else:
        try:
            raw = value.encode("utf-8")
        except UnicodeEncodeError:
            fail("invalid_encoding")
        if len(raw) > MAX_INPUT_BYTES:
            fail("input_limit")

    JsonPreflight(raw).document()
    try:
        parsed = json.loads(
            value,
            object_pairs_hook=reject_duplicate_keys,
            parse_constant=reject_constant,
        )
    except D0aError:
        raise
    except (ValueError, RecursionError, UnicodeError):
        fail("invalid_json")
    validate_json_shape(parsed)
    return parsed


class JsonPreflight:
    """Bounded JSON scanner with Rust parser diagnostic precedence."""

    def __init__(self, raw):
        self.raw = raw
        self.cursor = 0
        self.values = 0

    def document(self):
        self.value(1)
        self.space()
        if self.cursor != len(self.raw):
            fail("invalid_json")

    def space(self):
        while self.cursor < len(self.raw) and self.raw[self.cursor] in b" \n\r\t":
            self.cursor += 1

    def value(self, depth):
        self.space()
        if depth > MAX_DEPTH:
            fail("depth_limit")
        self.values += 1
        if self.values > MAX_VALUES:
            fail("value_limit")
        if self.cursor >= len(self.raw):
            fail("invalid_json")
        current = self.raw[self.cursor]
        if current == ord("{"):
            self.object(depth)
        elif current == ord("["):
            self.array(depth)
        elif current == ord('"'):
            self.string()
        elif current == ord("t"):
            self.literal(b"true")
        elif current == ord("f"):
            self.literal(b"false")
        elif current == ord("n"):
            self.literal(b"null")
        elif current == ord("-") or ord("0") <= current <= ord("9"):
            self.number()
        else:
            fail("invalid_json")

    def object(self, depth):
        self.cursor += 1
        self.space()
        if self.byte() == ord("}"):
            self.cursor += 1
            return
        keys = set()
        while True:
            if len(keys) >= MAX_OBJECT_FIELDS:
                fail("collection_limit")
            self.space()
            raw_key = self.string()
            try:
                key = json.loads(raw_key.decode("utf-8"))
            except (json.JSONDecodeError, UnicodeError):
                fail("invalid_json")
            if any(0xD800 <= ord(character) <= 0xDFFF for character in key):
                fail("invalid_json")
            if key in keys:
                fail("duplicate_field")
            keys.add(key)
            self.space()
            if self.byte() != ord(":"):
                fail("invalid_json")
            self.cursor += 1
            self.value(depth + 1)
            self.space()
            if self.byte() == ord("}"):
                self.cursor += 1
                return
            if self.byte() != ord(","):
                fail("invalid_json")
            self.cursor += 1

    def array(self, depth):
        self.cursor += 1
        self.space()
        if self.byte() == ord("]"):
            self.cursor += 1
            return
        count = 0
        while True:
            count += 1
            if count > MAX_ARRAY_ITEMS:
                fail("collection_limit")
            self.value(depth + 1)
            self.space()
            if self.byte() == ord("]"):
                self.cursor += 1
                return
            if self.byte() != ord(","):
                fail("invalid_json")
            self.cursor += 1

    def string(self):
        if self.byte() != ord('"'):
            fail("invalid_json")
        start = self.cursor
        self.cursor += 1
        while True:
            current = self.byte()
            if current == ord('"'):
                self.cursor += 1
                return self.raw[start:self.cursor]
            if current == ord("\\"):
                self.cursor += 1
                escaped = self.byte()
                if escaped is not None and escaped in b'"\\/bfnrt':
                    self.cursor += 1
                elif escaped == ord("u"):
                    self.cursor += 1
                    self.hex4()
                else:
                    fail("invalid_json")
            elif current is None or current <= 31:
                fail("invalid_json")
            elif current < 128:
                self.cursor += 1
            else:
                if 0xC2 <= current <= 0xDF:
                    width = 2
                elif 0xE0 <= current <= 0xEF:
                    width = 3
                elif 0xF0 <= current <= 0xF4:
                    width = 4
                else:
                    fail("invalid_json")
                if self.cursor + width > len(self.raw):
                    fail("invalid_json")
                try:
                    self.raw[self.cursor:self.cursor + width].decode("utf-8")
                except UnicodeDecodeError:
                    fail("invalid_json")
                self.cursor += width

    def hex4(self):
        for _ in range(4):
            current = self.byte()
            if current is None or not (
                ord("0") <= current <= ord("9")
                or ord("a") <= current <= ord("f")
                or ord("A") <= current <= ord("F")
            ):
                fail("invalid_json")
            self.cursor += 1

    def literal(self, literal):
        if self.raw[self.cursor:self.cursor + len(literal)] != literal:
            fail("invalid_json")
        self.cursor += len(literal)

    def number(self):
        start = self.cursor
        if self.byte() == ord("-"):
            self.cursor += 1
        current = self.byte()
        if current == ord("0"):
            self.cursor += 1
        elif current is not None and ord("1") <= current <= ord("9"):
            self.cursor += 1
            while self.byte() is not None and ord("0") <= self.byte() <= ord("9"):
                self.cursor += 1
        else:
            fail("invalid_json")
        if self.byte() == ord("."):
            self.cursor += 1
            fraction = self.cursor
            while self.byte() is not None and ord("0") <= self.byte() <= ord("9"):
                self.cursor += 1
            if self.cursor == fraction:
                fail("invalid_json")
        if self.byte() in (ord("e"), ord("E")):
            self.cursor += 1
            if self.byte() in (ord("+"), ord("-")):
                self.cursor += 1
            exponent = self.cursor
            while self.byte() is not None and ord("0") <= self.byte() <= ord("9"):
                self.cursor += 1
            if self.cursor == exponent:
                fail("invalid_json")
        if self.cursor == start:
            fail("invalid_json")

    def byte(self):
        if self.cursor >= len(self.raw):
            return None
        return self.raw[self.cursor]


def validate_json_shape(value):
    """Apply D0a structural caps before closed semantic validation."""
    count = 0

    def visit(current, depth):
        nonlocal count
        if depth > MAX_DEPTH:
            fail("depth_limit")
        count += 1
        if count > MAX_VALUES:
            fail("value_limit")
        if isinstance(current, dict):
            if len(current) > MAX_OBJECT_FIELDS:
                fail("collection_limit")
            for key, child in current.items():
                validate_scalar(key)
                visit(child, depth + 1)
        elif isinstance(current, list):
            if len(current) > MAX_ARRAY_ITEMS:
                fail("collection_limit")
            for child in current:
                visit(child, depth + 1)
        elif isinstance(current, str):
            validate_scalar(current)

    visit(value, 1)


def validate_scalar(value):
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        fail("invalid_json")


def canonical_bytes(value):
    """Canonical bytes for validated, number-free candidate subset."""
    reject_numbers(value)
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def reject_numbers(value):
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return
    if isinstance(value, (int, float)):
        raise ValueError("number outside candidate subset")
    if isinstance(value, list):
        for child in value:
            reject_numbers(child)
    elif isinstance(value, dict):
        for key, child in value.items():
            reject_numbers(key)
            reject_numbers(child)


def inspect_document(raw):
    document = parse_candidate(raw)
    root = require_object(document, "invalid_json")
    exact(
        root,
        {
            "languageVersion", "kind", "id", "createdAt", "origin", "native",
            "semantic", "extensions", "translation",
        },
    )
    if text(root, "languageVersion") != "rangoon.rusl.v0.1":
        fail("unsupported_version")
    kind = text(root, "kind")
    if kind not in {"capability_descriptor", "observation"}:
        fail("unsupported_kind")
    token(text(root, "id"))
    timestamp(text(root, "createdAt"))
    validate_origin(required_object(root, "origin"), kind)
    validate_native(required_object(root, "native"))
    validate_semantic(
        required_object(root, "semantic"), kind, required_object(root, "origin")
    )
    empty_array(root.get("extensions", missing()), "unsupported_extension")
    validate_translation(required_object(root, "translation"), kind)
    canonical = canonical_bytes(document)
    return Inspection(kind, canonical, hashlib.sha256(PROFILE + canonical).hexdigest())


def missing():
    return object()


def require_object(value, code):
    if not isinstance(value, dict):
        fail(code)
    return value


def required_object(mapping, key):
    value = mapping.get(key)
    if not isinstance(value, dict):
        fail("invalid_reference")
    return value


def exact(mapping, keys):
    if len(mapping) != len(keys) or set(mapping) != keys:
        fail("unknown_core_field")


def text(mapping, key):
    value = mapping.get(key)
    if not isinstance(value, str):
        fail("invalid_semantic_variant")
    return value


def bool_value(mapping, key):
    value = mapping.get(key)
    if not isinstance(value, bool):
        fail("invalid_native_reference")
    return value


def is_null(mapping, key):
    if key not in mapping:
        fail("unknown_core_field")
    return mapping[key] is None


def empty_array(value, code):
    if not isinstance(value, list) or value:
        fail(code)


def one_of(value, accepted):
    if value not in accepted:
        fail("invalid_semantic_variant")


def token(value):
    try:
        encoded = value.encode("ascii")
    except UnicodeEncodeError:
        fail("invalid_reference")
    if not 1 <= len(encoded) <= 128 or any(
        not (
            48 <= character <= 57
            or 65 <= character <= 90
            or 97 <= character <= 122
            or character in b"_.:-"
        )
        for character in encoded
    ):
        fail("invalid_reference")


def descriptive(value):
    if not 1 <= len(value.encode("utf-8")) <= 4096 or any(
        ord(character) <= 0x1F or 0x7F <= ord(character) <= 0x9F
        for character in value
    ):
        fail("invalid_semantic_variant")


def timestamp(value):
    punctuation = ((4, "-"), (7, "-"), (10, "T"), (13, ":"), (16, ":"), (19, "."), (23, "Z"))
    if len(value) != 24 or any(value[index] != literal for index, literal in punctuation):
        fail("invalid_semantic_variant")
    spans = ((0, 4), (5, 7), (8, 10), (11, 13), (14, 16), (17, 19), (20, 23))
    if not all(value[start:end].isascii() and value[start:end].isdigit() for start, end in spans):
        fail("invalid_semantic_variant")
    year, month, day, hour, minute, second = (int(value[start:end]) for start, end in spans[:6])
    if year == 0 or not 1 <= month <= 12 or hour > 23 or minute > 59 or second > 59:
        fail("invalid_semantic_variant")
    leap = year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)
    days = (31, 29 if leap else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if not 1 <= day <= days[month - 1]:
        fail("invalid_semantic_variant")


def validate_origin(origin, kind):
    if kind == "observation" and "sourceRef" not in origin:
        fail("invalid_reference")
    exact(
        origin,
        {
            "systemId", "environmentId", "engineId", "product", "productVersion",
            "protocol", "protocolVersion", "adapterId", "adapterVersion", "adapterDigest",
            "sourceRef", "observedAt", "receivedAt", "authenticity", "qualificationRef",
        },
    )
    descriptive(text(origin, "product"))
    descriptive(text(origin, "protocol"))
    if text(origin, "authenticity") != "unverified" or not is_null(origin, "qualificationRef"):
        fail("invalid_semantic_variant")
    if kind == "capability_descriptor":
        for key in (
            "systemId", "environmentId", "engineId", "productVersion", "protocolVersion",
            "adapterId", "adapterVersion", "adapterDigest", "sourceRef", "observedAt", "receivedAt",
        ):
            if not is_null(origin, key):
                fail("invalid_reference")
        return
    for key in (
        "systemId", "environmentId", "engineId", "productVersion", "protocolVersion", "adapterId", "adapterVersion",
    ):
        token(text(origin, key))
    digest = text(origin, "adapterDigest")
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        fail("invalid_reference")
    timestamp(text(origin, "observedAt"))
    timestamp(text(origin, "receivedAt"))
    source = required_object(origin, "sourceRef")
    validate_reference(source)
    for key in ("systemId", "environmentId", "engineId"):
        if text(source, key) != text(origin, key):
            fail("invalid_reference")


def validate_native(native):
    exact(native, {"state", "mediaType", "byteLength", "sha256", "hostRef", "redaction", "retention", "availableForRoundTrip"})
    if (
        text(native, "state") != "omitted"
        or text(native, "redaction") != "omitted"
        or text(native, "retention") != "none"
        or bool_value(native, "availableForRoundTrip")
        or not is_null(native, "mediaType")
        or not is_null(native, "byteLength")
        or not is_null(native, "sha256")
        or not is_null(native, "hostRef")
    ):
        fail("invalid_native_reference")


def validate_semantic(semantic, kind, origin):
    if kind == "capability_descriptor":
        exact(semantic, {"family", "operation", "effectClass", "supportState", "nativeOperationRef"})
        one_of(text(semantic, "family"), {"policy", "os_sandbox"})
        token(text(semantic, "operation"))
        one_of(text(semantic, "effectClass"), {"read", "write", "delete", "exec", "network", "inference", "mixed", "unknown"})
        if text(semantic, "supportState") != "unassessed" or not is_null(semantic, "nativeOperationRef"):
            fail("invalid_semantic_variant")
        return
    stage = text(semantic, "stage")
    if stage == "provider_policy_evaluation":
        exact(semantic, {"family", "stage", "operationRef", "nativeState", "projectedDecision"})
        if text(semantic, "family") != "policy":
            fail("invalid_semantic_variant")
        one_of(text(semantic, "projectedDecision"), {"allow", "deny", "unknown"})
    elif stage == "runtime_report":
        exact(semantic, {"family", "stage", "operationRef", "nativeState", "reportedEventClass"})
        if text(semantic, "family") != "os_sandbox":
            fail("invalid_semantic_variant")
        one_of(text(semantic, "reportedEventClass"), {"attempt", "active", "completed", "failed", "cancelled", "unknown"})
    else:
        fail("invalid_semantic_variant")
    token(text(semantic, "nativeState"))
    operation = required_object(semantic, "operationRef")
    validate_reference(operation)
    source = required_object(origin, "sourceRef")
    for key in ("systemId", "environmentId", "engineId", "contractVersion"):
        if text(operation, key) != text(source, key):
            fail("invalid_reference")


def validate_translation(translation, kind):
    exact(translation, {"profileId", "direction", "status", "authority", "fieldMap", "loss", "prerequisites"})
    if (
        text(translation, "profileId") != PROFILE_ID
        or text(translation, "direction") != "native_to_rusl"
        or text(translation, "authority") != "none"
    ):
        fail("unsupported_version")
    expected_status = "unassessed" if kind == "capability_descriptor" else "observation_only"
    if text(translation, "status") != expected_status:
        fail("invalid_semantic_variant")
    empty_array(translation.get("fieldMap", missing()), "invalid_semantic_variant")
    sorted_tokens(translation.get("loss", missing()))
    sorted_tokens(translation.get("prerequisites", missing()))


def sorted_tokens(value):
    if not isinstance(value, list):
        fail("invalid_semantic_variant")
    if len(value) > 16:
        fail("collection_limit")
    previous = None
    for current in value:
        if not isinstance(current, str):
            fail("invalid_semantic_variant")
        token(current)
        if previous is not None and previous >= current:
            fail("invalid_semantic_variant")
        previous = current


def validate_reference(reference):
    exact(reference, {"systemId", "environmentId", "engineId", "contractVersion", "nativeId"})
    for key in ("systemId", "environmentId", "engineId", "contractVersion", "nativeId"):
        token(text(reference, key))


class CandidateCanonicalVectors(unittest.TestCase):
    def test_independent_bytes_and_digests(self):
        vectors = parse_candidate(VECTORS.read_bytes())
        cases = vectors["cases"]
        self.assertEqual(len(cases), 6)
        for case in cases:
            with self.subTest(case=case["name"]):
                inspected = inspect_document(case["inputJson"].encode("utf-8"))
                self.assertEqual(inspected.canonical, case["canonicalJson"].encode("utf-8"))
                self.assertEqual(hashlib.sha256(inspected.canonical).hexdigest(), case["canonicalUtf8Sha256"])
                self.assertEqual(inspected.identity, case["documentDigestSha256"])

    def test_escape_and_key_order_do_not_change_candidate_identity(self):
        vectors = parse_candidate(VECTORS.read_bytes())
        by_name = {case["name"]: case for case in vectors["cases"]}
        escaped = by_name["policy_unicode_escaped"]
        reordered = by_name["policy_unicode_reordered"]
        self.assertNotEqual(escaped["inputJson"], reordered["inputJson"])
        self.assertEqual(
            inspect_document(escaped["inputJson"].encode("utf-8")).canonical,
            inspect_document(reordered["inputJson"].encode("utf-8")).canonical,
        )

    def test_duplicate_keys_and_numbers_are_outside_candidate_subset(self):
        with self.assertRaisesRegex(D0aError, "duplicate_field"):
            parse_candidate(b'{"a":true,"a":false}')
        for source in (b'{"value":1}', b'{"value":1e2}'):
            with self.assertRaisesRegex(ValueError, "number outside candidate subset"):
                canonical_bytes(parse_candidate(source))
        with self.assertRaisesRegex(D0aError, "invalid_json"):
            parse_candidate(b'{"value":NaN}')

    def test_preflight_diagnostic_precedence_matches_rust_scanner(self):
        first_key = '"field0":null'
        unique_fields = ",".join(
            f'"field{index}":null' for index in range(MAX_OBJECT_FIELDS)
        )
        with self.assertRaisesRegex(D0aError, "collection_limit"):
            parse_candidate(f"{{{unique_fields},{first_key}}}".encode("utf-8"))
        with self.assertRaisesRegex(D0aError, "duplicate_field"):
            parse_candidate(b'{"field0":null,"field0":null}')
        deeply_nested = b"[" * 1001 + b"null" + b"]" * 1001
        with self.assertRaisesRegex(D0aError, "depth_limit"):
            parse_candidate(deeply_nested)

    def test_malformed_root_wins_before_later_depth(self):
        with self.assertRaisesRegex(D0aError, "invalid_json"):
            parse_candidate(b"x" + b"[" * 17)

    def test_malformed_nested_token_wins_before_depth(self):
        with self.assertRaisesRegex(D0aError, "invalid_json"):
            parse_candidate(b"[" * 15 + b"x" + b"]" * 15)

    def test_depth_wins_before_later_syntax_error(self):
        with self.assertRaisesRegex(D0aError, "depth_limit"):
            parse_candidate(b"[" * 17 + b"x")

    def test_escaped_brackets_are_not_containers(self):
        self.assertEqual(
            parse_candidate(b'"\\u005b\\u005b\\u005d\\u005d"'), "[[]]"
        )

    def test_string_surrogate_has_fixed_encoding_diagnostic(self):
        with self.assertRaisesRegex(D0aError, "invalid_encoding"):
            parse_candidate("\ud800")

    def test_trailing_string_escape_has_fixed_json_diagnostic(self):
        with self.assertRaisesRegex(D0aError, "invalid_json"):
            parse_candidate(bytes((34, 92)))

    def test_ascii_byte_pairs_never_leak_python_exceptions(self):
        for first in range(128):
            for second in range(128):
                with self.subTest(first=first, second=second):
                    try:
                        parse_candidate(bytes((first, second)))
                    except D0aError:
                        pass
                    except Exception as error:
                        self.fail(f"raw {type(error).__name__} for {first},{second}")

    def test_bounded_seeded_byte_mutation_smoke_never_leaks_python_exception(self):
        """Seeded smoke only; it is not full fuzz or property conformance."""
        vectors = parse_candidate(VECTORS.read_bytes())
        source = vectors["cases"][1]["inputJson"].encode("utf-8")
        generator = random.Random(4017)
        for case in range(6_000):
            mutated = bytearray(source)
            for _ in range(generator.randint(1, 4)):
                mutated[generator.randrange(len(mutated))] = generator.randrange(256)
            with self.subTest(case=case):
                try:
                    inspected = inspect_document(bytes(mutated))
                    self.assertIsInstance(inspected, Inspection)
                except D0aError:
                    pass
                except Exception as error:
                    self.fail(f"raw {type(error).__name__} for mutation {case}")

    def test_all_parser_addressable_negative_vectors_have_pinned_results(self):
        vectors = parse_candidate(NEGATIVE.read_bytes())
        checked = 0
        for group in vectors["cases"]:
            if group["id"] == "D0A-N11":
                continue
            for item in group["inputs"]:
                checked += 1
                raw = (
                    item["inputUtf8"].encode("utf-8")
                    if "inputUtf8" in item
                    else base64.b64decode(item["inputBase64"], validate=True)
                )
                expected = item["expectedParser"]
                with self.subTest(group=group["id"], case=item["name"]):
                    if expected == "accept":
                        inspect_document(raw)
                    else:
                        with self.assertRaises(D0aError) as failure:
                            inspect_document(raw)
                        self.assertEqual(failure.exception.code, expected)
        self.assertEqual(checked, 20)

    def test_n11_is_host_only_and_not_a_parser_case(self):
        vectors = parse_candidate(NEGATIVE.read_bytes())
        n11 = next(group for group in vectors["cases"] if group["id"] == "D0A-N11")
        self.assertEqual(len(n11["inputs"]), 1)
        case = n11["inputs"][0]
        self.assertIsNone(case["expectedParser"])
        self.assertEqual(case["inputKind"], "o1_mapping_fragment")
        self.assertIn("refuse", case["expectedHost"])
        self.assertNotIn("inputBase64", case)
        self.assertIn("inputUtf8", case)


if __name__ == "__main__":
    unittest.main()
