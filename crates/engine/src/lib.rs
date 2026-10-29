//! YAML definitions, dependency graphs, and isolated DuckDB CSV ingestion.
//! Serial scheduling dispatches executor callbacks; SQL execution is T2.7.
pub mod error;
pub mod graph;
pub mod scheduler;
pub mod sources;
pub mod spec;

pub use error::{EngineError, GraphIssue, GraphValidationError};
pub use graph::PipelineGraph;
pub use scheduler::{DispatchError, ScheduleError, StageScheduler, plan_execution};
pub use sources::{SourceInfo, SourceSession};
pub use spec::{LoadedPipeline, load_pipeline, parse_pipeline, validate_pipeline};
