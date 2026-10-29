use std::{ffi::OsString, process::ExitCode};

use pipeline_autopsy_engine::{ExecutionSession, SourceSession, load_pipeline, plan_execution};

#[tokio::main]
async fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    match args.as_slice() {
        [] => {
            println!("Pipeline Autopsy {}", pipeline_autopsy_core::VERSION);
            println!(
                "YAML validation, execution planning, CSV ingestion, and SQL stage execution are available."
            );
            println!("Use --help for commands.");
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("Pipeline Autopsy {}", pipeline_autopsy_core::VERSION);
            println!(
                "Usage: pipeline-autopsy <validate|plan|ingest|run> <pipeline.yaml>; prepare <pipeline.yaml> <metadata.sqlite>"
            );
            println!(
                "  validate  Check YAML, definitions, graph, and local file paths (not SQL syntax)"
            );
            println!(
                "  prepare <pipeline.yaml> <metadata.sqlite>  Persist a new pending run and stage IDs"
            );
            println!("  run       Execute CSV and SQL stages in an isolated DuckDB session");
            println!("  plan      Show deterministic stage order without executing stages");
            println!(
                "  ingest    Validate and materialize CSV sources in an isolated DuckDB session"
            );
        }
        [command, path, database] if command == "prepare" => {
            let loaded = load_pipeline(path)?;
            let pool = pipeline_autopsy_storage::open_database(database).await?;
            let result = pipeline_autopsy_tracker::prepare_run(&pool, loaded.pipeline()).await;
            pool.close().await;
            let record = result?;
            println!("Prepared run {} (pending)", record.id);
            for stage in &record.stages {
                println!("Stage execution: {}", stage.execution_id(&record.id));
            }
            println!(
                "Saved to {}. No stages were executed.",
                database.to_string_lossy()
            );
        }
        [command, path]
            if command == "validate"
                || command == "plan"
                || command == "ingest"
                || command == "run" =>
        {
            let loaded = load_pipeline(path)?;
            println!(
                "Validated {} revision {}: {} stages, {} dependencies",
                loaded.pipeline().id,
                loaded.pipeline().version,
                loaded.graph().graph().node_count(),
                loaded.graph().graph().edge_count()
            );
            if command == "plan" {
                println!("Execution order (serial; ready-stage ties use stage IDs):");
                for (index, stage) in plan_execution(loaded.graph())?.iter().enumerate() {
                    println!("{}. {stage}", index + 1);
                }
                println!("Plan only; no stages were executed.");
            }
            if command == "run" {
                let session = ExecutionSession::run(&loaded)?;
                for output in session.outputs() {
                    println!(
                        "{}: {} rows, {} columns",
                        output.stage_id,
                        output.row_count,
                        output.columns.len()
                    );
                }
                println!(
                    "Execution completed. Contracts were not evaluated; results are discarded on exit."
                );
            }
            if command == "ingest" {
                let session = SourceSession::ingest(&loaded)?;
                for source in session.sources().values() {
                    println!(
                        "{}: {} rows, {} columns",
                        source.stage_id,
                        source.row_count,
                        source.columns.len()
                    );
                }
                println!(
                    "CSV sources loaded. SQL stages were not executed; the session is discarded on exit."
                );
            }
        }
        _ => {
            return Err(
                "usage: pipeline-autopsy <validate|plan|ingest|run> <pipeline.yaml>; prepare <pipeline.yaml> <metadata.sqlite> (or --help)"
                    .into(),
            );
        }
    }
    Ok(())
}
