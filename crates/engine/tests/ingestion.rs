use pipeline_autopsy_core::StageId;
use pipeline_autopsy_engine::{EngineError, SourceSession, load_pipeline};
use std::{fs, path::Path};

fn setup(root: &Path, csv: &str) -> std::path::PathBuf {
    fs::write(root.join("input's file.csv"), csv).unwrap();
    fs::write(
        root.join("query.sql"),
        "NOT EXECUTABLE SQL; this must not run",
    )
    .unwrap();
    let path = root.join("pipeline.yaml");
    fs::write(&path, "schema_version: 1\nid: local\nversion: '1'\nname: Local\nstages:\n  - id: source\n    name: Source\n    operation:\n      kind: csv_source\n      path: input's file.csv\n  - id: transform\n    name: Transform\n    inputs: [source]\n    operation:\n      kind: sql\n      path: query.sql\n").unwrap();
    path
}

#[test]
fn imports_typed_records_quotes_newlines_and_nulls_without_executing_sql() {
    let temporary = tempfile::Builder::new()
        .prefix("autopsy's csv café ")
        .tempdir()
        .unwrap();
    let path = setup(
        temporary.path(),
        "id,name,notes,optional\n1,Ada,\"north,south\",\n2,Grace,\"two\nlines\",value\n3,Lín,\"\",\n",
    );
    let session = SourceSession::ingest(&load_pipeline(&path).unwrap()).unwrap();
    let source = &session.sources()[&StageId::from("source")];
    assert_eq!(source.row_count, 3);
    assert_eq!(source.columns[0].name, "id");
    assert_eq!(source.columns[0].data_type, "BIGINT");
    assert_eq!(source.columns[1].data_type, "VARCHAR");
    let notes: String = session
        .connection()
        .query_row("SELECT notes FROM source WHERE id=2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(notes, "two\nlines");
    let quoted: String = session
        .connection()
        .query_row("SELECT notes FROM source WHERE id=3", [], |r| r.get(0))
        .unwrap();
    assert_eq!(quoted, "");
    let name: String = session
        .connection()
        .query_row("SELECT name FROM source WHERE id=3", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "Lín");
    let blank: bool = session
        .connection()
        .query_row("SELECT optional IS NULL FROM source WHERE id=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(blank);
    assert!(
        session
            .connection()
            .query_row("SELECT COUNT(*) FROM transform", [], |r| r.get::<_, u64>(0))
            .is_err()
    );
}

#[test]
fn materializes_sources_and_keeps_sessions_isolated() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(temporary.path(), "id\n1\n2\n");
    let loaded = load_pipeline(&path).unwrap();
    let first = SourceSession::ingest(&loaded).unwrap();
    fs::write(temporary.path().join("input's file.csv"), "id\n9\n").unwrap();
    let second = SourceSession::ingest(&loaded).unwrap();
    assert_eq!(
        first
            .connection()
            .query_row("SELECT COUNT(*) FROM source", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        2
    );
    assert_eq!(second.sources()[&StageId::from("source")].row_count, 1);
}

#[test]
fn malformed_csv_fails_with_stage_context_and_header_only_csv_is_empty() {
    let temporary = tempfile::tempdir().unwrap();
    let path = setup(temporary.path(), "id,name\n1,Ada\n2,Grace,extra\n");
    for malformed in [
        "id,name\n1,Ada\n2,Grace,extra\n",
        "id,name\n1,Ada\n2\n",
        "id,name\n1,\"unfinished\n",
        "id,name\n1,\"Ada\"junk\n",
    ] {
        fs::write(temporary.path().join("input's file.csv"), malformed).unwrap();
        let result = SourceSession::ingest(&load_pipeline(&path).unwrap());
        assert!(
            matches!(&result, Err(EngineError::Csv { stage, .. }) if stage == "source"),
            "unexpected ingestion result for {malformed:?}: {:?}",
            result.as_ref().map(|session| session.sources())
        );
    }
    fs::write(temporary.path().join("input's file.csv"), "id,name\n").unwrap();
    let session = SourceSession::ingest(&load_pipeline(&path).unwrap()).unwrap();
    assert_eq!(session.sources()[&StageId::from("source")].row_count, 0);
}

#[test]
fn reads_all_rows_for_type_inference_and_handles_quoted_sql_names() {
    let temporary = tempfile::tempdir().unwrap();
    let mut csv = String::from("id\n");
    for i in 0..25_000 {
        csv.push_str(&format!("{i}\n"));
    }
    csv.push_str("late-text\n");
    let path = setup(temporary.path(), &csv);
    let yaml = fs::read_to_string(&path)
        .unwrap()
        .replace("id: source", "id: select")
        .replace("inputs: [source]", "inputs: [select]");
    fs::write(&path, yaml).unwrap();
    let session = SourceSession::ingest(&load_pipeline(&path).unwrap()).unwrap();
    let source = &session.sources()[&StageId::from("select")];
    assert_eq!(source.row_count, 25_001);
    assert_eq!(source.columns[0].data_type, "VARCHAR");
    assert_eq!(
        session
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM \"select\" WHERE id='late-text'",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn sales_fixture_registers_both_inputs_and_preserves_unmatched_records() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sales/pipeline.yaml");
    let session = SourceSession::ingest(&load_pipeline(path).unwrap()).unwrap();
    assert_eq!(session.sources().len(), 2);
    assert_eq!(session.sources()[&StageId::from("orders")].row_count, 3);
    assert_eq!(session.sources()[&StageId::from("customers")].row_count, 2);
    assert_eq!(
        session
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM orders WHERE customer_id=999",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        1
    );
}
