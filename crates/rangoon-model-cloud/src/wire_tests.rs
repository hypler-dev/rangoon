use super::*;

fn valid(content: &str) -> String {
    format!(
        r#"{{"object":"response","id":"resp_123","model":"gpt-test","status":"completed","error":null,"incomplete_details":null,"output":[{{"id":"msg_123","type":"message","status":"completed","role":"assistant","content":[{{"type":"output_text","text":"{content}","annotations":[]}}]}}]}}"#
    )
}

#[test]
fn decodes_completed_response_and_camel_case_usage() {
    let raw = valid("keep \\ud83d\\ude00 exact").replace(
        "\"output\":[",
        "\"usage\":{\"input_tokens\":3,\"input_tokens_details\":{\"cached_tokens\":1,\"cache_write_tokens\":2},\"output_tokens\":2,\"output_tokens_details\":{\"reasoning_tokens\":2},\"total_tokens\":5},\"output\":[",
    );
    let decoded = response(raw.as_bytes(), 2).unwrap();
    assert_eq!(decoded.id, "resp_123");
    assert_eq!(decoded.model, "gpt-test");
    assert_eq!(decoded.content, "keep 😀 exact");
    assert_eq!(
        serde_json::to_string(&decoded.usage).unwrap(),
        r#"{"inputTokens":3,"outputTokens":2,"totalTokens":5,"cachedInputTokens":1,"cacheWriteInputTokens":2,"reasoningOutputTokens":2}"#
    );
}

#[test]
fn permits_inert_reasoning_and_ignored_top_metadata() {
    let raw = valid("ok").replace(
        "\"output\":[",
        "\"provider_metadata\":{\"trace\":true},\"output\":[{\"id\":\"rs_123\",\"type\":\"reasoning\",\"summary\":[],\"status\":\"completed\"},",
    );
    assert_eq!(response(raw.as_bytes(), 1).unwrap().content, "ok");
}

#[test]
fn rejects_incomplete_refusal_and_output_expansion() {
    let mut tool: Value = serde_json::from_str(&valid("ok")).unwrap();
    tool["output"] = serde_json::json!([{
        "type": "function_call", "id": "fc_synthetic", "call_id": "call_synthetic",
        "name": "execute", "arguments": "{}", "status": "completed"
    }]);
    assert_eq!(
        response(&serde_json::to_vec(&tool).unwrap(), 1),
        Err(Diagnostic::ResponseInvalid)
    );
    let mut multiple: Value = serde_json::from_str(&valid("ok")).unwrap();
    let message = multiple["output"][0].clone();
    multiple["output"].as_array_mut().unwrap().push(message);
    assert_eq!(
        response(&serde_json::to_vec(&multiple).unwrap(), 1),
        Err(Diagnostic::ResponseInvalid)
    );

    assert_eq!(
        response(
            valid("ok")
                .replace("\"status\":\"completed\"", "\"status\":\"incomplete\"")
                .as_bytes(),
            1
        ),
        Err(Diagnostic::ResponseIncomplete)
    );
    let incomplete = valid("ok")
        .replacen("\"status\":\"completed\"", "\"status\":\"incomplete\"", 1)
        .replace(
            "\"incomplete_details\":null",
            "\"incomplete_details\":{\"reason\":\"max_output_tokens\"}",
        );
    assert_eq!(
        response(incomplete.as_bytes(), 1),
        Err(Diagnostic::ResponseIncomplete)
    );
    assert_eq!(
        response(
            valid("ok")
                .replace(
                    r#"{"type":"output_text","text":"ok","annotations":[]}"#,
                    r#"{"type":"refusal","refusal":"declined"}"#,
                )
                .as_bytes(),
            1
        ),
        Err(Diagnostic::ResponseRefused)
    );
    assert_eq!(
        response(
            valid("ok")
                .replace("\"annotations\":[]", "\"annotations\":[{\"x\":1}]")
                .as_bytes(),
            1
        ),
        Err(Diagnostic::ResponseInvalid)
    );
}

#[test]
fn rejects_duplicate_keys_required_nulls_and_invalid_usage() {
    assert_eq!(
        response(br#"{"object":"response","\u006fbject":"response"}"#, 1),
        Err(Diagnostic::ResponseInvalid)
    );
    let bad_total = valid("ok").replace(
        "\"output\":[",
        "\"usage\":{\"input_tokens\":3,\"output_tokens\":2,\"total_tokens\":4},\"output\":[",
    );
    assert_eq!(
        response(bad_total.as_bytes(), 2),
        Err(Diagnostic::ResponseInvalid)
    );
    let missing_error = valid("ok").replace("\"error\":null,", "");
    assert_eq!(
        response(missing_error.as_bytes(), 1),
        Err(Diagnostic::ResponseInvalid)
    );
    let missing_incomplete = valid("ok").replace("\"incomplete_details\":null,", "");
    assert_eq!(
        response(missing_incomplete.as_bytes(), 1),
        Err(Diagnostic::ResponseInvalid)
    );
    let overflowing = valid("ok").replace(
        "\"output\":[",
        "\"usage\":{\"input_tokens\":18446744073709551615,\"output_tokens\":1,\"total_tokens\":0},\"output\":[",
    );
    assert_eq!(
        response(overflowing.as_bytes(), 1),
        Err(Diagnostic::ResponseInvalid)
    );
    let excessive_cached = valid("ok").replace(
        "\"output\":[",
        "\"usage\":{\"input_tokens\":1,\"input_tokens_details\":{\"cached_tokens\":2},\"output_tokens\":1,\"total_tokens\":2},\"output\":[",
    );
    assert_eq!(
        response(excessive_cached.as_bytes(), 1),
        Err(Diagnostic::ResponseInvalid)
    );
}

#[test]
fn preflight_enforces_exact_and_one_over_limits() {
    let exact = valid(&"x".repeat(MAX_CONTENT_BYTES));
    assert!(response(exact.as_bytes(), 1).is_ok());
    let excess = valid(&"x".repeat(MAX_CONTENT_BYTES + 1));
    assert_eq!(
        response(excess.as_bytes(), 1),
        Err(Diagnostic::ResponseTooLarge)
    );
    assert_eq!(
        preflight(&vec![b' '; super::super::json::RAW_LIMIT]),
        Err(Diagnostic::ResponseInvalid)
    );
    assert_eq!(
        preflight(&vec![b' '; super::super::json::RAW_LIMIT + 1]),
        Err(Diagnostic::ResponseTooLarge)
    );
}

#[test]
fn preflight_enforces_all_structural_and_numeric_bounds() {
    let exact_key = format!(r#"{{"{}":0}}"#, "k".repeat(128));
    assert!(preflight(exact_key.as_bytes()).is_ok());
    let excess_key = format!(r#"{{"{}":0}}"#, "k".repeat(129));
    assert_eq!(
        preflight(excess_key.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    let exact_string = format!(r#"{{"x":"{}"}}"#, "x".repeat(262_144));
    assert!(preflight(exact_string.as_bytes()).is_ok());
    let excess_string = format!(r#"{{"x":"{}"}}"#, "x".repeat(262_145));
    assert_eq!(
        preflight(excess_string.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    let exact_array = format!("[{}]", vec!["0"; 512].join(","));
    assert!(preflight(exact_array.as_bytes()).is_ok());
    let excess_array = format!("[{}]", vec!["0"; 513].join(","));
    assert_eq!(
        preflight(excess_array.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    let exact_object = format!(
        "{{{}}}",
        (0..64)
            .map(|index| format!("\"k{index}\":0"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(preflight(exact_object.as_bytes()).is_ok());
    let excess_object = format!(
        "{{{}}}",
        (0..65)
            .map(|index| format!("\"k{index}\":0"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        preflight(excess_object.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    let mut exact_depth = "{}".to_owned();
    for _ in 0..15 {
        exact_depth = format!(r#"{{"x":{exact_depth}}}"#);
    }
    assert!(preflight(exact_depth.as_bytes()).is_ok());
    let excess_depth = format!(r#"{{"x":{exact_depth}}}"#);
    assert_eq!(
        preflight(excess_depth.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    let value_limited = format!(
        "{{{}}}",
        (0..16)
            .map(|index| {
                let count = if index == 15 { 510 } else { 511 };
                format!("\"k{index}\":[{}]", vec!["0"; count].join(","))
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(preflight(value_limited.as_bytes()).is_ok());
    let values_excess = value_limited.replacen("0]}", "0,0]}", 1);
    assert_eq!(
        preflight(values_excess.as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );

    assert_eq!(preflight(br#"1e9999"#), Err(Diagnostic::ResponseInvalid));
    assert_eq!(
        preflight("1".repeat(129).as_bytes()),
        Err(Diagnostic::ResponseTooLarge)
    );
    assert_eq!(
        preflight(b"\"\xed\xa0\x80\""),
        Err(Diagnostic::ResponseInvalid)
    );
    assert_eq!(
        preflight(br#"\"\ud800\""#),
        Err(Diagnostic::ResponseInvalid)
    );
}
