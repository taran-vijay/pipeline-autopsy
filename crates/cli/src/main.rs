use std::{ffi::OsString, process::ExitCode};

use pipeline_autopsy_engine::{SourceSession, load_pipeline};

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
                "YAML validation and CSV ingestion are available. SQL execution is coming later."
            );
            println!("Use --help for commands.");
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("Pipeline Autopsy {}", pipeline_autopsy_core::VERSION);
            println!("Usage: pipeline-autopsy <validate|ingest> <pipeline.yaml>");
            println!(
                "  validate  Check YAML, definitions, graph, and local file paths (not SQL syntax)"
            );
            println!(
                "  ingest    Validate and materialize CSV sources in an isolated DuckDB session"
            );
        }
        [command, path] if command == "validate" || command == "ingest" => {
            let loaded = load_pipeline(path)?;
            println!(
                "Validated {} revision {}: {} stages, {} dependencies",
                loaded.pipeline().id,
                loaded.pipeline().version,
                loaded.graph().graph().node_count(),
                loaded.graph().graph().edge_count()
            );
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
                "usage: pipeline-autopsy <validate|ingest> <pipeline.yaml> (or --help)".into(),
            );
        }
    }
    Ok(())
}
