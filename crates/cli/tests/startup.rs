use std::process::Command;

#[test]
fn cli_starts_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .output()
        .expect("CLI should launch");

    assert!(output.status.success(), "CLI failed: {output:?}");
    assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("CLI output should be UTF-8");
    assert!(stdout.contains("Pipeline Autopsy"));
    assert!(stdout.contains(pipeline_autopsy_core::VERSION));
}

fn sales_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sales/pipeline.yaml")
}

#[test]
fn validates_a_definition_from_another_working_directory() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .arg("validate")
        .arg(sales_path())
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("4 stages, 3 dependencies"));
}

#[test]
fn ingests_both_csv_sources_without_claiming_sql_execution() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .arg("ingest")
        .arg(sales_path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("orders: 3 rows, 4 columns"));
    assert!(stdout.contains("customers: 2 rows, 2 columns"));
    assert!(stdout.contains("SQL stages were not executed"));
}

#[test]
fn invalid_commands_and_missing_files_exit_unsuccessfully() {
    for args in [
        vec!["run"],
        vec!["validate", "this-file-does-not-exist.yaml"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("error:")
        );
    }
}

#[test]
fn plan_prints_a_deterministic_order_without_executing_sql() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .arg("plan")
        .arg(sales_path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("1. customers\n2. orders\n3. join_customers\n4. daily_revenue"));
    assert!(stdout.contains("Plan only; no stages were executed."));
}

#[test]
fn run_executes_the_sales_pipeline_and_reports_actual_counts() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .arg("run")
        .arg(sales_path())
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("join_customers: 2 rows, 5 columns"));
    assert!(stdout.contains("daily_revenue: 1 rows, 2 columns"));
    assert!(stdout.contains("Execution completed."));
    assert!(stdout.contains("Contracts were not evaluated"));
}

#[test]
fn prepare_persists_unique_pending_identities_across_cli_processes() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("metadata.sqlite");
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
            .arg("prepare")
            .arg(sales_path())
            .arg(&database)
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let id = stdout
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(2)
            .unwrap();
        assert!(ids.insert(id.to_owned()));
        assert_eq!(stdout.matches("Stage execution:").count(), 4);
        assert!(stdout.contains(&format!("{id}/join_customers")));
        assert!(stdout.contains("(pending)"));
        assert!(stdout.contains("No stages were executed."));
    }
    assert!(database.is_file());
}
