//! YAML definitions, dependency graphs, and isolated DuckDB CSV ingestion.
//! SQL stage execution and scheduling are subsequent Track 2 tasks.
pub mod error;
pub mod graph;
pub mod sources;
pub mod spec;

pub use error::EngineError;
pub use graph::PipelineGraph;
pub use sources::{SourceInfo, SourceSession};
pub use spec::{LoadedPipeline, load_pipeline, parse_pipeline, validate_pipeline};
