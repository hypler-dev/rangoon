use super::*;

const ATTEMPT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const CREATOR: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const DESTINATION: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const OTHER_DESTINATION: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const VERIFIER: &str = "5555555555555555555555555555555555555555555555555555555555555555";
const OTHER_VERIFIER: &str = "6666666666666666666666666666666666666666666666666666666666666666";
const PREVIOUS: &str = "7777777777777777777777777777777777777777777777777777777777777777";
const SOURCE_IDENTITY: &str = "8888888888888888888888888888888888888888888888888888888888888888";
const SOURCE_ARCHIVE: &str = "9999999999999999999999999999999999999999999999999999999999999999";

fn record_json(phase: &str, source_schema: u32) -> String {
    let target_schema = source_schema.max(1);
    let (previous_digest, destination) = if phase == "intent" {
        ("null".to_owned(), "null".to_owned())
    } else {
        (format!("\"{PREVIOUS}\""), format!("\"{DESTINATION}\""))
    };
    let verifier = match phase {
        "reopened" | "selected" => format!(",\"verifierProcess\":\"{VERIFIER}\""),
        _ => String::new(),
    };
    format!(
        r#"{{"schemaVersion":"rangoon.workspace-adoption.v1","phase":"{phase}","attemptId":"{ATTEMPT}","creatorProcess":"{CREATOR}","previousDigest":{previous_digest},"sourceIdentity":"{SOURCE_IDENTITY}","sourceArchive":"{SOURCE_ARCHIVE}","sourceSchema":{source_schema},"targetSchema":{target_schema},"inventory":{{"sources":3,"capabilities":4,"revisions":5,"reviews":6,"recipes":7,"applications":8,"derivations":9,"workflows":10,"workflowRevisions":11}},"destination":{destination}{verifier}}}"#
    )
}

fn decoded(phase: &str, source_schema: u32) -> Record {
    Record::decode(record_json(phase, source_schema).as_bytes()).expect("valid literal fixture")
}

fn with_root_field(mut input: String, field: &str, value: &str) -> String {
    assert_eq!(input.pop(), Some('}'), "fixture object closes");
    format!("{input},\"{field}\":{value}}}")
}

fn refuses(bytes: &[u8], case: &str) {
    assert!(
        matches!(Record::decode(bytes), Err(Error::RecoveryRequired)),
        "accepted {case}"
    );
}

#[test]
fn codec_accepts_every_phase_and_supported_source_schema() {
    let phases = [
        ("intent", Phase::Intent, false),
        ("target", Phase::Target, false),
        ("copied", Phase::Copied, false),
        ("reopened", Phase::Reopened, true),
        ("selected", Phase::Selected, true),
    ];

    for (name, phase, verified) in phases {
        for source_schema in 0..=4 {
            let record = decoded(name, source_schema);
            assert_eq!(record.phase, phase, "{name} schema {source_schema}");
            assert_eq!(record.source_schema, source_schema);
            assert_eq!(record.target_schema, source_schema.max(1));
            assert_eq!(record.previous_digest.is_none(), phase == Phase::Intent);
            assert_eq!(record.destination.is_none(), phase == Phase::Intent);
            assert_eq!(record.verifier_process.is_some(), verified);

            let encoded = record.encode().expect("encode validated fixture");
            assert_eq!(
                Record::decode(&encoded).expect("decode encoded fixture"),
                record,
                "{name} schema {source_schema}"
            );
        }
    }
}

#[test]
fn codec_refuses_malformed_extra_and_oversized_json() {
    refuses(b"", "empty input");
    refuses(b"{", "malformed object");

    let duplicate = record_json("intent", 0).replacen(
        "\"phase\":\"intent\"",
        "\"phase\":\"intent\",\"phase\":\"target\"",
        1,
    );
    refuses(duplicate.as_bytes(), "duplicate field");

    let unknown = with_root_field(record_json("intent", 0), "untrusted", "true");
    refuses(unknown.as_bytes(), "unknown field");

    let mut extra = record_json("intent", 0).into_bytes();
    extra.extend_from_slice(b"{}");
    refuses(&extra, "extra JSON value");

    let oversized = vec![b' '; LIMIT + 1];
    refuses(&oversized, "over-limit input");
}

#[test]
fn codec_refuses_missing_or_untyped_inventory_fields() {
    let base = record_json("reopened", 4);
    let required = [
        "schemaVersion",
        "phase",
        "attemptId",
        "creatorProcess",
        "previousDigest",
        "sourceIdentity",
        "sourceArchive",
        "sourceSchema",
        "targetSchema",
        "inventory",
        "destination",
        "verifierProcess",
    ];
    for field in required {
        let mut value: serde_json::Value = serde_json::from_str(&base).expect("fixture JSON");
        value.as_object_mut().expect("fixture object").remove(field);
        let bytes = serde_json::to_vec(&value).expect("fixture bytes");
        refuses(&bytes, &format!("missing {field}"));
    }

    for field in [
        "sources",
        "capabilities",
        "revisions",
        "reviews",
        "recipes",
        "applications",
        "derivations",
        "workflows",
        "workflowRevisions",
    ] {
        let mut value: serde_json::Value = serde_json::from_str(&base).expect("fixture JSON");
        value
            .get_mut("inventory")
            .and_then(serde_json::Value::as_object_mut)
            .expect("fixture inventory")
            .remove(field);
        let bytes = serde_json::to_vec(&value).expect("fixture bytes");
        refuses(&bytes, &format!("missing inventory {field}"));
    }

    let overflow = base.replacen("\"sources\":3", "\"sources\":4294967296", 1);
    refuses(overflow.as_bytes(), "u32 inventory overflow");

    let inventory_unknown = base.replacen(
        "\"workflowRevisions\":11}",
        "\"workflowRevisions\":11,\"surplus\":12}",
        1,
    );
    refuses(inventory_unknown.as_bytes(), "unknown inventory field");
}

#[test]
fn codec_refuses_noncanonical_identifiers_and_schema_mappings() {
    let zero = "0".repeat(64);
    let uppercase = "A".repeat(64);

    let zero_attempt = record_json("intent", 1).replacen(ATTEMPT, &zero, 1);
    refuses(zero_attempt.as_bytes(), "zero attempt identifier");

    let zero_creator = record_json("intent", 1).replacen(CREATOR, &zero, 1);
    refuses(zero_creator.as_bytes(), "zero creator identifier");

    let zero_destination = record_json("target", 1).replacen(DESTINATION, &zero, 1);
    refuses(zero_destination.as_bytes(), "zero destination identifier");

    let uppercase_identity = record_json("intent", 1).replacen(SOURCE_IDENTITY, &uppercase, 1);
    refuses(uppercase_identity.as_bytes(), "uppercase source identity");

    let short_archive = record_json("intent", 1).replacen(SOURCE_ARCHIVE, "abc", 1);
    refuses(short_archive.as_bytes(), "short source archive");

    let uppercase_previous = record_json("target", 1).replacen(PREVIOUS, &uppercase, 1);
    refuses(uppercase_previous.as_bytes(), "uppercase previous digest");

    let zero_verifier = record_json("reopened", 1).replacen(VERIFIER, &zero, 1);
    refuses(zero_verifier.as_bytes(), "zero verifier identifier");

    let unknown_phase =
        record_json("intent", 1).replacen("\"phase\":\"intent\"", "\"phase\":\"unknown\"", 1);
    refuses(unknown_phase.as_bytes(), "unknown phase");

    let unsupported_source = record_json("target", 4)
        .replacen("\"sourceSchema\":4", "\"sourceSchema\":5", 1)
        .replacen("\"targetSchema\":4", "\"targetSchema\":5", 1);
    refuses(unsupported_source.as_bytes(), "unsupported source schema");

    let zero_target =
        record_json("target", 0).replacen("\"targetSchema\":1", "\"targetSchema\":0", 1);
    refuses(zero_target.as_bytes(), "schema zero target must be one");

    let wrong_target =
        record_json("target", 2).replacen("\"targetSchema\":2", "\"targetSchema\":1", 1);
    refuses(wrong_target.as_bytes(), "source and target schema mismatch");
}

#[test]
fn codec_refuses_wrong_phase_fields_and_verifiers() {
    let intent_previous = record_json("intent", 1).replacen(
        "\"previousDigest\":null",
        &format!("\"previousDigest\":\"{PREVIOUS}\""),
        1,
    );
    refuses(intent_previous.as_bytes(), "intent previous digest");

    let intent_destination = record_json("intent", 1).replacen(
        "\"destination\":null",
        &format!("\"destination\":\"{DESTINATION}\""),
        1,
    );
    refuses(intent_destination.as_bytes(), "intent destination");

    for phase in ["target", "copied", "reopened", "selected"] {
        let no_previous = record_json(phase, 1).replacen(
            &format!("\"previousDigest\":\"{PREVIOUS}\""),
            "\"previousDigest\":null",
            1,
        );
        refuses(
            no_previous.as_bytes(),
            &format!("{phase} missing previous digest"),
        );

        let no_destination = record_json(phase, 1).replacen(
            &format!("\"destination\":\"{DESTINATION}\""),
            "\"destination\":null",
            1,
        );
        refuses(
            no_destination.as_bytes(),
            &format!("{phase} missing destination"),
        );
    }

    for phase in ["intent", "target", "copied"] {
        let first_phase_verifier = with_root_field(
            record_json(phase, 1),
            "verifierProcess",
            &format!("\"{VERIFIER}\""),
        );
        refuses(
            first_phase_verifier.as_bytes(),
            &format!("{phase} verifier present"),
        );
    }

    for phase in ["reopened", "selected"] {
        let no_verifier =
            record_json(phase, 1).replacen(&format!(",\"verifierProcess\":\"{VERIFIER}\""), "", 1);
        refuses(no_verifier.as_bytes(), &format!("{phase} verifier absent"));

        let creator_verifier = record_json(phase, 1).replacen(VERIFIER, CREATOR, 1);
        refuses(
            creator_verifier.as_bytes(),
            &format!("{phase} verifier is creator"),
        );
    }
}

#[test]
fn same_binding_refuses_changed_common_destination_or_selected_verifier() {
    let intent = decoded("intent", 3);
    let target = decoded("target", 3);
    assert!(
        target.same_binding(&intent),
        "intent permits first destination"
    );

    let changed_common = Record::decode(
        record_json("target", 3)
            .replacen(SOURCE_ARCHIVE, OTHER_VERIFIER, 1)
            .as_bytes(),
    )
    .expect("valid changed common fixture");
    assert!(!changed_common.same_binding(&intent));

    let copied_other_destination = Record::decode(
        record_json("copied", 3)
            .replacen(DESTINATION, OTHER_DESTINATION, 1)
            .as_bytes(),
    )
    .expect("valid changed destination fixture");
    assert!(!copied_other_destination.same_binding(&target));

    let reopened = decoded("reopened", 3);
    let selected_other_verifier = Record::decode(
        record_json("selected", 3)
            .replacen(VERIFIER, OTHER_VERIFIER, 1)
            .as_bytes(),
    )
    .expect("valid changed verifier fixture");
    assert!(!selected_other_verifier.same_binding(&reopened));
}

#[test]
fn debug_is_fixed_redaction_for_synthetic_marker() {
    let mut record = decoded("intent", 1);
    record.source_archive = "synthetic-marker-must-not-log".to_owned();

    let debug = format!("{record:?}");
    assert_eq!(debug, "AdoptionRecord(<redacted>)");
    assert!(!debug.contains("synthetic-marker-must-not-log"));
}
