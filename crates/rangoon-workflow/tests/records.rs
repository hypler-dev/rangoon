use rangoon_domain::byte_digest;
use rangoon_workflow::DEFINITION_SCHEMA;
use rangoon_workflow::records::{
    RecordError, SaveIntent, decode_revision, prepare_revision, workflow_id_from_nonce,
};
use serde_json::{Value, json};

const DEFINITION: &str = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Draft \"A\" 😀","nodes":[{"id":"start","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},{"id":"finish","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],"controlEdges":[{"fromNode":"start","outlet":"next","toNode":"finish"}],"dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"finish","toPort":"text"}]}"#;
const LAYOUT: &str =
    r#"{"positions":[{"nodeIndex":1,"x":100000,"y":42},{"nodeIndex":0,"x":-100000,"y":0}]}"#;

fn workflow_id() -> String {
    workflow_id_from_nonce(&core::array::from_fn(|index| index as u8))
}
fn invalid_definition() -> String {
    json!({"schemaVersion":DEFINITION_SCHEMA,"title":"Draft","nodes":[],"controlEdges":[],"dataEdges":[]}).to_string()
}

fn record_wire() -> Value {
    let record = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    serde_json::from_slice(&record.serialized_bytes().unwrap()).unwrap()
}

#[test]
fn independent_golden_record_and_nonce_identity_match_exact_bytes() {
    const GOLDEN: &str = r#"{"schemaVersion":"rangoon.workflow-revision.v1","id":"workflow-revision:c54954b55718aa53d3752c9d6d760082146782253397ca61b043d2198efa3650","workflowId":"workflow:5043666a7285dd946b911fa27b2a0abc3bdf56145c09f5508fe6418072dd87df","parentRevisionId":null,"intent":"draft","definition":{"schemaVersion":"rangoon.workflow-definition.v1","title":"Draft \"A\" 😀","nodes":[{"id":"start","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},{"id":"finish","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],"controlEdges":[{"fromNode":"start","outlet":"next","toNode":"finish"}],"dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"finish","toPort":"text"}]},"layout":{"positions":[{"nodeIndex":0,"x":-100000,"y":0},{"nodeIndex":1,"x":100000,"y":42}]}}"#;
    assert_eq!(
        workflow_id(),
        "workflow:5043666a7285dd946b911fa27b2a0abc3bdf56145c09f5508fe6418072dd87df"
    );
    let decoded = decode_revision(GOLDEN.as_bytes()).unwrap();
    assert_eq!(decoded.serialized_bytes().unwrap(), GOLDEN.as_bytes());
    assert_eq!(
        decoded.id(),
        "workflow-revision:c54954b55718aa53d3752c9d6d760082146782253397ca61b043d2198efa3650"
    );
    let prepared = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    assert_eq!(prepared.serialized_bytes().unwrap(), GOLDEN.as_bytes());

    let definition = prepared.inspection().canonical_definition_bytes().unwrap();
    let layout = prepared.layout().serialized_bytes().unwrap();
    let mut frame = b"rangoon.workflow-revision.v1\0".to_vec();
    for field in [
        workflow_id().as_bytes(),
        b"",
        b"draft",
        definition.as_slice(),
        layout.as_slice(),
    ] {
        frame.extend_from_slice(&(field.len() as u64).to_be_bytes());
        frame.extend_from_slice(field);
    }
    assert_eq!(
        prepared.id(),
        format!("workflow-revision:{}", byte_digest(&frame))
    );
}

#[test]
fn draft_round_trips_invalid_definition_but_validated_requires_structure() {
    let draft = invalid_definition();
    let record = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        draft.as_bytes(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    assert!(!record.inspection().report().structurally_valid);
    assert!(record.inspection().report().definition_id.is_none());
    assert_eq!(
        decode_revision(&record.serialized_bytes().unwrap())
            .unwrap()
            .intent(),
        SaveIntent::Draft
    );
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Validated,
            draft.as_bytes(),
            br#"{"positions":[]}"#
        )
        .unwrap_err(),
        RecordError::ValidatedDefinitionRequired
    );
}

#[test]
fn layout_order_and_input_key_order_do_not_change_revision_identity() {
    let first = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    let reordered_definition = format!(
        " \n {} \t",
        serde_json::to_string(&serde_json::from_str::<Value>(DEFINITION).unwrap()).unwrap()
    );
    let reordered_layout = br#" { "positions" : [ {"y":0,"x":-100000,"nodeIndex":0}, {"y":42,"nodeIndex":1,"x":100000} ] } "#;
    let second = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        reordered_layout,
    )
    .unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(
        first.id(),
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            reordered_definition.as_bytes(),
            reordered_layout,
        )
        .unwrap()
        .id()
    );
    assert_eq!(first.layout().positions()[0].node_index, 0);
    let parent = format!("workflow-revision:{}", "a".repeat(64));
    assert_ne!(
        first.id(),
        prepare_revision(
            &workflow_id(),
            Some(&parent),
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            LAYOUT.as_bytes()
        )
        .unwrap()
        .id()
    );
    assert_ne!(
        first.id(),
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Validated,
            DEFINITION.as_bytes(),
            LAYOUT.as_bytes()
        )
        .unwrap()
        .id()
    );
    let changed_layout = br#"{"positions":[{"nodeIndex":0,"x":0,"y":0}]}"#;
    assert_ne!(
        first.id(),
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            changed_layout
        )
        .unwrap()
        .id()
    );
}

#[test]
fn record_and_layout_wire_errors_are_closed_and_do_not_repair_input() {
    let valid = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    let mut value: Value = serde_json::from_slice(&valid.serialized_bytes().unwrap()).unwrap();
    value["id"] = json!("workflow-revision:bad");
    assert_eq!(
        decode_revision(serde_json::to_vec(&value).unwrap().as_slice()).unwrap_err(),
        RecordError::InvalidRevisionId
    );
    value = serde_json::from_slice(&valid.serialized_bytes().unwrap()).unwrap();
    value["id"] = json!(format!("workflow-revision:{}", "a".repeat(64)));
    assert_eq!(
        decode_revision(serde_json::to_vec(&value).unwrap().as_slice()).unwrap_err(),
        RecordError::IdentityMismatch
    );
    assert_eq!(
        decode_revision(b"{} trailing").unwrap_err(),
        RecordError::InvalidJson
    );
    assert_eq!(
        decode_revision(&[0xff]).unwrap_err(),
        RecordError::InvalidEncoding
    );
    let duplicate = r#"{"schemaVersion":"rangoon.workflow-revision.v1","schemaVersion":"rangoon.workflow-revision.v1"}"#;
    assert_eq!(
        decode_revision(duplicate.as_bytes()).unwrap_err(),
        RecordError::InvalidJson
    );
    for (path, malformed) in [
        ("intent", json!("save")),
        ("operation", json!({"kind":"unknown"})),
        ("coordinate", json!("zero")),
    ] {
        let mut malformed_record = record_wire();
        match path {
            "intent" => malformed_record["intent"] = malformed,
            "operation" => malformed_record["definition"]["nodes"][0]["operation"] = malformed,
            "coordinate" => malformed_record["layout"]["positions"][0]["x"] = malformed,
            _ => unreachable!(),
        }
        assert_eq!(
            decode_revision(&serde_json::to_vec(&malformed_record).unwrap()).unwrap_err(),
            RecordError::InvalidJson,
            "{path}"
        );
    }
    let mut unknown_record_field = record_wire();
    unknown_record_field["extra"] = json!(true);
    assert_eq!(
        decode_revision(&serde_json::to_vec(&unknown_record_field).unwrap()).unwrap_err(),
        RecordError::InvalidJson
    );
    assert_eq!(
        prepare_revision(
            "workflow:bad",
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            LAYOUT.as_bytes()
        )
        .unwrap_err(),
        RecordError::InvalidWorkflowId
    );
    for layout in [
        br#"{"positions":[{"nodeIndex":2,"x":0,"y":0}]}"#.as_slice(),
        br#"{"positions":[{"nodeIndex":0,"x":100001,"y":0}]}"#.as_slice(),
        br#"{"positions":[{"nodeIndex":0,"x":0,"y":0},{"nodeIndex":0,"x":1,"y":1}]}"#.as_slice(),
    ] {
        assert_eq!(
            prepare_revision(
                &workflow_id(),
                None,
                SaveIntent::Draft,
                DEFINITION.as_bytes(),
                layout
            )
            .unwrap_err(),
            RecordError::LayoutInvalid
        );
    }
}

#[test]
fn record_limits_and_parent_nullable_requirement_are_enforced() {
    assert_eq!(
        decode_revision(&vec![b' '; 160 * 1024 + 1]).unwrap_err(),
        RecordError::InputLimit
    );
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            &vec![b' '; 16 * 1024 + 1]
        )
        .unwrap_err(),
        RecordError::InputLimit
    );
    let valid = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    let compact = String::from_utf8(valid.serialized_bytes().unwrap()).unwrap();
    let padded_record = compact.replacen(
        "\"layout\":{",
        &format!("\"layout\":{}{{", " ".repeat(16 * 1024 + 1)),
        1,
    );
    assert!(padded_record.len() <= 160 * 1024);
    assert_eq!(
        decode_revision(padded_record.as_bytes()).unwrap().id(),
        valid.id()
    );
    let missing_parent = format!(
        r#"{{"schemaVersion":"rangoon.workflow-revision.v1","id":"workflow-revision:{}","workflowId":"{}","intent":"draft","definition":{},"layout":{{"positions":[]}}}}"#,
        "a".repeat(64),
        workflow_id(),
        DEFINITION
    );
    assert_eq!(
        decode_revision(missing_parent.as_bytes()).unwrap_err(),
        RecordError::InvalidJson
    );
    let at_limit = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    assert_eq!(
        decode_revision(at_limit.as_bytes()).unwrap_err(),
        RecordError::InvalidJson
    );
    let deep = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert_eq!(
        decode_revision(deep.as_bytes()).unwrap_err(),
        RecordError::DepthLimit
    );
}

#[test]
fn exact_raw_limits_preserve_canonical_record_and_identity() {
    let base = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();

    let mut definition = DEFINITION.as_bytes().to_vec();
    definition.resize(128 * 1024, b' ');
    let from_definition_limit = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        &definition,
        LAYOUT.as_bytes(),
    )
    .unwrap();
    assert_eq!(from_definition_limit.id(), base.id());
    assert_eq!(
        from_definition_limit
            .inspection()
            .canonical_definition_bytes()
            .unwrap(),
        base.inspection().canonical_definition_bytes().unwrap()
    );

    let mut layout = LAYOUT.as_bytes().to_vec();
    layout.resize(16 * 1024, b' ');
    let from_layout_limit = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        &layout,
    )
    .unwrap();
    assert_eq!(from_layout_limit.id(), base.id());
    assert_eq!(
        from_layout_limit.layout().serialized_bytes().unwrap(),
        base.layout().serialized_bytes().unwrap()
    );

    let canonical = base.serialized_bytes().unwrap();
    let mut record = canonical.clone();
    record.resize(160 * 1024, b' ');
    let decoded = decode_revision(&record).unwrap();
    assert_eq!(decoded.id(), base.id());
    assert_eq!(decoded.serialized_bytes().unwrap(), canonical);
}

#[test]
fn identities_bind_workflow_content_node_order_parent_and_intent() {
    let base = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    let other_workflow = workflow_id_from_nonce(&[9; 32]);
    assert_ne!(
        base.id(),
        prepare_revision(
            &other_workflow,
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            LAYOUT.as_bytes()
        )
        .unwrap()
        .id()
    );
    let mut changed: Value = serde_json::from_str(DEFINITION).unwrap();
    changed["title"] = json!("Draft A😀");
    assert_ne!(
        base.id(),
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            serde_json::to_vec(&changed).unwrap().as_slice(),
            LAYOUT.as_bytes()
        )
        .unwrap()
        .id()
    );
    let mut reordered: Value = serde_json::from_str(DEFINITION).unwrap();
    reordered["nodes"].as_array_mut().unwrap().swap(0, 1);
    assert_ne!(
        base.id(),
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            serde_json::to_vec(&reordered).unwrap().as_slice(),
            LAYOUT.as_bytes()
        )
        .unwrap()
        .id()
    );
    let parent = format!("workflow-revision:{}", "c".repeat(64));
    let child = prepare_revision(
        &workflow_id(),
        Some(&parent),
        SaveIntent::Validated,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    assert_eq!(child.parent_revision_id(), Some(parent.as_str()));
    assert_eq!(
        decode_revision(&child.serialized_bytes().unwrap())
            .unwrap()
            .parent_revision_id(),
        Some(parent.as_str())
    );
    assert_eq!(
        serde_json::to_value(child.inspection().report()).unwrap()["referenceStatus"],
        "unverified"
    );
    assert_eq!(
        serde_json::to_value(child.inspection().report()).unwrap()["executionStatus"],
        "unavailable"
    );
    assert_eq!(
        serde_json::to_value(child.inspection().report()).unwrap()["authority"],
        "none"
    );
}

#[test]
fn layout_coordinate_and_typed_integer_boundaries_are_safe() {
    for (x, y) in [(-100000, 100000), (100000, -100000)] {
        let layout = json!({"positions":[{"nodeIndex":0,"x":x,"y":y}]}).to_string();
        let record = prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            layout.as_bytes(),
        )
        .unwrap();
        assert!(decode_revision(&record.serialized_bytes().unwrap()).is_ok());
    }
    for (x, y) in [(100001i64, 0), (-100001, 0), (i64::MIN, 0), (0, i64::MAX)] {
        let layout = json!({"positions":[{"nodeIndex":0,"x":x,"y":y}]}).to_string();
        assert_eq!(
            prepare_revision(
                &workflow_id(),
                None,
                SaveIntent::Draft,
                DEFINITION.as_bytes(),
                layout.as_bytes()
            )
            .unwrap_err(),
            RecordError::LayoutInvalid
        );
        let mut record = record_wire();
        record["layout"]["positions"] = json!([{"nodeIndex":0,"x":x,"y":y}]);
        assert_eq!(
            decode_revision(&serde_json::to_vec(&record).unwrap()).unwrap_err(),
            RecordError::LayoutInvalid
        );
    }
    for bad_index in ["-1", "1.5", "4294967296"] {
        let layout = format!(r#"{{"positions":[{{"nodeIndex":{bad_index},"x":0,"y":0}}]}}"#);
        assert_eq!(
            prepare_revision(
                &workflow_id(),
                None,
                SaveIntent::Draft,
                DEFINITION.as_bytes(),
                layout.as_bytes()
            )
            .unwrap_err(),
            RecordError::InvalidJson
        );
    }
    for bad_index in [json!(-1), json!(1.5), json!(4_294_967_296u64)] {
        let mut record = record_wire();
        record["layout"]["positions"][0]["nodeIndex"] = bad_index;
        assert_eq!(
            decode_revision(&serde_json::to_vec(&record).unwrap()).unwrap_err(),
            RecordError::InvalidJson
        );
    }
}

#[test]
fn layout_count_and_nested_hostile_wire_cases_are_closed() {
    let mut nodes = Vec::new();
    let mut positions = Vec::new();
    for index in 0..128u32 {
        nodes.push(json!({"id":format!("n{index}"),"title":"Node","operation":{"kind":"checkpoint","prompt":"P"},"inputs":[],"outputs":[]}));
        positions.push(json!({"nodeIndex":index,"x":0,"y":0}));
    }
    let draft = json!({"schemaVersion":DEFINITION_SCHEMA,"title":"Draft","nodes":nodes,"controlEdges":[],"dataEdges":[]});
    let mut layout = json!({"positions":positions});
    assert!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            serde_json::to_vec(&draft).unwrap().as_slice(),
            serde_json::to_vec(&layout).unwrap().as_slice()
        )
        .is_ok()
    );
    layout["positions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"nodeIndex":0,"x":0,"y":0}));
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            serde_json::to_vec(&draft).unwrap().as_slice(),
            serde_json::to_vec(&layout).unwrap().as_slice()
        )
        .unwrap_err(),
        RecordError::LayoutInvalid
    );

    let record = prepare_revision(
        &workflow_id(),
        None,
        SaveIntent::Draft,
        DEFINITION.as_bytes(),
        LAYOUT.as_bytes(),
    )
    .unwrap();
    let mut nested_duplicate = String::from_utf8(record.serialized_bytes().unwrap()).unwrap();
    nested_duplicate = nested_duplicate.replacen(
        "\"positions\":[",
        "\"positions\":[],\"posit\\u0069ons\":[",
        1,
    );
    assert_eq!(
        decode_revision(nested_duplicate.as_bytes()).unwrap_err(),
        RecordError::InvalidJson
    );
    let mut unknown: Value = serde_json::from_str(DEFINITION).unwrap();
    unknown["nodes"][0]["operation"]["extra"] = json!(true);
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            serde_json::to_vec(&unknown).unwrap().as_slice(),
            LAYOUT.as_bytes()
        )
        .unwrap_err(),
        RecordError::DefinitionInvalid
    );
    let mut wrong_schema = record_wire();
    wrong_schema["schemaVersion"] = json!("other");
    assert_eq!(
        decode_revision(&serde_json::to_vec(&wrong_schema).unwrap()).unwrap_err(),
        RecordError::UnsupportedSchema
    );
}

#[test]
fn decode_classifies_nested_resource_limits_after_typed_record_parsing() {
    let node = json!({
        "id":"checkpoint",
        "title":"Checkpoint",
        "operation":{"kind":"checkpoint","prompt":"P"},
        "inputs":[],
        "outputs":[]
    });
    let cases = [
        ("nodes", json!(vec![node.clone(); 129])),
        (
            "ports",
            json!(vec![json!({"name":"text","dataType":"text"}); 9]),
        ),
        (
            "control",
            json!(vec![
                json!({"fromNode":"start","outlet":"next","toNode":"finish"});
                257
            ]),
        ),
        (
            "data",
            json!(vec![
                json!({"fromNode":"start","fromPort":"text","toNode":"finish","toPort":"text"});
                1025
            ]),
        ),
    ];
    for (kind, values) in cases {
        let mut record = record_wire();
        match kind {
            "nodes" => record["definition"]["nodes"] = values,
            "ports" => record["definition"]["nodes"][0]["outputs"] = values,
            "control" => record["definition"]["controlEdges"] = values,
            "data" => record["definition"]["dataEdges"] = values,
            _ => unreachable!(),
        }
        assert_eq!(
            decode_revision(&serde_json::to_vec(&record).unwrap()).unwrap_err(),
            RecordError::DefinitionInvalid,
            "{kind}"
        );
    }

    for count in [129usize, 1025] {
        let mut record = record_wire();
        record["layout"]["positions"] = json!(
            (0..count)
                .map(|_| json!({"nodeIndex":0,"x":0,"y":0}))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            decode_revision(&serde_json::to_vec(&record).unwrap()).unwrap_err(),
            RecordError::LayoutInvalid,
            "positions {count}"
        );
    }
}

#[test]
fn raw_definition_and_layout_depth_bounds_are_classified_without_panics() {
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            &vec![b' '; 128 * 1024 + 1],
            br#"{"positions":[]}"#,
        )
        .unwrap_err(),
        RecordError::DefinitionInvalid
    );
    let at_limit = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            at_limit.as_bytes(),
        )
        .unwrap_err(),
        RecordError::InvalidJson
    );
    let too_deep = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert_eq!(
        prepare_revision(
            &workflow_id(),
            None,
            SaveIntent::Draft,
            DEFINITION.as_bytes(),
            too_deep.as_bytes(),
        )
        .unwrap_err(),
        RecordError::DepthLimit
    );
}
