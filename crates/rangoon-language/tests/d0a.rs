use rangoon_language::{Diagnostic, Kind, MAX_INPUT_BYTES, inspect};
use serde_json::Value;

const CANONICAL: &str = include_str!("../../../docs/rusl-d0a-canonical-vectors.json");
const NEGATIVE: &str = include_str!("../../../docs/rusl-d0a-negative-vectors.json");

#[test]
fn canonical_vectors_match_candidate_bytes_and_domain_separated_identity() {
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    let cases = vectors["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let inspected = inspect(case["inputJson"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(
            inspected.canonical_bytes(),
            case["canonicalJson"].as_str().unwrap().as_bytes(),
            "{}",
            case["name"]
        );
        assert_eq!(
            sha256(inspected.canonical_bytes()),
            case["canonicalUtf8Sha256"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            inspected.identity_sha256(),
            case["documentDigestSha256"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn canonical_vectors_report_only_the_two_inert_kinds() {
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    for case in vectors["cases"].as_array().unwrap() {
        let result = inspect(case["inputJson"].as_str().unwrap().as_bytes()).unwrap();
        let expected = match case["name"].as_str().unwrap() {
            "descriptor_ascii" => Kind::CapabilityDescriptor,
            _ => Kind::Observation,
        };
        assert_eq!(result.kind(), expected);
    }
}

#[test]
fn literal_negative_d0a_documents_return_the_pinned_codes() {
    let vectors: Value = serde_json::from_str(NEGATIVE).unwrap();
    for group in vectors["cases"].as_array().unwrap() {
        if group["id"] == "D0A-N11" {
            continue;
        }
        for input in group["inputs"].as_array().unwrap() {
            let bytes = match input.get("inputUtf8") {
                Some(value) => value.as_str().unwrap().as_bytes().to_vec(),
                None => decode_base64(input["inputBase64"].as_str().unwrap()),
            };
            let expected = input["expectedParser"].as_str().unwrap();
            match expected {
                "accept" => assert!(inspect(&bytes).is_ok(), "{} {}", group["id"], input["name"]),
                code => assert_eq!(
                    inspect(&bytes).unwrap_err().code(),
                    code,
                    "{} {}",
                    group["id"],
                    input["name"]
                ),
            }
        }
    }
}

#[test]
fn host_only_o1_mapping_fixture_is_not_a_parser_document() {
    let vectors: Value = serde_json::from_str(NEGATIVE).unwrap();
    let group = vectors["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "D0A-N11")
        .unwrap();
    assert!(
        group["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|input| input["expectedParser"].is_null())
    );
}

#[test]
fn limits_and_exact_timestamp_edges_fail_closed() {
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    let base = vectors["cases"][0]["inputJson"].as_str().unwrap();
    let padding = " ".repeat(MAX_INPUT_BYTES - base.len());
    assert!(inspect(format!("{base}{padding}").as_bytes()).is_ok());
    assert_eq!(
        inspect(format!("{base}{padding} ").as_bytes()).unwrap_err(),
        Diagnostic::InputLimit
    );
    assert_eq!(
        inspect(format!("{}null{}", "[".repeat(15), "]".repeat(15)).as_bytes()).unwrap_err(),
        Diagnostic::InvalidJson
    );
    assert_eq!(
        inspect(format!("{}null{}", "[".repeat(17), "]".repeat(17)).as_bytes()).unwrap_err(),
        Diagnostic::DepthLimit
    );
    let exact_values = format!(
        "[{}, {}, {}, {}]",
        array_of_nulls(),
        array_of_nulls(),
        array_of_nulls(),
        array_of_nulls_n(59)
    );
    assert_eq!(
        inspect(exact_values.as_bytes()).unwrap_err(),
        Diagnostic::InvalidJson
    );
    let values = format!(
        "[{}, {}, {}, {}]",
        array_of_nulls(),
        array_of_nulls(),
        array_of_nulls(),
        array_of_nulls()
    );
    assert_eq!(
        inspect(values.as_bytes()).unwrap_err(),
        Diagnostic::ValueLimit
    );
    let invalid_day = base.replacen("2026-10-16T00:00:00.000Z", "2026-02-29T00:00:00.000Z", 1);
    assert_eq!(
        inspect(invalid_day.as_bytes()).unwrap_err(),
        Diagnostic::InvalidSemanticVariant
    );
    assert_eq!(
        inspect(base.replacen("2026-10", "+001-10", 1).as_bytes()).unwrap_err(),
        Diagnostic::InvalidSemanticVariant
    );
    assert_eq!(
        inspect(base.replacen("2026-10", "2026-+1", 1).as_bytes()).unwrap_err(),
        Diagnostic::InvalidSemanticVariant
    );
    assert_eq!(
        inspect(array_of_nulls().as_bytes()).unwrap_err(),
        Diagnostic::InvalidJson
    );
    assert_eq!(
        inspect(array_of_nulls_n(65).as_bytes()).unwrap_err(),
        Diagnostic::CollectionLimit
    );
    assert_eq!(
        inspect(object_with_nulls(32).as_bytes()).unwrap_err(),
        Diagnostic::UnknownCoreField
    );
    assert_eq!(
        inspect(object_with_nulls(33).as_bytes()).unwrap_err(),
        Diagnostic::CollectionLimit
    );
}

#[test]
fn unknown_versions_kinds_and_extensions_do_not_downconvert() {
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    let base = vectors["cases"][1]["inputJson"].as_str().unwrap();
    assert_eq!(
        inspect(
            base.replacen("rangoon.rusl.v0.1", "rangoon.rusl.v0.0", 1)
                .as_bytes()
        )
        .unwrap_err(),
        Diagnostic::UnsupportedVersion
    );
    assert_eq!(
        inspect(base.replacen("\"observation\"", "\"action\"", 1).as_bytes()).unwrap_err(),
        Diagnostic::UnsupportedKind
    );
    assert_eq!(
        inspect(
            base.replacen("\"extensions\":[]", "\"extensions\":[\"future\"]", 1)
                .as_bytes()
        )
        .unwrap_err(),
        Diagnostic::UnsupportedExtension
    );
    assert_eq!(
        inspect(
            base.replacen("synthetic-policy-engine", "\\uD800", 1)
                .as_bytes()
        )
        .unwrap_err(),
        Diagnostic::InvalidJson
    );
    assert_eq!(
        inspect(
            base.replacen(
                "\"product\":\"synthetic-policy-engine\"",
                "\"product\":\"synthetic-policy-engine\",\"product\":\"again\"",
                1,
            )
            .as_bytes()
        )
        .unwrap_err(),
        Diagnostic::DuplicateField
    );
    assert_eq!(
        inspect(
            base.replacen(
                "\"id\":\"fixture:reported-allow-7\"",
                "\"id\":\"fixture:reported-allow-7\",\"\\u0069d\":\"duplicate\"",
                1,
            )
            .as_bytes()
        )
        .unwrap_err(),
        Diagnostic::DuplicateField
    );
}

#[test]
fn descriptor_text_and_observation_reference_limits_are_exact() {
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    let descriptor: Value =
        serde_json::from_str(vectors["cases"][0]["inputJson"].as_str().unwrap()).unwrap();
    let observation: Value =
        serde_json::from_str(vectors["cases"][1]["inputJson"].as_str().unwrap()).unwrap();

    let mut at_limit = descriptor.clone();
    at_limit["origin"]["product"] = Value::String("é".repeat(2048));
    assert!(inspect(&serde_json::to_vec(&at_limit).unwrap()).is_ok());
    at_limit["origin"]["product"] = Value::String(format!("{}a", "é".repeat(2048)));
    assert_eq!(
        inspect(&serde_json::to_vec(&at_limit).unwrap()).unwrap_err(),
        Diagnostic::InvalidSemanticVariant
    );

    let mut at_limit = observation.clone();
    let long_token = "x".repeat(128);
    at_limit["origin"]["sourceRef"]["nativeId"] = Value::String(long_token.clone());
    at_limit["semantic"]["operationRef"]["nativeId"] = Value::String(long_token);
    assert!(inspect(&serde_json::to_vec(&at_limit).unwrap()).is_ok());
    at_limit["origin"]["sourceRef"]["nativeId"] = Value::String("x".repeat(129));
    assert_eq!(
        inspect(&serde_json::to_vec(&at_limit).unwrap()).unwrap_err(),
        Diagnostic::InvalidReference
    );
    at_limit["origin"]["sourceRef"]["nativeId"] = Value::String("../secret".into());
    assert_eq!(
        inspect(&serde_json::to_vec(&at_limit).unwrap()).unwrap_err(),
        Diagnostic::InvalidReference
    );
}

#[test]
fn seeded_byte_mutations_never_panic_or_return_partial_inspection() {
    // Reproducible malformed-input smoke, not coverage-guided fuzzing.
    let vectors: Value = serde_json::from_str(CANONICAL).unwrap();
    let sources: Vec<Vec<u8>> = vectors["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["inputJson"].as_str().unwrap().as_bytes().to_vec())
        .collect();
    let mut state = 0x5a17_2026_u64;
    for case in 0..12_000 {
        let mut candidate = sources[case % sources.len()].clone();
        for _ in 0..(next_random(&mut state) % 4 + 1) {
            let offset = (next_random(&mut state) as usize) % candidate.len();
            candidate[offset] = next_random(&mut state) as u8;
        }
        let outcome = std::panic::catch_unwind(|| inspect(&candidate));
        let inspected = outcome.unwrap_or_else(|_| panic!("parser panicked on mutation {case}"));
        if let Ok(document) = inspected {
            assert!(!document.canonical_bytes().is_empty(), "mutation {case}");
            assert_eq!(document.identity_sha256().len(), 64, "mutation {case}");
        }
    }
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    *state >> 32
}

fn array_of_nulls() -> String {
    array_of_nulls_n(64)
}

fn array_of_nulls_n(count: usize) -> String {
    format!(
        "[{}]",
        std::iter::repeat_n("null", count)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn object_with_nulls(count: usize) -> String {
    format!(
        "{{{}}}",
        (0..count)
            .map(|index| format!("\"field{index}\":null"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn decode_base64(input: &str) -> Vec<u8> {
    assert_eq!(input.len() % 4, 0);
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.as_bytes().chunks_exact(4) {
        let padding = chunk.iter().filter(|&&byte| byte == b'=').count();
        assert!(
            padding <= 2 && (padding == 0 || chunk[4 - padding..].iter().all(|&byte| byte == b'='))
        );
        let sextets = [chunk[0], chunk[1], chunk[2], chunk[3]].map(|byte| {
            if byte == b'=' {
                0
            } else {
                base64_value(byte).unwrap()
            }
        });
        output.push((sextets[0] << 2) | (sextets[1] >> 4));
        if padding < 2 {
            output.push((sextets[1] << 4) | (sextets[2] >> 2));
        }
        if padding == 0 {
            output.push((sextets[2] << 6) | sextets[3]);
        }
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
