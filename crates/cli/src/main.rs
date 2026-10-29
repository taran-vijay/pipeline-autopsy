use std::{ffi::OsString, process::ExitCode};

use pipeline_autopsy_engine::{SourceSession, load_pipeline, plan_execution};

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    match args.as_slice() {
        [] => {
            println!("Pipeline Autopsy {}", pipeline_autopsy_core::VERSION);
            println!(
                "YAML validation, execution planning, and CSV ingestion are available. SQL execution is coming later."
            );
            println!("Use --help for commands.");
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("Pipeline Autopsy {}", pipeline_autopsy_core::VERSION);
            println!("Usage: pipeline-autopsy <validate|plan|ingest> <pipeline.yaml>");
            println!(
                "  validate  Check YAML, definitions, graph, and local file paths (not SQL syntax)"
            );
            println!("  plan      Show deterministic stage order without executing stages");
            println!(
                "  ingest    Validate and materialize CSV sources in an isolated DuckDB session"
            );
        }
        [command, path] if command == "validate" || command == "plan" || command == "ingest" => {
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
                "usage: pipeline-autopsy <validate|plan|ingest> <pipeline.yaml> (or --help)".into(),
            );
        }
    }
    Ok(())
}
