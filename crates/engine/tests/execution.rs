use pipeline_autopsy_core::{Pipeline, StageId};
use pipeline_autopsy_engine::{EngineError, ExecutionSession, load_pipeline};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn setup(root: &Path, first: &str, second: Option<&str>) -> PathBuf {
    fs::write(root.join("source.csv"), "id,name\n1,Ada\n2,Grace\n3,Lin\n").unwrap();
    fs::write(root.join("first.sql"), first).unwrap();
    let mut yaml = String::from(
        "schema_version: 1\nid: sql_demo\nversion: '1'\nname: SQL demo\nstages:\n  - id: source\n    name: Source\n    operation: {kind: csv_source, path: source.csv}\n  - id: result\n    name: Result\n    inputs: [source]\n    operation: {kind: sql, path: first.sql}\n",
    );
    if let Some(query) = second {
        fs::write(root.join("second.sql"), query).unwrap();
        yaml.push_str("  - id: final_result\n    name: Final\n    inputs: [result]\n    operation: {kind: sql, path: second.sql}\n");
    }
    let path = root.join("pipeline.yaml");
    fs::write(&path, yaml).unwrap();
    path
}

#[test]
fn sales_join_and_aggregate_produce_real_materialized_results() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sales/pipeline.yaml");
    let session = ExecutionSession::run(&load_pipeline(path).unwrap()).unwrap();
    let stages: Vec<_> = session
        .outputs()
        .iter()
        .map(|output| (output.stage_id.0.as_str(), output.row_count))
        .collect();
    assert_eq!(
        stages,
        vec![
            ("customers", 2),
            ("orders", 3),
            ("join_customers", 2),
            ("daily_revenue", 1)
        ]
    );
    let revenue: f64 = session
        .connection()
        .query_row("SELECT revenue FROM daily_revenue", [], |r| r.get(0))
        .unwrap();
    assert!((revenue - 49.45).abs() < 1e-9);
    let unmatched: u64 = session
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM join_customers WHERE customer_id=999",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unmatched, 0);
    let tables: u64 = session
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_type='BASE TABLE'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 4);
}

#[test]
fn executes_shuffled_declarations_in_dependency_order() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(
        temporary.path(),
        "SELECT id * 2 AS doubled FROM source",
        Some("SELECT SUM(doubled) AS total FROM result"),
    );
    let loaded = load_pipeline(&path).unwrap();
    let mut definition: Pipeline = loaded.pipeline().clone();
    definition.stages.reverse();
    // JSON is a YAML subset and uses exactly the same typed definition format.
    fs::write(&path, serde_json::to_string(&definition).unwrap()).unwrap();
    let session = ExecutionSession::run(&load_pipeline(path).unwrap()).unwrap();
    assert_eq!(
        session
            .outputs()
            .iter()
            .map(|o| o.stage_id.0.as_str())
            .collect::<Vec<_>>(),
        ["source", "result", "final_result"]
    );
    assert_eq!(
        session
            .connection()
            .query_row("SELECT total FROM final_result", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        12
    );
}

#[test]
fn supports_ctes_subqueries_set_operations_and_semicolons_in_literals() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(
        temporary.path(),
        "-- leading ; comment\nWITH filtered AS (SELECT id FROM SOURCE WHERE id > 1), picked AS (SELECT * FROM filtered) SELECT id, 'semi; -- text' AS label FROM (SELECT * FROM picked) AS p UNION ALL SELECT id, 'Ada''s text' FROM source WHERE id=1; /* trailing ; comment */",
        None,
    );
    let session = ExecutionSession::run(&load_pipeline(path).unwrap()).unwrap();
    assert_eq!(session.outputs()[1].row_count, 3);
    assert_eq!(
        session
            .connection()
            .query_row("SELECT label FROM result WHERE id=2", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "semi; -- text"
    );
    assert_eq!(
        session
            .connection()
            .query_row("SELECT label FROM result WHERE id=1", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "Ada's text"
    );
}

#[test]
fn zero_row_results_retain_schema_and_feed_downstream_aggregates() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(
        temporary.path(),
        "SELECT id, name FROM source WHERE FALSE",
        Some("SELECT COUNT(*) AS n FROM result"),
    );
    let session = ExecutionSession::run(&load_pipeline(path).unwrap()).unwrap();
    assert_eq!(session.outputs()[1].row_count, 0);
    assert_eq!(
        session.outputs()[1]
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "name"]
    );
    assert_eq!(session.outputs()[2].row_count, 1);
    assert_eq!(
        session
            .connection()
            .query_row("SELECT n FROM final_result", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn snapshots_intermediate_tables_and_isolates_repeated_runs() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(
        temporary.path(),
        "SELECT id FROM source WHERE id > 1",
        Some("SELECT SUM(id) AS total FROM result"),
    );
    let loaded = load_pipeline(&path).unwrap();
    let first = ExecutionSession::run(&loaded).unwrap();
    first
        .connection()
        .execute("DELETE FROM source", [])
        .unwrap();
    assert_eq!(
        first
            .connection()
            .query_row("SELECT COUNT(*) FROM result", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        first
            .connection()
            .query_row("SELECT total FROM final_result", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    fs::write(temporary.path().join("source.csv"), "id,name\n7,New\n").unwrap();
    let second = ExecutionSession::run(&loaded).unwrap();
    assert_eq!(second.outputs()[1].row_count, 1);
    assert_eq!(
        second
            .connection()
            .query_row("SELECT total FROM final_result", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        first
            .connection()
            .query_row("SELECT total FROM final_result", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
}

#[test]
fn rejects_scripts_mutations_and_unsupported_sources_before_execution() {
    let temporary = tempfile::tempdir().unwrap();
    for query in [
        "",
        "-- comment only",
        "SELECT * FROM source; DROP TABLE source;",
        "CREATE TABLE other AS SELECT * FROM source",
        "DELETE FROM source",
        "SELECT * INTO other FROM source",
        "SELECT * FROM read_csv('source.csv')",
        "SELECT * FROM 'source.csv'",
        "SELECT * FROM main.source",
        "TABLE source",
        "WITH RECURSIVE c AS (SELECT * FROM source) SELECT * FROM c",
        "WITH source AS (SELECT 1 AS id) SELECT * FROM source",
        "WITH sqlite_master AS (SELECT * FROM sqlite_master) SELECT * FROM sqlite_master",
        "WITH first AS (SELECT * FROM later), later AS (SELECT * FROM source) SELECT * FROM first",
        "SELECT * FROM (WITH scoped AS (SELECT * FROM source) SELECT * FROM scoped) p, scoped",
    ] {
        let path = setup(temporary.path(), query, None);
        let error = match ExecutionSession::run(&load_pipeline(path).unwrap()) {
            Err(error) => error,
            Ok(_) => panic!("accepted {query}"),
        };
        assert!(
            matches!(error, EngineError::SqlDefinition { stage, .. } if stage == "result"),
            "unexpected error for {query}"
        );
    }
    let target = temporary.path().join("must-not-be-written.csv");
    let query = format!(
        "SELECT * FROM source; COPY source TO '{}';",
        target.display()
    );
    let path = setup(temporary.path(), &query, None);
    assert!(ExecutionSession::run(&load_pipeline(path).unwrap()).is_err());
    assert!(!target.exists());
}

#[test]
fn rejects_undeclared_inputs_even_when_another_stage_would_have_loaded_them() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(temporary.path(), "SELECT * FROM hidden", None);
    let mut yaml = fs::read_to_string(&path).unwrap();
    yaml.push_str(
        "  - id: hidden\n    name: Hidden\n    operation: {kind: csv_source, path: source.csv}\n",
    );
    fs::write(&path, yaml).unwrap();
    for query in [
        "SELECT * FROM hidden",
        "SELECT * FROM source WHERE EXISTS(SELECT 1 FROM hidden)",
    ] {
        fs::write(temporary.path().join("first.sql"), query).unwrap();
        match ExecutionSession::run(&load_pipeline(&path).unwrap()) {
            Err(EngineError::SqlDefinition { message, .. }) => {
                assert!(message.contains("not a declared input"))
            }
            _ => panic!("undeclared relation was accepted"),
        }
    }
}

#[test]
fn reports_parser_binder_and_file_errors_with_stage_and_path() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(temporary.path(), "SELEC * FROM source", None);
    let loaded = load_pipeline(&path).unwrap();
    assert!(
        matches!(ExecutionSession::run(&loaded), Err(EngineError::SqlDefinition { stage, path, .. }) if stage=="result" && path.ends_with("first.sql"))
    );
    fs::write(
        temporary.path().join("first.sql"),
        "SELECT missing_column FROM source",
    )
    .unwrap();
    match ExecutionSession::run(&loaded) {
        Err(EngineError::Sql {
            stage,
            path,
            source,
        }) => {
            assert_eq!(stage, "result");
            assert!(path.ends_with("first.sql"));
            assert!(source.to_string().contains("missing_column"));
        }
        _ => panic!("expected a binder error"),
    }
    fs::remove_file(temporary.path().join("first.sql")).unwrap();
    assert!(
        matches!(ExecutionSession::run(&loaded), Err(EngineError::SqlRead { stage, .. }) if stage=="result")
    );
}

#[test]
fn quotes_output_names_that_are_sql_keywords() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(temporary.path(), "SELECT id FROM source", None);
    let yaml = fs::read_to_string(&path)
        .unwrap()
        .replace("id: result", "id: select");
    fs::write(&path, yaml).unwrap();
    let session = ExecutionSession::run(&load_pipeline(path).unwrap()).unwrap();
    assert_eq!(session.outputs()[1].stage_id, StageId::from("select"));
    assert_eq!(
        session
            .connection()
            .query_row("SELECT COUNT(*) FROM \"select\"", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        3
    );
}
