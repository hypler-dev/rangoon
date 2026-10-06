use super::*;
use serde_json::{Value, json};

fn scan(value: Value, kind: WireKind) -> Result<(), Diagnostic> {
    preflight(&serde_json::to_vec(&value).unwrap(), kind)
}

#[test]
fn raw_byte_caps_apply_before_decoding_and_allow_exact_boundary() {
    for (kind, cap) in [
        (WireKind::Request, 8 * 1024 * 1024),
        (WireKind::Response, 128 * 1024),
    ] {
        let mut bytes = b"{}".to_vec();
        bytes.resize(cap, b' ');
        assert!(preflight(&bytes, kind).is_ok());
        bytes.push(b' ');
        assert_eq!(preflight(&bytes, kind), Err(kind.limit()));
        bytes[0] = 255;
        assert_eq!(preflight(&bytes, kind), Err(kind.limit()));
    }
}

#[test]
fn rejects_invalid_utf8_escapes_surrogates_numbers_duplicates_and_trailing_json() {
    for raw in [
        b"\xff".as_slice(),
        br#"{"a":1,"\u0061":2}"#,
        br#"{"a":1,"a":2}"#,
        br#""\ud800""#,
        br#""\udc00""#,
        br#""\ud800\u0041""#,
        br#""\x00""#,
        b"\"\n\"",
        b"{}{}",
        b"01",
        b"-1",
        b"1e0",
        b"1.0",
        b"18446744073709551616",
        b"[0,]",
        b"{\"a\":0,}",
        b"[true false]",
        b"{\"a\" 0}",
        b"[",
        b"\"unterminated",
    ] {
        assert_eq!(
            preflight(raw, WireKind::Request),
            Err(Diagnostic::InputInvalid),
            "{raw:?}"
        );
        assert_eq!(
            preflight(raw, WireKind::Response),
            Err(Diagnostic::ResponseInvalid),
            "{raw:?}"
        );
    }
    for raw in [
        br#""\ud83d\ude00""#.as_slice(),
        b"18446744073709551615",
        br#"{"\u0061":1}"#,
    ] {
        assert!(preflight(raw, WireKind::Request).is_ok());
    }
}

#[test]
fn generic_depth_array_object_key_and_string_limits_are_exact() {
    for kind in [WireKind::Request, WireKind::Response] {
        let exact = format!("{}0{}", "[".repeat(16), "]".repeat(16));
        assert!(preflight(exact.as_bytes(), kind).is_ok());
        assert_eq!(
            preflight(format!("[{exact}]").as_bytes(), kind),
            Err(kind.limit())
        );
        assert!(scan(json!(vec![0; 512]), kind).is_ok());
        assert_eq!(scan(json!(vec![0; 513]), kind), Err(kind.limit()));
        for count in [32, 33] {
            let value: Value = (0..count).map(|i| (format!("k{i}"), json!(0))).collect();
            assert_eq!(
                scan(value, kind),
                if count == 32 {
                    Ok(())
                } else {
                    Err(kind.limit())
                }
            );
        }
        assert!(scan(json!({"k".repeat(128):0}), kind).is_ok());
        assert_eq!(scan(json!({"k".repeat(129):0}), kind), Err(kind.limit()));
        assert!(scan(json!({"x":"é".repeat(32 * 1024)}), kind).is_ok());
        assert_eq!(
            scan(json!({"x":format!("{}a", "é".repeat(32 * 1024))}), kind),
            Err(kind.limit())
        );
    }
}

#[test]
fn total_value_counter_bounds_even_small_strings_and_containers() {
    for kind in [WireKind::Request, WireKind::Response] {
        let mut values: Vec<Value> = (0..15).map(|_| json!(vec![0; 512])).collect();
        values.push(json!(vec![0; 495])); // root + 15*(array+512) + array+495 = 8192
        assert!(scan(json!(values), kind).is_ok());
        values[15] = json!(vec![0; 496]);
        assert_eq!(scan(json!(values), kind), Err(kind.limit()));
    }
}

#[test]
fn input_content_scope_target_and_range_caps_precede_typed_allocation() {
    let k = WireKind::Request;
    assert!(
        scan(
            json!({"inputs":vec![json!({"content":"a".repeat(256*1024)});16]}),
            k
        )
        .is_ok()
    );
    assert_eq!(
        scan(json!({"inputs":[{"content":"a".repeat(256*1024+1)}]}), k),
        Err(k.limit())
    );
    assert_eq!(
        scan(json!({"inputs":vec![json!({});17]}), k),
        Err(k.limit())
    );
    for (key, cap) in [("scope", 512), ("content", 256 * 1024)] {
        assert!(scan(json!({"inputs":[{key:"a".repeat(cap)}]}), k).is_ok());
        assert_eq!(
            scan(json!({"inputs":[{key:"a".repeat(cap+1)}]}), k),
            Err(k.limit())
        );
    }
    for (key, cap) in [("profileId", 64), ("profileSha256", 64), ("model", 128)] {
        assert!(scan(json!({"target":{key:"a".repeat(cap)}}), k).is_ok());
        assert_eq!(
            scan(json!({"target":{key:"a".repeat(cap+1)}}), k),
            Err(k.limit())
        );
    }
    assert!(
        scan(
            json!({"inputs":[{"requiredProtectedRanges":vec![0;256]}]}),
            k
        )
        .is_ok()
    );
    assert_eq!(
        scan(
            json!({"inputs":[{"requiredProtectedRanges":vec![0;257]}]}),
            k
        ),
        Err(k.limit())
    );
    assert_eq!(
        scan(
            json!({"inputs":[{"requiredProtectedRanges":vec![0;128]},{"requiredProtectedRanges":vec![0;129]}]}),
            k
        ),
        Err(k.limit())
    );
    assert!(scan(json!({"inputs":[{"selections":vec![0;256]}]}), k).is_ok());
    assert_eq!(
        scan(json!({"inputs":[{"selections":vec![0;257]}]}), k),
        Err(k.limit())
    );
    assert!(
        scan(
            json!({"inputs":[{"selections":vec![0;128]},{"selections":vec![0;128]}]}),
            k
        )
        .is_ok()
    );
    assert_eq!(
        scan(
            json!({"inputs":[{"selections":vec![0;128]},{"selections":vec![0;129]}]}),
            k
        ),
        Err(k.limit())
    );
}

#[test]
fn response_specific_and_aggregate_limits_are_checked_before_materialization() {
    let k = WireKind::Response;
    for (key, cap) in [
        ("title", 120),
        ("authoredText", 16 * 1024),
        ("explanation", 1024),
    ] {
        assert!(scan(json!({"proposals":[{key:"a".repeat(cap)}]}), k).is_ok());
        assert_eq!(
            scan(json!({"proposals":[{key:"a".repeat(cap+1)}]}), k),
            Err(k.limit())
        );
    }
    assert!(scan(json!({"proposals":vec![json!({});16]}), k).is_ok());
    assert_eq!(
        scan(json!({"proposals":vec![json!({});17]}), k),
        Err(k.limit())
    );
    assert!(scan(json!({"proposals":[{"citations":vec![0;64]}]}), k).is_ok());
    assert_eq!(
        scan(json!({"proposals":[{"citations":vec![0;65]}]}), k),
        Err(k.limit())
    );
    let mut proposals = vec![json!({"citations":vec![0;64],"authoredText":"a".repeat(16*1024)}); 4];
    assert!(scan(json!({"proposals":proposals}), k).is_ok());
    proposals.push(json!({"citations":[0]}));
    assert_eq!(scan(json!({"proposals":proposals}), k), Err(k.limit()));
    proposals[4] = json!({"authoredText":"x"});
    assert_eq!(scan(json!({"proposals":proposals}), k), Err(k.limit()));
    assert!(scan(json!({"uncertainties":vec!["a".repeat(1024);32]}), k).is_ok());
    assert_eq!(
        scan(json!({"uncertainties":vec!["a";33]}), k),
        Err(k.limit())
    );
    assert_eq!(
        scan(json!({"uncertainties":["a".repeat(1025)]}), k),
        Err(k.limit())
    );
    // The request-only long-content exception must not apply to model output.
    assert_eq!(
        scan(json!({"inputs":[{"content":"a".repeat(65537)}]}), k),
        Err(k.limit())
    );
}

#[test]
fn escaped_text_is_measured_by_decoded_utf8_bytes_without_constructing_it() {
    let exact = format!(
        r#"{{"inputs":[{{"scope":"{}"}}]}}"#,
        "\\ud83d\\ude00".repeat(128)
    );
    assert!(preflight(exact.as_bytes(), WireKind::Request).is_ok());
    let excess = format!(
        r#"{{"inputs":[{{"scope":"{}x"}}]}}"#,
        "\\ud83d\\ude00".repeat(128)
    );
    assert_eq!(
        preflight(excess.as_bytes(), WireKind::Request),
        Err(Diagnostic::InputLimit)
    );
    let key = "\\u0061".repeat(128);
    assert!(preflight(format!(r#"{{"{key}":0}}"#).as_bytes(), WireKind::Request).is_ok());
}

#[test]
fn numeric_token_width_is_a_resource_limit_and_u64_overflow_is_invalid() {
    for kind in [WireKind::Request, WireKind::Response] {
        assert!(preflight(b"18446744073709551615", kind).is_ok());
        assert_eq!(
            preflight(b"18446744073709551616", kind),
            Err(kind.invalid())
        );
        assert_eq!(preflight(b"100000000000000000000", kind), Err(kind.limit()));
    }
}

#[test]
fn json_whitespace_accepts_crlf_but_bom_is_only_valid_inside_a_string() {
    for kind in [WireKind::Request, WireKind::Response] {
        assert!(preflight(b"\r\n{\r\n\t\"text\" : \"ok\"\r\n}\r\n", kind).is_ok());
        assert_eq!(
            preflight("\u{feff}{}".as_bytes(), kind),
            Err(kind.invalid())
        );
        assert!(preflight("{\"text\":\"\u{feff}café\\r\\n\"}".as_bytes(), kind).is_ok());
    }
}
