use std::{fs, path::PathBuf};

use pipeline_autopsy_core::{ContractRule, StageId, StageOperation};
use pipeline_autopsy_engine::{EngineError, load_pipeline, parse_pipeline, validate_pipeline};

const SALES: &str = include_str!("../../../examples/sales/pipeline.yaml");

#[test]
fn parses_shared_models_and_builds_input_to_consumer_edges() {
    let mut pipeline = parse_pipeline(SALES).unwrap();
    // Definitions need not be written in execution order.
    pipeline.stages.reverse();
    let graph = validate_pipeline(&pipeline).unwrap();
    assert_eq!(graph.graph().node_count(), 4);
    assert_eq!(graph.graph().edge_count(), 3);
    assert_eq!(
        graph.inputs(&StageId::from("join_customers")).unwrap(),
        vec![StageId::from("customers"), StageId::from("orders")]
    );
    assert_eq!(
        graph.consumers(&StageId::from("join_customers")).unwrap(),
        vec![StageId::from("daily_revenue")]
    );
    assert_eq!(graph.inputs(&StageId::from("missing")), None);
}

#[test]
fn rejects_yaml_typos_duplicate_keys_and_multiple_documents() {
    for invalid in [
        SALES.replace("schema_version: 1", "schema_version: 1\nschema_version: 1"),
        SALES.replace("path: data/orders.csv", "paht: data/orders.csv"),
        SALES.replace("kind: csv_source", "kind: python"),
        format!("{SALES}\n---\n{SALES}"),
        "schema_version: [".into(),
    ] {
        assert!(matches!(
            parse_pipeline(&invalid),
            Err(EngineError::Yaml(_))
        ));
    }
}

#[test]
fn rejects_unsupported_versions_empty_ids_and_invalid_stage_shapes() {
    for bad in [
        SALES.replace("schema_version: 1", "schema_version: 2"),
        SALES.replace("id: sales_fixture", "id: ''"),
        SALES.replace("id: orders", "id: bad-name"),
        SALES.replace("name: Orders", "name: ''"),
        SALES.replace("name: Orders", "name: Orders\n    inputs: [customers]"),
        SALES.replace("inputs: [join_customers]", "inputs: []"),
        SALES.replace("path: data/orders.csv", "path: /absolute.csv"),
    ] {
        assert!(parse_pipeline(&bad).is_err(), "accepted {bad}");
    }
    let mut pipeline = parse_pipeline(SALES).unwrap();
    pipeline.stages.clear();
    assert!(validate_pipeline(&pipeline).is_err());
}

#[test]
fn rejects_missing_inputs_duplicate_names_edges_and_cycles() {
    let pipeline = parse_pipeline(SALES).unwrap();
    let mut missing = pipeline.clone();
    missing.stages[2].inputs[0] = StageId::from("absent");
    assert!(matches!(
        validate_pipeline(&missing),
        Err(EngineError::Graph(_))
    ));
    let mut names = pipeline.clone();
    names.stages[1].id = StageId::from("ORDERS");
    assert!(matches!(
        validate_pipeline(&names),
        Err(EngineError::Graph(_))
    ));
    let mut edges = pipeline.clone();
    edges.stages[2].inputs.push(StageId::from("orders"));
    assert!(validate_pipeline(&edges).is_err());
    let mut cycle = pipeline.clone();
    cycle.stages[2].inputs.push(StageId::from("daily_revenue"));
    assert!(matches!(
        validate_pipeline(&cycle),
        Err(EngineError::Graph(_))
    ));
    let mut self_loop = pipeline;
    self_loop.stages[3]
        .inputs
        .push(StageId::from("daily_revenue"));
    assert!(matches!(
        validate_pipeline(&self_loop),
        Err(EngineError::Graph(_))
    ));
}

#[test]
fn validates_contract_targets_keys_and_thresholds_without_evaluating_data() {
    let original = parse_pipeline(SALES).unwrap();
    let mut target = original.clone();
    target.contracts[0].stage_id = StageId::from("absent");
    assert!(validate_pipeline(&target).is_err());
    let mut duplicate = original.clone();
    duplicate.contracts.push(duplicate.contracts[0].clone());
    assert!(validate_pipeline(&duplicate).is_err());
    for ratio in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
        let mut pipeline = original.clone();
        pipeline.contracts[0].rule = ContractRule::RowLoss {
            input_stage: StageId::from("orders"),
            max_loss_ratio: ratio,
        };
        assert!(validate_pipeline(&pipeline).is_err());
    }
    let mut columns = original.clone();
    columns.contracts[0].rule = ContractRule::Unique { columns: vec![] };
    assert!(validate_pipeline(&columns).is_err());
    let mut reference = original;
    if let ContractRule::JoinCoverage {
        reference_stage, ..
    } = &mut reference.contracts[0].rule
    {
        *reference_stage = StageId::from("daily_revenue");
    }
    assert!(validate_pipeline(&reference).is_err());
}

fn local_definition(root: &std::path::Path, data_path: &str) -> PathBuf {
    let text = format!(
        "schema_version: 1\nid: local\nversion: '1'\nname: Local\nstages:\n  - id: source\n    name: Source\n    operation:\n      kind: csv_source\n      path: {data_path}\n"
    );
    let path = root.join("pipeline.yaml");
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn resolves_paths_from_definition_directory_and_rejects_missing_or_escaped_files() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("input.csv"), "id\n1\n").unwrap();
    let path = local_definition(&project, "input.csv");
    let loaded = load_pipeline(&path).unwrap();
    assert_eq!(
        loaded.stage_path(&StageId::from("source")).unwrap(),
        fs::canonicalize(project.join("input.csv")).unwrap()
    );
    assert_eq!(loaded.base_dir(), fs::canonicalize(&project).unwrap());
    local_definition(&project, "missing.csv");
    assert!(matches!(load_pipeline(&path), Err(EngineError::Io { .. })));
    fs::write(temporary.path().join("outside.csv"), "id\n1\n").unwrap();
    local_definition(&project, "../outside.csv");
    assert!(
        load_pipeline(&path)
            .unwrap_err()
            .to_string()
            .contains("escapes")
    );
    local_definition(&project, ".");
    assert!(load_pipeline(&path).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape_and_literal_glob_file_names() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(temporary.path().join("outside.csv"), "id\n1\n").unwrap();
    std::os::unix::fs::symlink(
        temporary.path().join("outside.csv"),
        project.join("input.csv"),
    )
    .unwrap();
    let path = local_definition(&project, "input.csv");
    assert!(
        load_pipeline(&path)
            .unwrap_err()
            .to_string()
            .contains("escapes")
    );
    fs::write(project.join("data*.csv"), "id\n1\n").unwrap();
    local_definition(&project, "data*.csv");
    assert!(
        load_pipeline(&path)
            .unwrap_err()
            .to_string()
            .contains("glob")
    );
}

#[test]
fn operation_payload_round_trips_through_existing_json_contract() {
    let pipeline = parse_pipeline(SALES).unwrap();
    let wire = serde_json::to_value(&pipeline).unwrap();
    let restored: pipeline_autopsy_core::Pipeline = serde_json::from_value(wire).unwrap();
    assert_eq!(pipeline, restored);
    assert!(matches!(
        &restored.stages[0].operation,
        StageOperation::CsvSource { .. }
    ));
}
