use rangoon_domain::byte_digest;
use rangoon_workflow::{DEFINITION_SCHEMA, DiagnosticCode, WorkflowError, inspect_definition};
use serde_json::{Value, json};

fn capability_id() -> String {
    format!("capability:{}", "a".repeat(64))
}
fn revision_id() -> String {
    format!("revision:{}", "b".repeat(64))
}

fn simple() -> Value {
    json!({
        "schemaVersion": DEFINITION_SCHEMA,
        "title": "Simple workflow",
        "nodes": [
            {"id":"source","title":"Source","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":capability_id(),"revisionId":revision_id()},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"result","dataType":"text"}]},
            {"id":"sink","title":"Sink","operation":{"kind":"output"},"inputs":[{"name":"result","dataType":"text"}],"outputs":[]}
        ],
        "controlEdges": [
            {"fromNode":"source","outlet":"next","toNode":"work"},
            {"fromNode":"work","outlet":"next","toNode":"sink"}
        ],
        "dataEdges": [
            {"fromNode":"source","fromPort":"text","toNode":"work","toPort":"text"},
            {"fromNode":"work","fromPort":"result","toNode":"sink","toPort":"result"}
        ]
    })
}

fn inspect(value: &Value) -> rangoon_workflow::DefinitionInspection {
    inspect_definition(serde_json::to_vec(value).unwrap().as_slice()).unwrap()
}
fn codes(value: &Value) -> Vec<DiagnosticCode> {
    inspect(value)
        .report()
        .diagnostics
        .iter()
        .map(|item| item.code)
        .collect()
}

#[test]
fn valid_definition_has_pinned_identity_dependency_and_inert_status() {
    let value = simple();
    let inspection = inspect(&value);
    let report = inspection.report();
    assert!(report.structurally_valid);
    assert_eq!(report.topological_order, ["source", "work", "sink"]);
    assert_eq!(report.dependencies.len(), 1);
    assert_eq!(
        serde_json::to_value(report).unwrap()["referenceStatus"],
        "unverified"
    );
    assert_eq!(
        serde_json::to_value(report).unwrap()["executionStatus"],
        "unavailable"
    );
    assert_eq!(serde_json::to_value(report).unwrap()["authority"], "none");

    let canonical = inspection.canonical_definition_bytes().unwrap();
    assert!(canonical.starts_with(b"{\"schemaVersion\":"));
    let mut framed = b"rangoon.workflow-definition.v1\0".to_vec();
    framed.extend_from_slice(&(canonical.len() as u64).to_be_bytes());
    framed.extend_from_slice(&canonical);
    let expected_id = format!("workflow-definition:{}", byte_digest(&framed));
    assert_eq!(report.definition_id.as_deref(), Some(expected_id.as_str()));
    assert_eq!(
        serde_json::to_value(&inspection)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["definition", "report"]
    );
}

#[test]
fn whitespace_and_key_order_do_not_change_identity_but_content_changes_do() {
    let compact = serde_json::to_string(&simple()).unwrap();
    let reordered = format!(
        " {{ \n \"dataEdges\":{}, \"title\":\"Simple workflow\", \"nodes\":{}, \"schemaVersion\":\"{}\", \"controlEdges\":{} }} ",
        simple()["dataEdges"],
        simple()["nodes"],
        DEFINITION_SCHEMA,
        simple()["controlEdges"]
    );
    let left = inspect_definition(compact.as_bytes()).unwrap();
    let right = inspect_definition(reordered.as_bytes()).unwrap();
    assert_eq!(left.report().definition_id, right.report().definition_id);
    let nested_reordered = format!(
        r#"{{"dataEdges":[{{"toPort":"text","toNode":"work","fromPort":"text","fromNode":"source"}},{{"toPort":"result","toNode":"sink","fromPort":"result","fromNode":"work"}}],"controlEdges":[{{"toNode":"work","outlet":"next","fromNode":"source"}},{{"toNode":"sink","outlet":"next","fromNode":"work"}}],"nodes":[{{"outputs":[{{"dataType":"text","name":"text"}}],"inputs":[],"operation":{{"kind":"input"}},"title":"Source","id":"source"}},{{"outputs":[{{"dataType":"text","name":"result"}}],"inputs":[{{"dataType":"text","name":"text"}}],"operation":{{"revisionId":"{}","capabilityId":"{}","kind":"capability"}},"title":"Work","id":"work"}},{{"outputs":[],"inputs":[{{"dataType":"text","name":"result"}}],"operation":{{"kind":"output"}},"title":"Sink","id":"sink"}}],"title":"Simple workflow","schemaVersion":"{}"}}"#,
        revision_id(),
        capability_id(),
        DEFINITION_SCHEMA,
    );
    assert_eq!(
        left.report().definition_id,
        inspect_definition(nested_reordered.as_bytes())
            .unwrap()
            .report()
            .definition_id
    );
    let mut changed = simple();
    changed["title"] = json!("Changed workflow");
    assert_ne!(
        left.report().definition_id,
        inspect(&changed).report().definition_id
    );
    changed = simple();
    changed["controlEdges"].as_array_mut().unwrap().swap(0, 1);
    assert_ne!(
        left.report().definition_id,
        inspect(&changed).report().definition_id
    );
    changed = simple();
    changed["nodes"][1]["title"] = json!("Renamed work");
    assert_ne!(
        left.report().definition_id,
        inspect(&changed).report().definition_id
    );
    changed = simple();
    changed["nodes"][1]["operation"]["revisionId"] = json!(format!("revision:{}", "c".repeat(64)));
    assert_ne!(
        left.report().definition_id,
        inspect(&changed).report().definition_id
    );
}

#[test]
fn canonical_bytes_match_the_independent_escape_and_digest_fixture() {
    const GOLDEN: &str = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Review 😀","nodes":[{"id":"start","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"source","dataType":"text"}]},{"id":"review","title":"Checkpoint","operation":{"kind":"checkpoint","prompt":"Review \"quoted\" / \\ path\n😀 \u0001"},"inputs":[],"outputs":[]},{"id":"finish","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"result","dataType":"text"}],"outputs":[]}],"controlEdges":[{"fromNode":"start","outlet":"next","toNode":"review"},{"fromNode":"review","outlet":"next","toNode":"finish"}],"dataEdges":[{"fromNode":"start","fromPort":"source","toNode":"finish","toPort":"result"}]}"#;
    let inspection = inspect_definition(GOLDEN.as_bytes()).unwrap();
    assert_eq!(
        inspection.canonical_definition_bytes().unwrap(),
        GOLDEN.as_bytes()
    );
    assert_eq!(
        inspection.report().definition_id.as_deref(),
        Some(
            "workflow-definition:72a9c7a46884d4f5b305ea0831b5e6fdc8a55ed7f4bba130bdbd3015c8bcddc8"
        )
    );
}

#[test]
fn branch_checkpoint_and_dominance_are_structural_not_execution() {
    let workflow = json!({
        "schemaVersion": DEFINITION_SCHEMA, "title":"Branch", "nodes":[
            {"id":"source","title":"Source","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"condition","dataType":"boolean"},{"name":"text","dataType":"text"}]},
            {"id":"branch","title":"Branch","operation":{"kind":"branch"},"inputs":[{"name":"condition","dataType":"boolean"}],"outputs":[]},
            {"id":"yes","title":"Yes","operation":{"kind":"checkpoint","prompt":"Confirm route"},"inputs":[],"outputs":[]},
            {"id":"no","title":"No","operation":{"kind":"checkpoint","prompt":"Review route"},"inputs":[],"outputs":[]},
            {"id":"sink","title":"Sink","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}
        ], "controlEdges":[
            {"fromNode":"source","outlet":"next","toNode":"branch"}, {"fromNode":"branch","outlet":"true","toNode":"yes"}, {"fromNode":"branch","outlet":"false","toNode":"no"},
            {"fromNode":"yes","outlet":"next","toNode":"sink"}, {"fromNode":"no","outlet":"next","toNode":"sink"}
        ], "dataEdges":[
            {"fromNode":"source","fromPort":"condition","toNode":"branch","toPort":"condition"}, {"fromNode":"source","fromPort":"text","toNode":"sink","toPort":"text"}
        ]
    });
    assert!(inspect(&workflow).report().structurally_valid);

    let mut unavailable = workflow;
    unavailable["nodes"][2] = json!({"id":"yes","title":"Yes","operation":{"kind":"capability","capabilityId":capability_id(),"revisionId":revision_id()},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"result","dataType":"text"}]});
    unavailable["nodes"][4]["inputs"] = json!([{"name":"result","dataType":"text"}]);
    unavailable["dataEdges"] = json!([
        {"fromNode":"source","fromPort":"condition","toNode":"branch","toPort":"condition"},
        {"fromNode":"source","fromPort":"text","toNode":"yes","toPort":"text"},
        {"fromNode":"yes","fromPort":"result","toNode":"sink","toPort":"result"}
    ]);
    assert!(codes(&unavailable).contains(&DiagnosticCode::DataNotDominating));
}

#[test]
fn static_check_declarations_are_valid_and_inert() {
    for check in [
        json!({"kind":"non_empty"}),
        json!({"kind":"contains_text","needle":"needle"}),
    ] {
        let workflow = json!({
            "schemaVersion": DEFINITION_SCHEMA, "title":"Check", "nodes":[
                {"id":"source","title":"Source","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"value","dataType":"text"}]},
                {"id":"check","title":"Check","operation":{"kind":"check","check":check},"inputs":[{"name":"value","dataType":"text"}],"outputs":[{"name":"passed","dataType":"boolean"}]},
                {"id":"sink","title":"Sink","operation":{"kind":"output"},"inputs":[{"name":"passed","dataType":"boolean"}],"outputs":[]}
            ], "controlEdges":[
                {"fromNode":"source","outlet":"next","toNode":"check"}, {"fromNode":"check","outlet":"next","toNode":"sink"}
            ], "dataEdges":[
                {"fromNode":"source","fromPort":"value","toNode":"check","toPort":"value"}, {"fromNode":"check","fromPort":"passed","toNode":"sink","toPort":"passed"}
            ]
        });
        let inspection = inspect(&workflow);
        let report = inspection.report();
        assert!(report.structurally_valid);
        assert_eq!(
            serde_json::to_value(report).unwrap()["executionStatus"],
            "unavailable"
        );
    }
}

#[test]
fn data_dominance_stays_in_data_edge_order_before_later_edge_faults() {
    let workflow = json!({
        "schemaVersion": DEFINITION_SCHEMA, "title":"Ordered data", "nodes":[
            {"id":"source","title":"Source","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"condition","dataType":"boolean"},{"name":"text","dataType":"text"}]},
            {"id":"branch","title":"Branch","operation":{"kind":"branch"},"inputs":[{"name":"condition","dataType":"boolean"}],"outputs":[]},
            {"id":"yes","title":"Yes","operation":{"kind":"capability","capabilityId":capability_id(),"revisionId":revision_id()},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"result","dataType":"text"}]},
            {"id":"no","title":"No","operation":{"kind":"checkpoint","prompt":"Review"},"inputs":[],"outputs":[]},
            {"id":"sink","title":"Sink","operation":{"kind":"output"},"inputs":[{"name":"result","dataType":"text"}],"outputs":[]}
        ], "controlEdges":[
            {"fromNode":"source","outlet":"next","toNode":"branch"}, {"fromNode":"branch","outlet":"true","toNode":"yes"}, {"fromNode":"branch","outlet":"false","toNode":"no"},
            {"fromNode":"yes","outlet":"next","toNode":"sink"}, {"fromNode":"no","outlet":"next","toNode":"sink"}
        ], "dataEdges":[
            {"fromNode":"yes","fromPort":"result","toNode":"sink","toPort":"result"},
            {"fromNode":"source","fromPort":"text","toNode":"sink","toPort":"missing"},
            {"fromNode":"source","fromPort":"text","toNode":"yes","toPort":"text"},
            {"fromNode":"source","fromPort":"condition","toNode":"branch","toPort":"condition"}
        ]
    });
    let inspection = inspect(&workflow);
    let report = inspection.report();
    let data_codes: Vec<_> = report
        .diagnostics
        .iter()
        .filter_map(|item| item.data_edge_index.map(|index| (index, item.code)))
        .collect();
    assert_eq!(
        data_codes,
        [
            (0, DiagnosticCode::DataNotDominating),
            (1, DiagnosticCode::MissingInputPort),
        ]
    );
}

#[test]
fn exact_ports_edges_and_input_mappings_are_enforced() {
    let mut value = simple();
    value["nodes"][1]["inputs"][0]["dataType"] = json!("json");
    value["controlEdges"][1]["outlet"] = json!("true");
    let duplicate = value["dataEdges"][0].clone();
    value["dataEdges"].as_array_mut().unwrap().push(duplicate);
    value["dataEdges"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fromNode":"source","fromPort":"text","toNode":"sink","toPort":"missing"}));
    let actual = codes(&value);
    assert!(actual.contains(&DiagnosticCode::TypeMismatch));
    assert!(actual.contains(&DiagnosticCode::InvalidControlOutlet));
    assert!(actual.contains(&DiagnosticCode::ControlCardinality));
    assert!(actual.contains(&DiagnosticCode::DuplicateDataEdge));
    assert!(actual.contains(&DiagnosticCode::MissingInputPort));
}

#[test]
fn direct_mapping_and_graph_negative_cases_have_closed_diagnostics() {
    let mut missing_mapping = simple();
    missing_mapping["dataEdges"]
        .as_array_mut()
        .unwrap()
        .remove(0);

    let mut multiple_producers = simple();
    multiple_producers["dataEdges"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "fromNode":"source","fromPort":"text","toNode":"sink","toPort":"result"
        }));

    for value in [&missing_mapping, &multiple_producers] {
        assert!(codes(value).contains(&DiagnosticCode::InputCardinality));
    }

    let mut unreachable = simple();
    unreachable["nodes"].as_array_mut().unwrap().push(json!({
        "id":"orphan","title":"Orphan","operation":{"kind":"checkpoint","prompt":"Review"},"inputs":[],"outputs":[]
    }));
    let graph_codes = codes(&unreachable);
    assert!(graph_codes.contains(&DiagnosticCode::UnreachableNode));
    assert!(graph_codes.contains(&DiagnosticCode::NoOutputPath));

    let mut incomplete_branch = simple();
    incomplete_branch["nodes"][0]["outputs"] = json!([{"name":"condition","dataType":"boolean"}]);
    incomplete_branch["nodes"][1]["operation"] = json!({"kind":"branch"});
    incomplete_branch["nodes"][1]["inputs"] = json!([{"name":"condition","dataType":"boolean"}]);
    incomplete_branch["nodes"][1]["outputs"] = json!([]);
    assert!(codes(&incomplete_branch).contains(&DiagnosticCode::ControlCardinality));

    let mut endpoint_and_self = simple();
    endpoint_and_self["dataEdges"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "fromNode":"missing","fromPort":"text","toNode":"sink","toPort":"result"
        }));
    endpoint_and_self["dataEdges"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "fromNode":"source","fromPort":"text","toNode":"source","toPort":"text"
        }));
    let data_codes = codes(&endpoint_and_self);
    assert!(data_codes.contains(&DiagnosticCode::MissingDataEndpoint));
    assert!(data_codes.contains(&DiagnosticCode::SelfDataEdge));
}

#[test]
fn diagnostics_follow_the_contract_stage_and_location_order() {
    let mut value = simple();
    value["nodes"][1]["operation"]["capabilityId"] = json!("capability:bad");
    value["nodes"][1]["inputs"][0]["name"] = json!("");
    value["nodes"][2]["operation"] = json!({"kind":"input"});
    value["controlEdges"].as_array_mut().unwrap().push(json!({
        "fromNode":"missing","outlet":"next","toNode":"work"
    }));
    assert_eq!(
        codes(&value),
        [
            DiagnosticCode::InvalidReference,
            DiagnosticCode::InvalidInputPortName,
            DiagnosticCode::InvalidOperationPorts,
            DiagnosticCode::EntryCount,
            DiagnosticCode::OutputMissing,
            DiagnosticCode::MissingControlEndpoint,
            DiagnosticCode::ControlCardinality,
        ]
    );
}

#[test]
fn operation_port_shape_precedes_same_node_detail_diagnostic() {
    let mut capability = simple();
    capability["nodes"][1]["operation"]["capabilityId"] = json!("capability:bad");
    capability["nodes"][1]["outputs"] = json!([]);
    let capability_codes: Vec<_> = inspect(&capability)
        .report()
        .diagnostics
        .iter()
        .filter(|item| item.node_index == Some(1))
        .map(|item| item.code)
        .collect();
    assert_eq!(
        capability_codes,
        [
            DiagnosticCode::InvalidOperationPorts,
            DiagnosticCode::InvalidReference
        ]
    );

    let mut check = simple();
    check["nodes"][1]["operation"] =
        json!({"kind":"check","check":{"kind":"contains_text","needle":""}});
    check["nodes"][1]["inputs"] = json!([]);
    check["nodes"][1]["outputs"] = json!([]);
    let check_codes: Vec<_> = inspect(&check)
        .report()
        .diagnostics
        .iter()
        .filter(|item| item.node_index == Some(1))
        .map(|item| item.code)
        .collect();
    assert_eq!(
        check_codes,
        [
            DiagnosticCode::InvalidOperationPorts,
            DiagnosticCode::InvalidCheckNeedle
        ]
    );

    let mut checkpoint = simple();
    checkpoint["nodes"][1]["operation"] = json!({"kind":"checkpoint","prompt":" "});
    let checkpoint_codes: Vec<_> = inspect(&checkpoint)
        .report()
        .diagnostics
        .iter()
        .filter(|item| item.node_index == Some(1))
        .map(|item| item.code)
        .collect();
    assert_eq!(
        checkpoint_codes,
        [
            DiagnosticCode::InvalidOperationPorts,
            DiagnosticCode::InvalidCheckpointPrompt,
        ]
    );
}

#[test]
fn graph_endpoint_cycle_reachability_and_node_id_errors_are_reported() {
    let mut value = simple();
    value["nodes"][2]["id"] = json!("work");
    let actual = codes(&value);
    assert!(actual.contains(&DiagnosticCode::DuplicateNodeId));
    assert!(!actual.contains(&DiagnosticCode::MissingControlEndpoint));
    assert!(!actual.contains(&DiagnosticCode::SelfControlEdge));

    let mut endpoints = simple();
    endpoints["controlEdges"][0]["toNode"] = json!("gone");
    endpoints["controlEdges"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fromNode":"work","outlet":"next","toNode":"work"}));
    let actual = codes(&endpoints);
    assert!(actual.contains(&DiagnosticCode::MissingControlEndpoint));
    assert!(actual.contains(&DiagnosticCode::SelfControlEdge));

    let mut cycle = simple();
    cycle["controlEdges"][1]["toNode"] = json!("source");
    assert!(codes(&cycle).contains(&DiagnosticCode::Cycle));
}

#[test]
fn invalid_port_namespaces_skip_only_ambiguous_data_mapping_diagnostics() {
    let mut value = simple();
    value["nodes"][1]["inputs"][0]["name"] = json!("");
    value["nodes"][1]["outputs"] = json!([
        {"name":"result","dataType":"text"},
        {"name":"result","dataType":"text"}
    ]);
    value["controlEdges"][0]["toNode"] = json!("gone");
    value["nodes"][2]["inputs"] = json!([
        {"name":"result","dataType":"text"},
        {"name":"flag","dataType":"boolean"}
    ]);
    value["dataEdges"].as_array_mut().unwrap().push(json!({
        "fromNode":"source","fromPort":"text","toNode":"sink","toPort":"flag"
    }));
    value["dataEdges"].as_array_mut().unwrap().push(json!({
        "fromNode":"source","fromPort":"text","toNode":"sink","toPort":"missing"
    }));
    value["dataEdges"].as_array_mut().unwrap().push(json!({
        "fromNode":"work","fromPort":"result","toNode":"sink","toPort":"missing"
    }));
    value["dataEdges"].as_array_mut().unwrap().push(json!({
        "fromNode":"source","fromPort":"missing","toNode":"work","toPort":"text"
    }));
    let actual = codes(&value);
    assert!(actual.contains(&DiagnosticCode::MissingControlEndpoint));
    assert!(actual.contains(&DiagnosticCode::DuplicateOutputPort));
    assert!(actual.contains(&DiagnosticCode::TypeMismatch));
    assert!(actual.contains(&DiagnosticCode::MissingInputPort));
    assert!(actual.contains(&DiagnosticCode::MissingOutputPort));
    assert!(!actual.contains(&DiagnosticCode::InputCardinality));
}

#[test]
fn closed_wire_rejects_hostile_text_unknown_duplicate_and_invalid_input() {
    let mut value = simple();
    value["nodes"][1]["operation"]["capabilityId"] = json!("capability:not-a-digest");
    value["nodes"][1]["title"] = json!("<script>inert</script>");
    assert!(codes(&value).contains(&DiagnosticCode::InvalidReference));
    assert!(
        inspect(&value).definition().nodes[1]
            .title
            .contains("<script>")
    );

    let duplicate = r#"{"schemaVersion":"rangoon.workflow-definition.v1","schemaVersion":"rangoon.workflow-definition.v1"}"#;
    assert_eq!(
        inspect_definition(duplicate.as_bytes()).unwrap_err().code(),
        "invalid_json"
    );
    let nested_duplicate = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"X","nodes":[{"id":"start","title":"Start","operation":{"kind":"input","kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]}],"controlEdges":[],"dataEdges":[]}"#;
    assert_eq!(
        inspect_definition(nested_duplicate.as_bytes())
            .unwrap_err()
            .code(),
        "invalid_json"
    );
    let unit_extra = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"X","nodes":[{"id":"start","title":"Start","operation":{"kind":"input","prompt":"no"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]}],"controlEdges":[],"dataEdges":[]}"#;
    assert_eq!(
        inspect_definition(unit_extra.as_bytes())
            .unwrap_err()
            .code(),
        "invalid_json"
    );
    let nested_variant = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"X","nodes":[{"id":"start","title":"Start","operation":{"kind":"check","check":{"kind":"unknown"}},"inputs":[{"name":"value","dataType":"text"}],"outputs":[{"name":"passed","dataType":"boolean"}]}],"controlEdges":[],"dataEdges":[]}"#;
    assert_eq!(
        inspect_definition(nested_variant.as_bytes())
            .unwrap_err()
            .code(),
        "invalid_json"
    );
    let unknown = r#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"X","nodes":[],"controlEdges":[],"dataEdges":[],"extra":true}"#;
    assert_eq!(
        inspect_definition(unknown.as_bytes()).unwrap_err().code(),
        "invalid_json"
    );
    assert_eq!(
        inspect_definition(&[0xff]).unwrap_err().code(),
        "invalid_encoding"
    );
    assert_eq!(
        inspect_definition(b"{} trailing").unwrap_err().code(),
        "invalid_json"
    );
    assert_eq!(
        inspect_definition(
            br#"{"schemaVersion":"other","title":"X","nodes":[],"controlEdges":[],"dataEdges":[]}"#
        )
        .unwrap_err()
        .code(),
        "unsupported_schema"
    );
    assert_eq!(
        inspect_definition(&vec![b' '; 128 * 1024 + 1])
            .unwrap_err()
            .code(),
        "input_limit"
    );
    let deep = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert_eq!(
        inspect_definition(deep.as_bytes()).unwrap_err().code(),
        "depth_limit"
    );
    let at_depth = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    assert_eq!(
        inspect_definition(at_depth.as_bytes()).unwrap_err().code(),
        "invalid_json"
    );
    let mut brace_text = simple();
    brace_text["title"] = json!("{\\\"still string\\\"}".repeat(6));
    assert!(inspect(&brace_text).report().structurally_valid);
}

#[test]
fn boundaries_include_a_maximum_node_graph_and_bounded_diagnostics() {
    let mut nodes = Vec::new();
    nodes.push(json!({"id":"n0","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"value","dataType":"text"}]}));
    for index in 1..127 {
        nodes.push(json!({"id":format!("n{index}"),"title":"Capability","operation":{"kind":"capability","capabilityId":capability_id(),"revisionId":revision_id()},"inputs":[{"name":"value","dataType":"text"}],"outputs":[{"name":"value","dataType":"text"}]}));
    }
    nodes.push(json!({"id":"n127","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"value","dataType":"text"}],"outputs":[]}));
    let mut controls = Vec::new();
    let mut data = Vec::new();
    for index in 0..127 {
        controls.push(json!({"fromNode":format!("n{index}"),"outlet":"next","toNode":format!("n{}", index + 1)}));
        data.push(json!({"fromNode":format!("n{index}"),"fromPort":"value","toNode":format!("n{}", index + 1),"toPort":"value"}));
    }
    let largest = json!({"schemaVersion":DEFINITION_SCHEMA,"title":"Largest","nodes":nodes,"controlEdges":controls,"dataEdges":data});
    assert!(inspect(&largest).report().structurally_valid);
    let mut too_many = largest.clone();
    let extra_node = too_many["nodes"][0].clone();
    too_many["nodes"].as_array_mut().unwrap().push(extra_node);
    assert_eq!(
        inspect_definition(serde_json::to_vec(&too_many).unwrap().as_slice()).unwrap_err(),
        WorkflowError::CollectionLimit
    );

    let mut at_control_limit = largest.clone();
    let repeated_control = at_control_limit["controlEdges"][0].clone();
    while at_control_limit["controlEdges"].as_array().unwrap().len() < 256 {
        at_control_limit["controlEdges"]
            .as_array_mut()
            .unwrap()
            .push(repeated_control.clone());
    }
    assert!(inspect_definition(serde_json::to_vec(&at_control_limit).unwrap().as_slice()).is_ok());
    at_control_limit["controlEdges"]
        .as_array_mut()
        .unwrap()
        .push(repeated_control);
    assert_eq!(
        inspect_definition(serde_json::to_vec(&at_control_limit).unwrap().as_slice()).unwrap_err(),
        WorkflowError::CollectionLimit
    );

    let mut at_data_limit = largest.clone();
    let repeated_data = at_data_limit["dataEdges"][0].clone();
    while at_data_limit["dataEdges"].as_array().unwrap().len() < 1024 {
        at_data_limit["dataEdges"]
            .as_array_mut()
            .unwrap()
            .push(repeated_data.clone());
    }
    assert!(inspect_definition(serde_json::to_vec(&at_data_limit).unwrap().as_slice()).is_ok());
    at_data_limit["dataEdges"]
        .as_array_mut()
        .unwrap()
        .push(repeated_data);
    assert_eq!(
        inspect_definition(serde_json::to_vec(&at_data_limit).unwrap().as_slice()).unwrap_err(),
        WorkflowError::CollectionLimit
    );

    let mut nine_ports = simple();
    for index in 0..7 {
        nine_ports["nodes"][0]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":format!("extra{index}"),"dataType":"text"}));
    }
    assert!(inspect_definition(serde_json::to_vec(&nine_ports).unwrap().as_slice()).is_ok());
    nine_ports["nodes"][0]["outputs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"extra8","dataType":"text"}));
    assert_eq!(
        inspect_definition(serde_json::to_vec(&nine_ports).unwrap().as_slice()).unwrap_err(),
        WorkflowError::CollectionLimit
    );

    for node_count in [0, 1] {
        let mut too_few = simple();
        too_few["nodes"] = json!(simple()["nodes"].as_array().unwrap()[..node_count].to_vec());
        too_few["controlEdges"] = json!([]);
        too_few["dataEdges"] = json!([]);
        let inspection = inspect(&too_few);
        let report = inspection.report().clone();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::InvalidNodeCount)
        );
        assert!(!report.structurally_valid);
        assert!(report.definition_id.is_none());
        assert!(!inspection.canonical_definition_bytes().unwrap().is_empty());
    }

    let raw = serde_json::to_vec(&simple()).unwrap();
    let mut exactly_at_limit = raw;
    exactly_at_limit.resize(128 * 1024, b' ');
    assert!(
        inspect_definition(&exactly_at_limit)
            .unwrap()
            .report()
            .structurally_valid
    );

    let mut noisy = largest;
    for node in noisy["nodes"].as_array_mut().unwrap() {
        node["id"] = json!("");
        node["title"] = json!("");
    }
    let report = inspect(&noisy).report().clone();
    assert_eq!(report.diagnostics.len(), 128);
    assert!(report.diagnostics_truncated);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .take(5)
            .map(|item| item.code)
            .collect::<Vec<_>>(),
        [
            DiagnosticCode::InvalidNodeId,
            DiagnosticCode::InvalidTitle,
            DiagnosticCode::InvalidNodeId,
            DiagnosticCode::DuplicateNodeId,
            DiagnosticCode::InvalidTitle,
        ]
    );
}
